// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use crate::license_detection::LicenseDetectionError;
use std::fmt;

#[derive(Debug, thiserror::Error)]
pub(crate) enum FileScanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Timeout(FileScanTimeout),
}

#[derive(Debug, Clone)]
pub(crate) struct FileScanTimeout {
    pub(crate) phase: TimeoutPhase,
    pub(crate) seconds: f64,
}

impl fmt::Display for FileScanTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.phase {
            TimeoutPhase::ReadingContent => {
                write!(
                    f,
                    "Timeout while reading file content (> {:.2}s)",
                    self.seconds
                )
            }
            TimeoutPhase::ExtractingMetadata => {
                write!(
                    f,
                    "Timeout while extracting package/text metadata (> {:.2}s)",
                    self.seconds
                )
            }
            TimeoutPhase::ExtractingText => {
                write!(
                    f,
                    "Timeout while extracting text content (> {:.2}s)",
                    self.seconds
                )
            }
            TimeoutPhase::BeforeLicenseScan => {
                write!(f, "Timeout before license scan (> {:.2}s)", self.seconds)
            }
            TimeoutPhase::DuringLicenseScan => {
                write!(f, "Timeout during license scan (> {:.2}s)", self.seconds)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimeoutPhase {
    ReadingContent,
    ExtractingMetadata,
    ExtractingText,
    BeforeLicenseScan,
    DuringLicenseScan,
}

impl FileScanError {
    pub(crate) fn from_license_detection_timeout(seconds: f64) -> Self {
        Self::Timeout(FileScanTimeout {
            phase: TimeoutPhase::DuringLicenseScan,
            seconds,
        })
    }
}

impl From<LicenseDetectionError> for FileScanError {
    fn from(err: LicenseDetectionError) -> Self {
        match err {
            LicenseDetectionError::Timeout => Self::from_license_detection_timeout(0.0),
        }
    }
}

pub(crate) fn is_timeout_diagnostic_message(message: &str) -> bool {
    message.starts_with("Timeout while ")
        || message.starts_with("Timeout before ")
        || message.starts_with("Timeout during ")
        || message.starts_with("Processing interrupted due to timeout")
}
