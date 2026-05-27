// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::thread;

use uuid::Uuid;

use crate::serve_api::{
    AsyncJobState, AsyncJobStatusResponse, AsyncScanAcceptedResponse, SyncScanRequest,
};

const DEFAULT_ASYNC_MAX_PROCESSORS_PER_JOB: usize = 4;
const DEFAULT_ASYNC_RETAINED_TERMINAL_JOBS: usize = 64;

pub(super) enum JobOutcome {
    Succeeded { result_body: String },
    Failed { message: String, status_code: u16 },
}

#[derive(Debug, Clone)]
pub(super) struct AsyncJobController {
    inner: Arc<Mutex<AsyncJobControllerState>>,
    processor_budget: usize,
    max_processors_per_job: usize,
    max_retained_terminal_jobs: usize,
}

#[derive(Debug)]
struct AsyncJobControllerState {
    active_processors: usize,
    jobs: HashMap<String, AsyncJobRecord>,
    pending: VecDeque<PendingAsyncJob>,
    completed: VecDeque<String>,
}

#[derive(Debug)]
struct PendingAsyncJob {
    job_id: String,
    request: SyncScanRequest,
}

#[derive(Debug)]
pub(super) struct AsyncJobRecord {
    pub(super) state: AsyncJobState,
    pub(super) allocated_processors: Option<usize>,
    pub(super) result_body: Option<String>,
    pub(super) error_message: Option<String>,
    pub(super) error_status_code: Option<u16>,
}

#[derive(Debug)]
pub(super) struct DispatchedAsyncJob {
    pub(super) job_id: String,
    pub(super) request: SyncScanRequest,
    pub(super) allocated_processors: usize,
}

#[derive(Debug, Clone)]
pub(super) struct AsyncJobSnapshot {
    job_id: String,
    state: AsyncJobState,
    allocated_processors: Option<usize>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct AsyncJobResultSnapshot {
    pub(super) state: AsyncJobState,
    pub(super) result_body: Option<String>,
    pub(super) error_message: Option<String>,
    pub(super) error_status_code: Option<u16>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AsyncSubmitError {
    QueueFull,
}

impl AsyncJobController {
    pub(super) fn new() -> Self {
        let processor_budget = default_async_processor_budget();
        Self::with_limits(
            processor_budget,
            processor_budget.clamp(1, DEFAULT_ASYNC_MAX_PROCESSORS_PER_JOB),
            DEFAULT_ASYNC_RETAINED_TERMINAL_JOBS,
        )
    }

    pub(super) fn with_limits(
        processor_budget: usize,
        max_processors_per_job: usize,
        max_retained_terminal_jobs: usize,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AsyncJobControllerState {
                active_processors: 0,
                jobs: HashMap::new(),
                pending: VecDeque::new(),
                completed: VecDeque::new(),
            })),
            processor_budget: processor_budget.max(1),
            max_processors_per_job: max_processors_per_job.max(1),
            max_retained_terminal_jobs: max_retained_terminal_jobs.max(1),
        }
    }

    pub(super) fn submit(
        &self,
        request: SyncScanRequest,
    ) -> std::result::Result<(AsyncScanAcceptedResponse, Vec<DispatchedAsyncJob>), AsyncSubmitError>
    {
        let mut inner = self
            .inner
            .lock()
            .expect("serve async job lock should not be poisoned");
        if inner.non_terminal_jobs() >= self.max_non_terminal_jobs() {
            return Err(AsyncSubmitError::QueueFull);
        }

        let job_id = format!("job-{}", Uuid::new_v4().simple());
        inner.jobs.insert(job_id.clone(), AsyncJobRecord::pending());
        inner.pending.push_back(PendingAsyncJob {
            job_id: job_id.clone(),
            request,
        });
        let dispatches = self.schedule_locked(&mut inner);
        let snapshot = inner
            .status_snapshot(&job_id)
            .expect("submitted async job should be present");
        let response = AsyncScanAcceptedResponse {
            status: "accepted".to_string(),
            job_id: job_id.clone(),
            state: snapshot.state,
            status_url: format!("/v1/jobs/{job_id}"),
            result_url: format!("/v1/jobs/{job_id}/result"),
        };
        Ok((response, dispatches))
    }

    pub(super) fn status_snapshot(&self, job_id: &str) -> Option<AsyncJobSnapshot> {
        self.inner
            .lock()
            .expect("serve async job lock should not be poisoned")
            .status_snapshot(job_id)
    }

    pub(super) fn result_snapshot(&self, job_id: &str) -> Option<AsyncJobResultSnapshot> {
        self.inner
            .lock()
            .expect("serve async job lock should not be poisoned")
            .result_snapshot(job_id)
    }

    pub(super) fn complete_job(
        &self,
        job_id: String,
        allocated_processors: usize,
        outcome: JobOutcome,
    ) -> Vec<DispatchedAsyncJob> {
        let mut inner = self
            .inner
            .lock()
            .expect("serve async job lock should not be poisoned");
        inner.active_processors = inner.active_processors.saturating_sub(allocated_processors);

        let record = inner
            .jobs
            .get_mut(&job_id)
            .expect("running async job should have metadata");
        match outcome {
            JobOutcome::Succeeded { result_body } => {
                record.state = AsyncJobState::Succeeded;
                record.result_body = Some(result_body);
                record.error_message = None;
            }
            JobOutcome::Failed {
                message,
                status_code,
            } => {
                record.state = AsyncJobState::Failed;
                record.result_body = None;
                record.error_message = Some(message);
                record.error_status_code = Some(status_code);
            }
        }

        inner.completed.push_back(job_id);
        inner.evict_completed_jobs(self.max_retained_terminal_jobs);

        self.schedule_locked(&mut inner)
    }

    #[cfg(test)]
    pub(super) fn insert_job(&self, job_id: String, record: AsyncJobRecord) {
        self.inner
            .lock()
            .expect("serve async job lock should not be poisoned")
            .jobs
            .insert(job_id, record);
    }

    fn max_non_terminal_jobs(&self) -> usize {
        self.processor_budget.saturating_mul(4).max(4)
    }

    fn schedule_locked(&self, inner: &mut AsyncJobControllerState) -> Vec<DispatchedAsyncJob> {
        let mut dispatches = Vec::new();

        while !inner.pending.is_empty() {
            let available_processors = self
                .processor_budget
                .saturating_sub(inner.active_processors);
            if available_processors == 0 {
                break;
            }

            let allocated_processors = available_processors.min(self.max_processors_per_job).max(1);
            let PendingAsyncJob { job_id, request } = inner
                .pending
                .pop_front()
                .expect("pending async job should still exist");
            let record = inner
                .jobs
                .get_mut(&job_id)
                .expect("queued async job should have metadata");
            record.state = AsyncJobState::Running;
            record.allocated_processors = Some(allocated_processors);
            record.error_message = None;
            inner.active_processors += allocated_processors;
            dispatches.push(DispatchedAsyncJob {
                job_id,
                request,
                allocated_processors,
            });
        }

        dispatches
    }
}

impl AsyncJobControllerState {
    fn non_terminal_jobs(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| matches!(job.state, AsyncJobState::Pending | AsyncJobState::Running))
            .count()
    }

    fn status_snapshot(&self, job_id: &str) -> Option<AsyncJobSnapshot> {
        self.jobs.get(job_id).map(|job| AsyncJobSnapshot {
            job_id: job_id.to_string(),
            state: job.state,
            allocated_processors: job.allocated_processors,
            error_message: job.error_message.clone(),
        })
    }

    fn result_snapshot(&self, job_id: &str) -> Option<AsyncJobResultSnapshot> {
        self.jobs.get(job_id).map(|job| AsyncJobResultSnapshot {
            state: job.state,
            result_body: job.result_body.clone(),
            error_message: job.error_message.clone(),
            error_status_code: job.error_status_code,
        })
    }

    fn evict_completed_jobs(&mut self, max_retained_terminal_jobs: usize) {
        while self.completed.len() > max_retained_terminal_jobs {
            let Some(job_id) = self.completed.pop_front() else {
                break;
            };
            self.jobs.remove(&job_id);
        }
    }
}

impl AsyncJobRecord {
    pub(super) fn pending() -> Self {
        Self {
            state: AsyncJobState::Pending,
            allocated_processors: None,
            result_body: None,
            error_message: None,
            error_status_code: None,
        }
    }

    #[cfg(test)]
    pub(super) fn succeeded(result_body: String, allocated_processors: usize) -> Self {
        Self {
            state: AsyncJobState::Succeeded,
            allocated_processors: Some(allocated_processors),
            result_body: Some(result_body),
            error_message: None,
            error_status_code: None,
        }
    }

    #[cfg(test)]
    pub(super) fn failed(message: String, status_code: u16, allocated_processors: usize) -> Self {
        Self {
            state: AsyncJobState::Failed,
            allocated_processors: Some(allocated_processors),
            result_body: None,
            error_message: Some(message),
            error_status_code: Some(status_code),
        }
    }
}

impl AsyncJobSnapshot {
    pub(super) fn into_status_response(self) -> AsyncJobStatusResponse {
        AsyncJobStatusResponse {
            job_id: self.job_id,
            state: self.state,
            result_ready: matches!(self.state, AsyncJobState::Succeeded),
            allocated_processors: self.allocated_processors,
            message: self.error_message,
        }
    }
}

fn default_async_processor_budget() -> usize {
    let cpus = thread::available_parallelism().map_or(1, |count| count.get());
    if cpus > 1 { cpus - 1 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve_api::{SyncScanInput, SyncScanOptions};

    fn dummy_async_request() -> SyncScanRequest {
        SyncScanRequest {
            input: SyncScanInput::Paths {
                paths: vec!["/tmp".to_string()],
            },
            options: SyncScanOptions::default(),
        }
    }

    #[test]
    fn async_job_controller_rejects_submit_when_queue_is_full() {
        let controller = AsyncJobController::with_limits(1, 1, 8);
        {
            let mut inner = controller
                .inner
                .lock()
                .expect("test async job lock should not be poisoned");
            for index in 0..controller.max_non_terminal_jobs() {
                inner
                    .jobs
                    .insert(format!("job-{index}"), AsyncJobRecord::pending());
            }
        }

        let result = controller.submit(dummy_async_request());
        assert!(matches!(result, Err(AsyncSubmitError::QueueFull)));
    }

    #[test]
    fn scheduler_leaves_extra_jobs_pending_when_budget_is_exhausted() {
        let controller = AsyncJobController::with_limits(4, 2, 8);
        let mut inner = AsyncJobControllerState {
            active_processors: 0,
            jobs: HashMap::new(),
            pending: VecDeque::new(),
            completed: VecDeque::new(),
        };

        for job_id in ["job-a", "job-b", "job-c"] {
            inner
                .jobs
                .insert(job_id.to_string(), AsyncJobRecord::pending());
            inner.pending.push_back(PendingAsyncJob {
                job_id: job_id.to_string(),
                request: dummy_async_request(),
            });
        }

        let dispatches = controller.schedule_locked(&mut inner);
        assert_eq!(dispatches.len(), 2);
        assert_eq!(inner.active_processors, 4);
        assert_eq!(inner.pending.len(), 1);
        assert_eq!(inner.jobs["job-a"].state, AsyncJobState::Running);
        assert_eq!(inner.jobs["job-b"].state, AsyncJobState::Running);
        assert_eq!(inner.jobs["job-c"].state, AsyncJobState::Pending);
    }

    #[test]
    fn completed_jobs_are_evicted_when_retention_limit_is_exceeded() {
        let mut inner = AsyncJobControllerState {
            active_processors: 0,
            jobs: HashMap::from([
                (
                    "job-old".to_string(),
                    AsyncJobRecord::succeeded("old".to_string(), 1),
                ),
                (
                    "job-new".to_string(),
                    AsyncJobRecord::succeeded("new".to_string(), 1),
                ),
            ]),
            pending: VecDeque::new(),
            completed: VecDeque::from(["job-old".to_string(), "job-new".to_string()]),
        };

        inner.evict_completed_jobs(1);

        assert!(!inner.jobs.contains_key("job-old"));
        assert!(inner.jobs.contains_key("job-new"));
        assert_eq!(inner.completed.len(), 1);
    }
}
