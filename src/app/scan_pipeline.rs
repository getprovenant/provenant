// SPDX-FileCopyrightText: nexB Inc. and others
// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0
// Derived from ScanCode Toolkit (Apache-2.0); modified. See NOTICE.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use regex::Regex;

use crate::app::request::{InputMode, ScanRequest};
use crate::app::scan_plan::ScanPlan;
use crate::app::scan_runtime::{
    NativeScanSelection, build_license_cache_config, build_paths_file_warning_messages,
    describe_license_engine_source, init_license_engine, prepare_cache_config,
    resolve_native_scan_selection,
};
use crate::assembly;
use crate::cache::{
    CacheConfig, IncrementalManifest, IncrementalManifestEntry, build_collection_exclude_patterns,
    incremental_manifest_path, load_incremental_manifest, manifest_entry_matches_path,
    metadata_fingerprint, write_incremental_manifest,
};
use crate::cli::ProcessMode;
use crate::license_detection::LicenseDetectionEngine;
use crate::license_detection::license_cache::LicenseCacheConfig;
use crate::models::{FileInfo, FileType, Output, Sha256Digest};
use crate::post_processing::{
    CreateOutputContext, CreateOutputOptions, DEFAULT_LICENSEDB_URL_TEMPLATE, FacetRule,
    apply_license_policy_from_file, apply_package_reference_following, build_facet_rules,
    collect_top_level_license_detections, collect_top_level_license_references, create_output,
};
use crate::progress::{ScanProgress, format_default_scan_error};
use crate::scan_result_shaping::{
    apply_cli_path_selection_filter, apply_ignore_resource_filter, apply_mark_source,
    apply_only_findings_filter, apply_user_path_filters_to_collected, filter_redundant_clues,
    filter_redundant_clues_with_rules, load_and_merge_json_inputs, normalize_paths,
    normalize_top_level_output_paths, populate_info_resource_counts,
    prepare_filter_clue_rule_lookup, trim_preloaded_assembly_to_files,
};
use crate::scanner::{
    ProcessResult, collect_paths, collect_selected_paths, process_collected_with_memory_limit,
    process_collected_with_memory_limit_sequential, scan_options_fingerprint,
};
use crate::utils::hash::calculate_sha256;

pub(crate) struct ScanSession {
    pub(crate) scan_result: ProcessResult,
    pub(crate) total_dirs: usize,
    pub(crate) preloaded_assembly: assembly::AssemblyResult,
    pub(crate) preloaded_license_detections: Vec<crate::models::TopLevelLicenseDetection>,
    pub(crate) preloaded_license_references: Vec<crate::models::LicenseReference>,
    pub(crate) preloaded_license_rule_references: Vec<crate::models::LicenseRuleReference>,
    pub(crate) extra_errors: Vec<String>,
    pub(crate) extra_warnings: Vec<String>,
    pub(crate) imported_spdx_license_list_version: Option<String>,
    pub(crate) imported_license_index_provenance: Option<crate::models::LicenseIndexProvenance>,
    pub(crate) active_license_engine: Option<Arc<LicenseDetectionEngine>>,
    pub(crate) shared_cache_config: Option<CacheConfig>,
    pub(crate) shared_license_cache_config: Option<LicenseCacheConfig>,
}

pub(crate) fn load_scan_session(
    request: &ScanRequest,
    scan_plan: &ScanPlan,
    progress: &Arc<ScanProgress>,
) -> Result<ScanSession> {
    let mut shared_license_cache_config: Option<LicenseCacheConfig> = None;

    progress.start_discovery();
    let shared_cache_config = if matches!(request.input_mode, InputMode::FromJson) {
        let cache_config = prepare_cache_config(None, request)?;
        shared_license_cache_config = Some(build_license_cache_config(&cache_config, request));
        Some(cache_config)
    } else {
        None
    };

    if matches!(request.input_mode, InputMode::FromJson) {
        let loaded = load_and_merge_json_inputs(
            &request.input_paths,
            request.strip_root,
            request.full_root,
        )?;
        let directories_count = loaded.directory_count();
        let files_count = loaded.file_count();
        let size_count = loaded.file_size_count();
        progress.finish_discovery(
            files_count,
            directories_count,
            size_count,
            loaded.excluded_count,
        );
        let (
            process_result,
            assembly_result,
            license_detections,
            license_references,
            license_rule_references,
            extra_errors,
            imported_spdx_license_list_version,
            imported_license_index_provenance,
        ) = loaded.into_parts()?;
        return Ok(ScanSession {
            scan_result: process_result,
            total_dirs: directories_count,
            preloaded_assembly: assembly_result,
            preloaded_license_detections: license_detections,
            preloaded_license_references: license_references,
            preloaded_license_rule_references: license_rule_references,
            extra_errors,
            extra_warnings: Vec::new(),
            imported_spdx_license_list_version,
            imported_license_index_provenance,
            active_license_engine: None,
            shared_cache_config,
            shared_license_cache_config,
        });
    }

    let session = load_native_scan_session(request, scan_plan, progress)?;
    Ok(session)
}

pub(crate) struct ExecutedRequest {
    pub(crate) output: Output,
    pub(crate) progress: Arc<ScanProgress>,
    pub(crate) start_time: DateTime<Utc>,
}

pub(crate) fn execute_request(request: &ScanRequest) -> Result<ExecutedRequest> {
    let start_time = Utc::now();
    let scan_plan = ScanPlan::from_request(request);

    let progress = Arc::new(ScanProgress::new(scan_plan.progress_mode));
    progress.set_processes(request.process_mode);
    progress.set_scan_names(scan_plan.scan_names.clone());
    progress.init_logging_bridge();
    progress.start_setup();
    let facet_rules = build_facet_rules(&request.facet)?;

    let ignore_author_patterns =
        compile_regex_patterns("ignore_author_patterns", &request.ignore_author)?;
    let ignore_holder_patterns = compile_regex_patterns(
        "ignore_copyright_holder_patterns",
        &request.ignore_copyright_holder,
    )?;
    progress.finish_setup();

    let session = load_scan_session(request, &scan_plan, &progress)?;
    let output = build_output_model(
        session,
        request,
        &progress,
        &facet_rules,
        &ignore_author_patterns,
        &ignore_holder_patterns,
        start_time,
    )?;

    Ok(ExecutedRequest {
        output,
        progress,
        start_time,
    })
}

pub(crate) fn build_output_model(
    mut session: ScanSession,
    request: &ScanRequest,
    progress: &Arc<ScanProgress>,
    facet_rules: &[FacetRule],
    ignore_author_patterns: &[Regex],
    ignore_copyright_holder_patterns: &[Regex],
    start_time: DateTime<Utc>,
) -> Result<Output> {
    progress.start_post_scan();

    if request.filter_clues {
        progress.post_scan_step("Filtering redundant clues...");
        let clue_rule_lookup = record_detail_timing(progress, "post-scan:filter-clues", || {
            prepare_filter_clue_rule_lookup(
                &session.scan_result.files,
                session.active_license_engine.as_deref(),
                request.license_dataset_path.as_deref(),
                session.shared_license_cache_config.as_ref(),
            )
        })?;
        if let Some(clue_rule_lookup) = clue_rule_lookup.as_ref() {
            filter_redundant_clues_with_rules(
                &mut session.scan_result.files,
                Some(clue_rule_lookup),
            );
        } else {
            filter_redundant_clues(&mut session.scan_result.files);
        }
    }

    if !ignore_author_patterns.is_empty() || !ignore_copyright_holder_patterns.is_empty() {
        progress.post_scan_step("Applying ignore-resource filters...");
        record_detail_timing(progress, "post-scan:ignore-resource", || {
            apply_ignore_resource_filter(
                &mut session.scan_result.files,
                ignore_copyright_holder_patterns,
                ignore_author_patterns,
            );
        });
    }

    if matches!(request.input_mode, InputMode::FromJson)
        && (!request.include.is_empty() || !request.exclude.is_empty())
    {
        progress.post_scan_step("Applying path selection filters...");
        record_detail_timing(progress, "output-filter:path-selection", || {
            apply_cli_path_selection_filter(
                &mut session.scan_result.files,
                &request.include,
                &request.exclude,
            );
        });
    }

    if request.only_findings {
        progress.post_scan_step("Filtering to resources with findings...");
        record_detail_timing(progress, "output-filter:only-findings", || {
            apply_only_findings_filter(&mut session.scan_result.files);
        });
    }

    if request.info && request.mark_source {
        progress.post_scan_step("Marking source files...");
        record_detail_timing(progress, "post-scan:mark-source", || {
            apply_mark_source(&mut session.scan_result.files);
        });
    }

    if should_include_info_surface(&session.scan_result.files, request) {
        progress.post_scan_step("Populating info resource counts...");
        record_detail_timing(progress, "post-scan:info-resource-counts", || {
            populate_info_resource_counts(&mut session.scan_result.files);
        });
    }

    progress.post_scan_step("Backfilling license provenance...");
    record_detail_timing(progress, "post-scan:license-provenance", || {
        for file in &mut session.scan_result.files {
            file.backfill_license_provenance();
        }
    });

    if matches!(request.input_mode, InputMode::FromJson) {
        for err in &session.extra_errors {
            progress.record_additional_error(err);
        }
    }

    let mut extra_errors = session.extra_errors;
    if let Some(policy_path) = request.license_policy.as_deref() {
        progress.post_scan_step("Applying license policy...");
        let license_policy_errors =
            record_detail_timing(progress, "post-scan:license-policy", || {
                apply_license_policy_from_file(
                    &mut session.scan_result.files,
                    Path::new(policy_path),
                )
            })?;
        for err in &license_policy_errors {
            progress.record_additional_error(err);
        }
        extra_errors.extend(license_policy_errors);
    }

    if matches!(request.input_mode, InputMode::FromJson) {
        progress.post_scan_step("Trimming preloaded assembly to filtered files...");
        record_detail_timing(progress, "post-scan:trim-preloaded-assembly", || {
            trim_preloaded_assembly_to_files(
                &session.scan_result.files,
                &mut session.preloaded_assembly.packages,
                &mut session.preloaded_assembly.dependencies,
            );
        });
    }

    progress.finish_post_scan();

    let manifests_seen = session
        .scan_result
        .files
        .iter()
        .map(|file| file.package_data.len())
        .sum();
    let skip_assembly = request.no_assemble || request.package_only;

    let mut assembly_result = if skip_assembly {
        assembly::AssemblyResult {
            packages: Vec::new(),
            dependencies: Vec::new(),
        }
    } else {
        progress.start_assembly();

        let mut result = if matches!(request.input_mode, InputMode::FromJson)
            && (!session.preloaded_assembly.packages.is_empty()
                || !session.preloaded_assembly.dependencies.is_empty())
        {
            progress.assembly_step("Using preloaded assembly...");
            session.preloaded_assembly
        } else {
            assembly::assemble(&mut session.scan_result.files)
        };

        progress.assembly_step("Backfilling package license provenance...");
        record_detail_timing(progress, "assembly:package-license-provenance", || {
            for package in &mut result.packages {
                package.backfill_license_provenance();
            }
        });

        progress.assembly_step("Applying package reference following...");
        record_detail_timing(progress, "assembly:package-reference-following", || {
            apply_package_reference_following(&mut session.scan_result.files, &mut result.packages);
        });

        progress.finish_assembly(result.packages.len(), manifests_seen);
        result
    };

    progress.start_finalize();

    if matches!(request.input_mode, InputMode::Native) && (request.strip_root || request.full_root)
    {
        let root_path = request
            .input_paths
            .first()
            .ok_or_else(|| anyhow!("No input path available for path normalization"))?;
        progress.finalize_step("Normalizing paths...");
        record_detail_timing(progress, "finalize:path-normalization", || {
            normalize_paths(
                &mut session.scan_result.files,
                root_path,
                request.strip_root,
                request.full_root,
            );
            normalize_top_level_output_paths(
                &mut assembly_result.packages,
                &mut assembly_result.dependencies,
                root_path,
                request.strip_root,
            );
        });
    }

    progress.finalize_step("Collecting license detections...");
    let license_detections = record_detail_timing(progress, "finalize:license-detections", || {
        let preserve_preloaded_top_level_detections =
            matches!(request.input_mode, InputMode::FromJson)
                && (request.only_findings
                    || !request.include.is_empty()
                    || !request.exclude.is_empty());
        collect_top_level_license_detections_for_mode(
            &session.scan_result.files,
            session.preloaded_license_detections,
            preserve_preloaded_top_level_detections,
            matches!(request.input_mode, InputMode::FromJson) && request.input_paths.len() > 1,
        )
    });

    let should_recompute_license_references = matches!(request.input_mode, InputMode::FromJson)
        && (!session.preloaded_license_references.is_empty()
            || !session.preloaded_license_rule_references.is_empty()
            || request.license_references
            || (request.license_url_template != DEFAULT_LICENSEDB_URL_TEMPLATE
                && !session.preloaded_license_references.is_empty()));

    if should_recompute_license_references && session.active_license_engine.is_none() {
        progress.start_license_detection_engine_creation();
        session.active_license_engine = Some(init_license_engine(
            session
                .shared_cache_config
                .as_ref()
                .expect("cache config should be prepared before license engine init"),
            request,
        )?);
        progress.finish_license_detection_engine_creation("finalize:license-engine-creation");
    }

    progress.finalize_step("Collecting license references...");
    let (license_references, license_rule_references) =
        record_detail_timing(progress, "finalize:license-references", || {
            if matches!(request.input_mode, InputMode::FromJson)
                && !should_recompute_license_references
            {
                (
                    session.preloaded_license_references,
                    session.preloaded_license_rule_references,
                )
            } else if request.license_references || should_recompute_license_references {
                if let Some(engine) = session.active_license_engine.as_deref() {
                    collect_top_level_license_references(
                        &session.scan_result.files,
                        &assembly_result.packages,
                        engine.index(),
                        &request.license_url_template,
                    )
                } else {
                    (Vec::new(), Vec::new())
                }
            } else {
                (Vec::new(), Vec::new())
            }
        });

    let end_time = Utc::now();
    let spdx_license_list_version = session
        .active_license_engine
        .as_ref()
        .and_then(|engine| engine.spdx_license_list_version().map(ToOwned::to_owned))
        .or(session.imported_spdx_license_list_version)
        .unwrap_or(LicenseDetectionEngine::embedded_spdx_license_list_version()?);
    let license_index_provenance = session
        .active_license_engine
        .as_ref()
        .and_then(|engine| engine.license_index_provenance().cloned())
        .or(session.imported_license_index_provenance);

    progress.finalize_step("Preparing output...");
    let output = record_detail_timing(progress, "finalize:output-prepare", || {
        create_output(
            start_time,
            end_time,
            session.scan_result,
            CreateOutputContext {
                total_dirs: session.total_dirs,
                assembly_result,
                license_detections,
                license_references,
                license_rule_references,
                spdx_license_list_version,
                license_index_provenance,
                extra_errors,
                extra_warnings: session.extra_warnings,
                header_options: request.output_header_options.clone(),
                options: CreateOutputOptions {
                    facet_rules,
                    include_classify: request.classify,
                    include_summary: request.summary,
                    include_license_clarity_score: request.license_clarity_score,
                    include_tallies: request.tallies,
                    include_tallies_of_key_files: request.tallies_key_files,
                    include_tallies_with_details: request.tallies_with_details,
                    include_tallies_by_facet: request.tallies_by_facet,
                    include_generated: request.generated,
                    verbose: request.verbose,
                },
            },
        )
    });
    progress.finish_finalize();

    Ok(output)
}

pub(crate) fn compile_regex_patterns(option_name: &str, patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|pattern| {
            Regex::new(pattern).map_err(|err| {
                anyhow!("Invalid regex for {option_name} pattern \"{pattern}\": {err}")
            })
        })
        .collect()
}

fn load_native_scan_session(
    request: &ScanRequest,
    scan_plan: &ScanPlan,
    progress: &Arc<ScanProgress>,
) -> Result<ScanSession> {
    let NativeScanSelection {
        scan_path,
        selected_paths,
        collection_frontier,
        missing_entries: missing_paths_file_entries,
    } = resolve_native_scan_selection(request)?;
    let paths_file_warnings = build_paths_file_warning_messages(&missing_paths_file_entries);
    for warning in &paths_file_warnings {
        progress.output_written(warning);
    }

    let cache_config = prepare_cache_config(Some(Path::new(&scan_path)), request)?;
    let shared_license_cache_config = Some(build_license_cache_config(&cache_config, request));
    let shared_cache_config = Some(cache_config.clone());
    let collection_exclude_patterns =
        build_collection_exclude_patterns(Path::new(&scan_path), cache_config.root_dir());

    let mut collected = if request.paths_files.is_empty() {
        collect_paths(&scan_path, request.max_depth, &collection_exclude_patterns)
    } else {
        collect_selected_paths(
            Path::new(&scan_path),
            &collection_frontier,
            request.max_depth,
            &collection_exclude_patterns,
        )
    };
    let user_excluded_count = apply_user_path_filters_to_collected(
        &mut collected,
        Path::new(&scan_path),
        &selected_paths,
        &request.include,
        &request.exclude,
    );
    let total_files = collected.file_count();
    let total_dirs = collected.directory_count();
    let total_size = collected.total_file_bytes;
    let excluded_count = collected.excluded_count + user_excluded_count;
    let all_collected_files = collected.files.clone();
    let ordered_file_paths: Vec<PathBuf> = collected
        .files
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    let runtime_errors = collected
        .collection_errors
        .iter()
        .map(|(path, err)| format_default_scan_error(path, err))
        .collect();
    for (path, err) in &collected.collection_errors {
        progress.record_runtime_error(path, err);
    }
    progress.finish_discovery(total_files, total_dirs, total_size, excluded_count);
    if !request.quiet {
        progress.output_written(&format!(
            "Found {} files in {} directories ({} items excluded)",
            total_files, total_dirs, excluded_count
        ));
    }

    let license_engine = if request.license {
        progress.start_setup();
        progress.start_license_detection_engine_creation();
        let engine = init_license_engine(
            shared_cache_config
                .as_ref()
                .expect("cache config should be prepared before license engine init"),
            request,
        )?;
        progress.finish_license_detection_engine_creation("setup_scan:licenses");
        progress.finish_setup();
        progress.output_written(&describe_license_engine_source(
            &engine,
            request.license_dataset_path.as_deref(),
        ));
        Some(engine)
    } else {
        None
    };

    let process_mode = request.process_mode;
    let text_options = scan_plan.text_options.clone();
    let license_options = scan_plan.license_options;
    let options_fingerprint =
        scan_options_fingerprint(&text_options, license_options, license_engine.as_deref());

    if request.incremental {
        let manifest_path = incremental_manifest_path(
            cache_config.root_dir(),
            &incremental_manifest_key(Path::new(&scan_path), &options_fingerprint),
        );
        let previous_manifest = load_incremental_manifest(&manifest_path, &options_fingerprint)?;
        let reused_files = partition_incremental_files(
            &mut collected.files,
            Path::new(&scan_path),
            previous_manifest.as_ref(),
        );
        progress.record_incremental_reused(reused_files.len());
    }

    if let Some(message) = process_mode_message(process_mode) {
        progress.output_written(message);
    }
    progress.start_scan(collected.file_count());
    let mut result = match process_mode {
        ProcessMode::Parallel(thread_count) => run_with_thread_pool(thread_count, || {
            Ok(process_collected_with_memory_limit(
                &collected,
                Arc::clone(progress),
                license_engine.clone(),
                license_options,
                &text_options,
                request.max_in_memory,
            ))
        })?,
        ProcessMode::SequentialWithTimeouts | ProcessMode::SequentialWithoutTimeouts => {
            process_collected_with_memory_limit_sequential(
                &collected,
                Arc::clone(progress),
                license_engine.clone(),
                license_options,
                &text_options,
                request.max_in_memory,
            )
        }
    };

    if request.incremental {
        let manifest_path = incremental_manifest_path(
            cache_config.root_dir(),
            &incremental_manifest_key(Path::new(&scan_path), &options_fingerprint),
        );
        let reused_files = partition_incremental_files(
            &mut all_collected_files.clone(),
            Path::new(&scan_path),
            load_incremental_manifest(&manifest_path, &options_fingerprint)?.as_ref(),
        );
        result.files =
            merge_incremental_file_results(result.files, reused_files, &ordered_file_paths);

        let manifest = build_incremental_manifest(
            Path::new(&scan_path),
            &all_collected_files,
            &result.files,
            &options_fingerprint,
        );
        write_incremental_manifest(cache_config.root_dir(), &manifest_path, &manifest)?;
    }

    result.excluded_count = excluded_count;
    progress.finish_scan();

    Ok(ScanSession {
        scan_result: result,
        total_dirs,
        preloaded_assembly: assembly::AssemblyResult {
            packages: Vec::new(),
            dependencies: Vec::new(),
        },
        preloaded_license_detections: Vec::new(),
        preloaded_license_references: Vec::new(),
        preloaded_license_rule_references: Vec::new(),
        extra_errors: runtime_errors,
        extra_warnings: paths_file_warnings,
        imported_spdx_license_list_version: None,
        imported_license_index_provenance: None,
        active_license_engine: license_engine,
        shared_cache_config,
        shared_license_cache_config,
    })
}

pub(crate) fn collect_top_level_license_detections_for_mode(
    files: &[FileInfo],
    preloaded: Vec<crate::models::TopLevelLicenseDetection>,
    preserve_preloaded: bool,
    clear_for_multi_input_replay: bool,
) -> Vec<crate::models::TopLevelLicenseDetection> {
    if clear_for_multi_input_replay {
        Vec::new()
    } else if preserve_preloaded {
        preloaded
    } else {
        collect_top_level_license_detections(files)
    }
}

fn process_mode_message(process_mode: ProcessMode) -> Option<&'static str> {
    match process_mode {
        ProcessMode::SequentialWithTimeouts => Some("Disabling multi-processing for debugging."),
        ProcessMode::SequentialWithoutTimeouts => {
            Some("Disabling multi-processing and multi-threading for debugging.")
        }
        ProcessMode::Parallel(_) => None,
    }
}

fn should_include_info_surface(files: &[crate::models::FileInfo], request: &ScanRequest) -> bool {
    request.info
        || files.iter().any(|file| {
            file.date.is_some()
                || file.sha1.is_some()
                || file.md5.is_some()
                || file.sha256.is_some()
                || file.sha1_git.is_some()
                || file.mime_type.is_some()
                || file.file_type_label.is_some()
                || file.programming_language.is_some()
                || file.is_binary.is_some()
                || file.is_text.is_some()
                || file.is_archive.is_some()
                || file.is_media.is_some()
                || file.is_source.is_some()
                || file.is_script.is_some()
                || file.files_count.is_some()
                || file.dirs_count.is_some()
                || file.size_count.is_some()
        })
}

fn record_detail_timing<T, F>(progress: &Arc<ScanProgress>, name: impl Into<String>, f: F) -> T
where
    F: FnOnce() -> T,
{
    let started = Instant::now();
    let result = f();
    progress.record_detail_timing(name.into(), started.elapsed().as_secs_f64());
    result
}

fn run_with_thread_pool<T, F>(threads: usize, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send,
    T: Send,
{
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .build()?;
    pool.install(f)
}

fn partition_incremental_files(
    collected_files: &mut Vec<(PathBuf, fs::Metadata)>,
    scan_root: &Path,
    manifest: Option<&IncrementalManifest>,
) -> Vec<FileInfo> {
    let Some(manifest) = manifest else {
        return Vec::new();
    };

    let mut files_to_scan = Vec::new();
    let mut reused_files = Vec::new();

    for (path, metadata) in collected_files.drain(..) {
        let relative_path = normalize_relative_scan_path(&path, scan_root);
        let Some(entry) = manifest.entry(&relative_path) else {
            files_to_scan.push((path, metadata));
            continue;
        };

        match manifest_entry_matches_path(entry, &path, &metadata) {
            Ok(true) => reused_files.push(entry.file_info.clone()),
            Ok(false) | Err(_) => files_to_scan.push((path, metadata)),
        }
    }

    *collected_files = files_to_scan;
    reused_files
}

fn merge_incremental_file_results(
    processed_files: Vec<FileInfo>,
    reused_files: Vec<FileInfo>,
    ordered_file_paths: &[PathBuf],
) -> Vec<FileInfo> {
    let mut processed_file_entries = std::collections::HashMap::new();
    let mut directory_entries = Vec::new();
    for file in processed_files {
        if file.file_type == FileType::File {
            processed_file_entries.insert(file.path.clone(), file);
        } else {
            directory_entries.push(file);
        }
    }

    let mut reused_file_entries: std::collections::HashMap<_, _> = reused_files
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect();

    let mut merged_files = Vec::new();
    for path in ordered_file_paths {
        let path_string = path.to_string_lossy().to_string();
        if let Some(file) = processed_file_entries.remove(&path_string) {
            merged_files.push(file);
            continue;
        }

        if let Some(file) = reused_file_entries.remove(&path_string) {
            merged_files.push(file);
        }
    }

    merged_files.extend(processed_file_entries.into_values());
    merged_files.extend(reused_file_entries.into_values());
    merged_files.extend(directory_entries);
    merged_files
}

fn build_incremental_manifest(
    scan_root: &Path,
    collected_files: &[(PathBuf, fs::Metadata)],
    files: &[FileInfo],
    options_fingerprint: &str,
) -> IncrementalManifest {
    let files_by_relative_path: std::collections::HashMap<_, _> = files
        .iter()
        .filter(|file| file.file_type == FileType::File)
        .map(|file| {
            (
                normalize_relative_scan_path(Path::new(&file.path), scan_root),
                file.clone(),
            )
        })
        .collect();

    let entries = collected_files
        .iter()
        .filter_map(|(path, metadata)| {
            let relative_path = normalize_relative_scan_path(path, scan_root);
            let state = metadata_fingerprint(metadata)?;
            let file_info = files_by_relative_path.get(&relative_path)?.clone();
            let content_sha256 = file_info.sha256.unwrap_or_else(|| {
                fs::read(path)
                    .map(|bytes| calculate_sha256(&bytes))
                    .unwrap_or_else(|_| {
                        Sha256Digest::from_hex(
                            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                        )
                        .unwrap()
                    })
            });
            Some((
                relative_path,
                IncrementalManifestEntry {
                    state,
                    content_sha256,
                    file_info,
                },
            ))
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    IncrementalManifest::new(options_fingerprint.to_string(), entries)
}

fn incremental_manifest_key(scan_root: &Path, options_fingerprint: &str) -> String {
    let canonical_root = fs::canonicalize(scan_root).unwrap_or_else(|_| scan_root.to_path_buf());
    calculate_sha256(
        format!(
            "{}\n{options_fingerprint}",
            canonical_root.to_string_lossy()
        )
        .as_bytes(),
    )
    .as_hex()
}

fn normalize_relative_scan_path(path: &Path, scan_root: &Path) -> String {
    path.strip_prefix(scan_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
