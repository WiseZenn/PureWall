#[cfg_attr(not(test), allow(dead_code))]
mod compare;
#[cfg_attr(not(test), allow(dead_code))]
mod dataset;
#[cfg_attr(not(test), allow(dead_code))]
mod metrics;
#[cfg_attr(not(test), allow(dead_code))]
mod owned_temp;
#[cfg_attr(not(test), allow(dead_code))]
mod protocol;
#[cfg_attr(not(test), allow(dead_code))]
mod report;
#[cfg_attr(not(test), allow(dead_code))]
mod scenarios;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};

use self::{
    compare::{compare_reports, Verdict},
    dataset::{
        generate_dataset_with_explicit_stress_opt_in, DatasetScale, GeneratedDataset,
        DATASET_SCHEMA_VERSION, DATASET_SEED,
    },
    metrics::{collect_run_metadata, RunMetadata},
    owned_temp::{CleanupFailure, CleanupFailureState, OwnedRunRoot},
    protocol::{
        BenchmarkReport, CacheState, DatasetFingerprint, RunStatus, ScenarioRecord,
        ScenarioSamples, REPORT_SCHEMA_VERSION, SCENARIO_CONTRACT_VERSION,
    },
    scenarios::{
        run_library_scenarios, run_playback_and_media_scenarios, SamplePolicy, ScenarioFailureKind,
        ScenarioRunFailure,
    },
};

static RUN_ID_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
enum HarnessCommand {
    Run {
        scale: DatasetScale,
        report: PathBuf,
        keep_data: bool,
        allow_stress: bool,
    },
    Compare {
        baseline: PathBuf,
        candidate: PathBuf,
        output: PathBuf,
    },
}

struct CompletedRun {
    metadata: RunMetadata,
    dataset: DatasetFingerprint,
    scenarios: Vec<ScenarioRecord>,
}

fn parse_command(args: &[String]) -> Result<HarnessCommand> {
    let command = args
        .first()
        .context("performance harness command is required")?;
    match command.as_str() {
        "run" => parse_run_command(&args[1..]),
        "compare" => parse_compare_command(&args[1..]),
        other => bail!("unknown performance harness command: {other}"),
    }
}

fn parse_run_command(args: &[String]) -> Result<HarnessCommand> {
    let mut scale = None;
    let mut report = None;
    let mut keep_data = false;
    let mut allow_stress = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--scale" => {
                if scale.is_some() {
                    bail!("duplicate --scale option");
                }
                index += 1;
                let value = args.get(index).context("--scale requires a value")?;
                scale = Some(match value.as_str() {
                    "ci" => DatasetScale::Ci,
                    "standard" => DatasetScale::Standard,
                    "stress" => DatasetScale::Stress,
                    _ => bail!("unsupported performance scale: {value}"),
                });
            }
            "--report" => {
                if report.is_some() {
                    bail!("duplicate --report option");
                }
                index += 1;
                report = Some(parse_absolute_json_path(
                    args.get(index).context("--report requires a value")?,
                    "report",
                )?);
            }
            "--keep-data" => {
                if keep_data {
                    bail!("duplicate --keep-data option");
                }
                keep_data = true;
            }
            "--allow-stress" => {
                if allow_stress {
                    bail!("duplicate --allow-stress option");
                }
                allow_stress = true;
            }
            option => bail!("unknown run option: {option}"),
        }
        index += 1;
    }
    let scale = scale.context("run requires --scale")?;
    if scale.requires_explicit_opt_in() != allow_stress {
        if scale.requires_explicit_opt_in() {
            bail!("stress scale requires --allow-stress");
        }
        bail!("--allow-stress is valid only with stress scale");
    }
    Ok(HarnessCommand::Run {
        scale,
        report: report.context("run requires --report")?,
        keep_data,
        allow_stress,
    })
}

fn parse_compare_command(args: &[String]) -> Result<HarnessCommand> {
    let mut baseline = None;
    let mut candidate = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        let (slot, label) = match args[index].as_str() {
            "--baseline" => (&mut baseline, "baseline"),
            "--candidate" => (&mut candidate, "candidate"),
            "--output" => (&mut output, "output"),
            option => bail!("unknown compare option: {option}"),
        };
        if slot.is_some() {
            bail!("duplicate --{label} option");
        }
        index += 1;
        *slot = Some(parse_absolute_json_path(
            args.get(index)
                .with_context(|| format!("--{label} requires a value"))?,
            label,
        )?);
        index += 1;
    }
    Ok(HarnessCommand::Compare {
        baseline: baseline.context("compare requires --baseline")?,
        candidate: candidate.context("compare requires --candidate")?,
        output: output.context("compare requires --output")?,
    })
}

fn parse_absolute_json_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if !path.is_absolute() {
        bail!("{label} path must be absolute");
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
        bail!("{label} path must use the .json extension");
    }
    Ok(path.to_path_buf())
}

fn failed_pre_generation_report(
    metadata: RunMetadata,
    scale: DatasetScale,
    error: String,
) -> BenchmarkReport {
    let (item_count, source_count) = scale.layout();
    BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scenario_contract_version: SCENARIO_CONTRACT_VERSION,
        git_commit: metadata.git_commit,
        generated_at_utc: metadata.generated_at_utc,
        environment: metadata.environment,
        dataset: DatasetFingerprint {
            schema_version: DATASET_SCHEMA_VERSION,
            seed: DATASET_SEED,
            item_count,
            source_count,
            manifest_digest: None,
        },
        scenarios: Vec::new(),
        run_status: RunStatus::Failed,
        errors: vec![error],
    }
}

fn failed_scenario_report(
    metadata: RunMetadata,
    dataset: DatasetFingerprint,
    failure: ScenarioRunFailure,
) -> BenchmarkReport {
    let error = format!("{:#}", failure.error);
    let scenario_id = failure.scenario_id;
    let timeout_count = u64::from(matches!(failure.kind, ScenarioFailureKind::Timeout { .. }));
    let mut scenarios = failure.completed_records;
    scenarios.push(ScenarioRecord {
        id: scenario_id.clone(),
        critical: true,
        selected_hotspot: false,
        cache_state: failed_scenario_cache_state(&scenario_id),
        status: RunStatus::Failed,
        samples: ScenarioSamples {
            elapsed_micros: Vec::new(),
            median_micros: 0,
            p95_micros: 0,
            peak_working_set_bytes: 0,
        },
        expected_count: None,
        actual_count: None,
        database_bytes: None,
        wal_bytes: None,
        cache_hits: None,
        cache_misses: None,
        generated_derivatives: None,
        queue_peak_pending: None,
        timeout_count,
        errors: vec![error.clone()],
    });
    BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scenario_contract_version: SCENARIO_CONTRACT_VERSION,
        git_commit: metadata.git_commit,
        generated_at_utc: metadata.generated_at_utc,
        environment: metadata.environment,
        dataset,
        scenarios,
        run_status: RunStatus::Failed,
        errors: vec![format!(
            "performance scenario {scenario_id} failed: {error}"
        )],
    }
}

fn failed_scenario_cache_state(scenario_id: &str) -> CacheState {
    match scenario_id {
        "media.thumbnail.cold" | "media.preview.cold" => CacheState::ColdDerivative,
        "media.thumbnail.warm" => CacheState::WarmDerivative,
        _ => CacheState::NotApplicable,
    }
}

fn failed_orchestration_report(
    metadata: RunMetadata,
    dataset: DatasetFingerprint,
    completed_records: Vec<ScenarioRecord>,
    operation: &str,
    error: &anyhow::Error,
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scenario_contract_version: SCENARIO_CONTRACT_VERSION,
        git_commit: metadata.git_commit,
        generated_at_utc: metadata.generated_at_utc,
        environment: metadata.environment,
        dataset,
        scenarios: completed_records,
        run_status: RunStatus::Failed,
        errors: vec![format!(
            "performance orchestration {operation} failed: {error:#}"
        )],
    }
}

fn execute_compare(baseline: &Path, candidate: &Path, output: &Path) -> Result<()> {
    let baseline = report::read_benchmark_report(baseline)?;
    let candidate = report::read_benchmark_report(candidate)?;
    let comparison = compare_reports(&baseline, &candidate);
    let overall = comparison.overall;
    report::write_comparison_report(output, &comparison)?;
    match overall {
        Verdict::Improved | Verdict::Stable => Ok(()),
        Verdict::Regressed | Verdict::Failed | Verdict::NotComparable => {
            bail!("performance comparison ended with verdict {overall:?}")
        }
    }
}

pub(crate) fn run_if_requested(args: &[String]) -> Option<anyhow::Result<()>> {
    let marker = args.iter().position(|arg| arg == "--performance-harness")?;
    let command_args = &args[marker + 1..];
    let command = command_args.first().map(String::as_str).unwrap_or("");
    Some(match command {
        "self-test-entry" => {
            println!("PureWall performance harness entry OK");
            Ok(())
        }
        _ => parse_command(command_args).and_then(execute_command),
    })
}

fn execute_command(command: HarnessCommand) -> Result<()> {
    match command {
        HarnessCommand::Run {
            scale,
            report,
            keep_data,
            allow_stress,
        } => execute_run(scale, &report, keep_data, allow_stress),
        HarnessCommand::Compare {
            baseline,
            candidate,
            output,
        } => execute_compare(&baseline, &candidate, &output),
    }
}

fn execute_run(
    scale: DatasetScale,
    report_path: &Path,
    keep_data: bool,
    allow_stress: bool,
) -> Result<()> {
    let sequence = RUN_ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let run_id = format!(
        "run-{}-{}-{sequence}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    );
    execute_run_with_id(scale, report_path, keep_data, allow_stress, &run_id)
}

fn execute_run_with_id(
    scale: DatasetScale,
    report_path: &Path,
    keep_data: bool,
    allow_stress: bool,
    run_id: &str,
) -> Result<()> {
    let policy = sample_policy(scale);
    let (metadata, run) = initialize_run(
        scale,
        report_path,
        run_id,
        || collect_run_metadata(policy),
        OwnedRunRoot::create,
    )?;
    if let Err(error) = validate_report_outside_owned_root(report_path, &run) {
        return fail_owned_run_before_generation(
            report_path,
            metadata,
            scale,
            run,
            "report target validation",
            error,
        );
    }

    let dataset =
        match generate_dataset_with_explicit_stress_opt_in(&run, scale, DATASET_SEED, allow_stress)
        {
            Ok(dataset) => dataset,
            Err(error) => {
                let report = failed_pre_generation_report(
                    metadata,
                    scale,
                    format!("performance dataset generation failed: {error:#}"),
                );
                return persist_failed_report_and_retain(report_path, report, run, error);
            }
        };
    let dataset_fingerprint = dataset_fingerprint(&dataset);

    let library_records = match run_library_scenarios(&dataset, &run, policy) {
        Ok(records) => records,
        Err(failure) => {
            let original = anyhow::anyhow!("{:#}", failure.error);
            let report = failed_scenario_report(metadata, dataset_fingerprint, failure);
            return persist_failed_report_and_retain(report_path, report, run, original);
        }
    };
    if let Err(error) = dataset
        .restore_baseline()
        .context("restore performance dataset baseline after library scenarios")
    {
        let report = failed_orchestration_report(
            metadata,
            dataset_fingerprint,
            library_records,
            "restore dataset baseline",
            &error,
        );
        return persist_failed_report_and_retain(report_path, report, run, error);
    }
    let media_records = match run_playback_and_media_scenarios(&dataset, &run, policy) {
        Ok(records) => records,
        Err(mut failure) => {
            let original = anyhow::anyhow!("{:#}", failure.error);
            let mut completed = library_records;
            completed.append(&mut failure.completed_records);
            failure.completed_records = completed;
            let report = failed_scenario_report(metadata, dataset_fingerprint, failure);
            return persist_failed_report_and_retain(report_path, report, run, original);
        }
    };

    let mut scenarios = library_records;
    scenarios.extend(media_records);
    finish_run_with_cleanup(
        report_path,
        CompletedRun {
            metadata,
            dataset: dataset_fingerprint,
            scenarios,
        },
        run,
        keep_data,
        OwnedRunRoot::cleanup,
        OwnedRunRoot::retain,
    )
}

fn finish_run_with_cleanup<Cleanup, Finish>(
    report_path: &Path,
    completed: CompletedRun,
    run: OwnedRunRoot,
    keep_data: bool,
    cleanup: Cleanup,
    finish: Finish,
) -> Result<()>
where
    Cleanup: FnOnce(OwnedRunRoot) -> std::result::Result<(), CleanupFailure>,
    Finish: FnOnce(OwnedRunRoot) -> PathBuf,
{
    let CompletedRun {
        metadata,
        dataset,
        scenarios,
    } = completed;
    let passed_report = BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scenario_contract_version: SCENARIO_CONTRACT_VERSION,
        git_commit: metadata.git_commit.clone(),
        generated_at_utc: metadata.generated_at_utc.clone(),
        environment: metadata.environment.clone(),
        dataset: dataset.clone(),
        scenarios: scenarios.clone(),
        run_status: RunStatus::Passed,
        errors: Vec::new(),
    };
    if let Err(error) = report::write_benchmark_report(report_path, &passed_report) {
        let failed_report = failed_orchestration_report(
            metadata,
            dataset,
            scenarios,
            "write passed report",
            &error,
        );
        return persist_failed_report_and_finish(report_path, failed_report, run, error, finish);
    }
    if keep_data {
        let retained = finish(run);
        println!(
            "PureWall performance data retained at {}",
            retained.display()
        );
        return Ok(());
    }

    match cleanup(run) {
        Ok(()) => Ok(()),
        Err(failure) => persist_cleanup_failure_report_and_finish(
            report_path,
            metadata,
            dataset,
            scenarios,
            failure,
            finish,
        ),
    }
}

fn persist_cleanup_failure_report_and_finish<Finish>(
    report_path: &Path,
    metadata: RunMetadata,
    dataset: DatasetFingerprint,
    scenarios: Vec<ScenarioRecord>,
    failure: CleanupFailure,
    finish: Finish,
) -> Result<()>
where
    Finish: FnOnce(OwnedRunRoot) -> PathBuf,
{
    let (run, original, state) = failure.into_parts();
    let original = original.context("cleanup owned performance run root");
    let failed_report = failed_orchestration_report(
        metadata,
        dataset,
        scenarios,
        "cleanup owned run root",
        &original,
    );
    let write_result = report::write_benchmark_report(report_path, &failed_report);
    let retained = finish(run);
    let retained_message = match state {
        CleanupFailureState::BeforeRemoval => {
            format!("performance data retained intact at {}", retained.display())
        }
        CleanupFailureState::RemovalMayBePartial => format!(
            "remaining performance data may be partial at {}",
            retained.display()
        ),
    };
    match write_result {
        Ok(()) => Err(original.context(retained_message)),
        Err(write_error) => Err(original.context(format!(
            "write failed performance report: {write_error:#}; {retained_message}"
        ))),
    }
}

fn initialize_run<MetadataCollector, RootFactory>(
    scale: DatasetScale,
    report_path: &Path,
    run_id: &str,
    mut collect_metadata: MetadataCollector,
    create_root: RootFactory,
) -> Result<(RunMetadata, OwnedRunRoot)>
where
    MetadataCollector: FnMut() -> Result<RunMetadata>,
    RootFactory: FnOnce(&str) -> Result<OwnedRunRoot>,
{
    report::preflight_benchmark_report_target(report_path).with_context(|| {
        format!(
            "cannot write performance report {}; owned data root was not created",
            report_path.display()
        )
    })?;

    let metadata = match collect_metadata() {
        Ok(metadata) => metadata,
        Err(original) => {
            let report_metadata = match collect_metadata() {
                Ok(metadata) => metadata,
                Err(report_metadata_error) => {
                    return Err(original.context(format!(
                        "cannot write failed performance report because truthful metadata collection also failed: {report_metadata_error:#}; owned data root was not created"
                    )));
                }
            };
            let report = failed_pre_generation_report(
                report_metadata,
                scale,
                format!("performance metadata collection failed: {original:#}"),
            );
            return persist_failed_report_before_root(report_path, report, original);
        }
    };

    match create_root(run_id) {
        Ok(run) => Ok((metadata, run)),
        Err(original) => {
            let report = failed_pre_generation_report(
                metadata,
                scale,
                format!("performance run-root preflight failed: {original:#}"),
            );
            persist_failed_report_before_root(report_path, report, original)
        }
    }
}

fn persist_failed_report_before_root<T>(
    report_path: &Path,
    report: BenchmarkReport,
    original: anyhow::Error,
) -> Result<T> {
    match report::write_benchmark_report(report_path, &report) {
        Ok(()) => Err(original.context(format!(
            "failed performance report written to {}; owned data root was not created",
            report_path.display()
        ))),
        Err(write_error) => Err(original.context(format!(
            "cannot write failed performance report {}: {write_error:#}; owned data root was not created",
            report_path.display()
        ))),
    }
}

fn fail_owned_run_before_generation(
    report_path: &Path,
    metadata: RunMetadata,
    scale: DatasetScale,
    run: OwnedRunRoot,
    operation: &str,
    original: anyhow::Error,
) -> Result<()> {
    fail_owned_run_before_generation_with(
        report_path,
        metadata,
        scale,
        run,
        operation,
        original,
        OwnedRunRoot::retain,
    )
}

fn fail_owned_run_before_generation_with<Finish>(
    report_path: &Path,
    metadata: RunMetadata,
    scale: DatasetScale,
    run: OwnedRunRoot,
    operation: &str,
    original: anyhow::Error,
    finish: Finish,
) -> Result<()>
where
    Finish: FnOnce(OwnedRunRoot) -> PathBuf,
{
    let report = failed_pre_generation_report(
        metadata,
        scale,
        format!("performance {operation} failed: {original:#}"),
    );
    persist_failed_report_and_finish(report_path, report, run, original, finish)
}

fn sample_policy(scale: DatasetScale) -> SamplePolicy {
    match scale {
        DatasetScale::Ci => SamplePolicy::ci(),
        DatasetScale::Standard | DatasetScale::Stress => SamplePolicy::standard(),
    }
}

fn dataset_fingerprint(dataset: &GeneratedDataset) -> DatasetFingerprint {
    DatasetFingerprint {
        schema_version: dataset.manifest.schema_version,
        seed: dataset.manifest.seed,
        item_count: dataset.manifest.item_count,
        source_count: dataset.manifest.source_count,
        manifest_digest: Some(dataset.manifest.logical_digest.clone()),
    }
}

fn validate_report_outside_owned_root(report_path: &Path, run: &OwnedRunRoot) -> Result<()> {
    let parent = report_path
        .parent()
        .context("performance report path has no parent")?;
    let canonical_parent = std::fs::canonicalize(parent).with_context(|| {
        format!(
            "canonicalize performance report parent {}",
            parent.display()
        )
    })?;
    if canonical_parent.starts_with(run.root()) || run.root().starts_with(&canonical_parent) {
        bail!("performance report must be outside the owned temporary data root");
    }
    Ok(())
}

fn persist_failed_report_and_retain(
    report_path: &Path,
    report: BenchmarkReport,
    run: OwnedRunRoot,
    original: anyhow::Error,
) -> Result<()> {
    persist_failed_report_and_finish(report_path, report, run, original, OwnedRunRoot::retain)
}

fn persist_failed_report_and_finish<Finish>(
    report_path: &Path,
    report: BenchmarkReport,
    run: OwnedRunRoot,
    original: anyhow::Error,
    finish: Finish,
) -> Result<()>
where
    Finish: FnOnce(OwnedRunRoot) -> PathBuf,
{
    let write_result = report::write_benchmark_report(report_path, &report);
    let retained = finish(run);
    match write_result {
        Ok(()) => Err(original.context(format!(
            "performance data retained at {}",
            retained.display()
        ))),
        Err(write_error) => Err(original.context(format!(
            "write failed performance report: {write_error:#}; performance data retained at {}",
            retained.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use crate::performance_harness::{
        metrics::RunMetadata,
        protocol::{protocol_test_report, DatasetFingerprint, EnvironmentFingerprint, RunStatus},
        scenarios::{ScenarioFailureKind, ScenarioRunFailure},
    };

    static RUNNER_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_runner_test_directory(label: &str) -> PathBuf {
        let sequence = RUNNER_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir()
            .join("purewall-performance-runner-tests")
            .join(format!("{label}-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("create runner test directory");
        directory
    }

    fn test_run_metadata() -> RunMetadata {
        RunMetadata {
            git_commit: "1".repeat(40),
            generated_at_utc: "2026-08-22T00:00:00Z".into(),
            environment: EnvironmentFingerprint {
                os: "windows".into(),
                arch: "x86_64".into(),
                cpu: "test-cpu".into(),
                logical_cores: 8,
                installed_memory_bytes: 16 * 1024 * 1024 * 1024,
                rust_profile: "release".into(),
                config_digest: "config-v1".into(),
            },
        }
    }

    #[test]
    fn ordinary_arguments_do_not_enter_the_harness() {
        let args = vec!["purewall.exe".into(), "--action".into(), "next".into()];
        assert!(run_if_requested(&args).is_none());
    }

    #[test]
    fn explicit_marker_runs_entry_self_test() {
        let args = vec![
            "purewall.exe".into(),
            "--performance-harness".into(),
            "self-test-entry".into(),
        ];
        assert!(run_if_requested(&args).expect("marker consumed").is_ok());
    }

    #[test]
    fn unknown_command_is_controlled() {
        let args = vec![
            "purewall.exe".into(),
            "--performance-harness".into(),
            "unknown".into(),
        ];
        let error = run_if_requested(&args)
            .expect("marker consumed")
            .expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "unknown performance harness command: unknown"
        );
    }

    #[test]
    fn parser_requires_absolute_reports_and_explicit_stress() {
        let relative =
            ["run", "--scale", "standard", "--report", "relative.json"].map(str::to_string);
        let unsafe_stress = [
            "run",
            "--scale",
            "stress",
            "--report",
            "D:\\reports\\stress.json",
        ]
        .map(str::to_string);
        let explicit_stress = [
            "run",
            "--scale",
            "stress",
            "--allow-stress",
            "--report",
            "D:\\reports\\stress.json",
        ]
        .map(str::to_string);
        assert!(parse_command(&relative).is_err());
        assert!(parse_command(&unsafe_stress).is_err());
        assert!(parse_command(&explicit_stress).is_ok());
    }

    #[test]
    fn parser_rejects_duplicate_unknown_and_incomplete_options() {
        for arguments in [
            vec![
                "run",
                "--scale",
                "ci",
                "--scale",
                "ci",
                "--report",
                "D:\\reports\\ci.json",
            ],
            vec![
                "run",
                "--scale",
                "ci",
                "--report",
                "D:\\reports\\ci.json",
                "--unknown",
            ],
            vec!["run", "--scale", "ci"],
            vec![
                "compare",
                "--baseline",
                "D:\\reports\\base.json",
                "--candidate",
                "D:\\reports\\candidate.json",
            ],
            vec![
                "compare",
                "--baseline",
                "relative.json",
                "--candidate",
                "D:\\reports\\candidate.json",
                "--output",
                "D:\\reports\\comparison.json",
            ],
        ] {
            let arguments = arguments
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            assert!(parse_command(&arguments).is_err());
        }
    }

    #[test]
    fn pre_generation_failure_report_has_requested_fields_without_samples() {
        let report = failed_pre_generation_report(
            test_run_metadata(),
            DatasetScale::Ci,
            "dataset preflight failed".into(),
        );
        assert_eq!(report.run_status, RunStatus::Failed);
        assert_eq!(
            report.dataset,
            DatasetFingerprint {
                schema_version: 1,
                seed: dataset::DATASET_SEED,
                item_count: 120,
                source_count: 2,
                manifest_digest: None,
            }
        );
        assert!(report.scenarios.is_empty());
        assert_eq!(report.errors, vec!["dataset preflight failed"]);
    }

    #[test]
    fn invalid_report_target_is_rejected_before_metadata_or_root_creation() {
        let directory = unique_runner_test_directory("invalid-report-target");
        let invalid_target = directory.join("target.json");
        std::fs::create_dir(&invalid_target).unwrap();
        let metadata_calls = Cell::new(0);
        let root_calls = Cell::new(0);

        let error = initialize_run(
            DatasetScale::Ci,
            &invalid_target,
            "invalid-report-target",
            || {
                metadata_calls.set(metadata_calls.get() + 1);
                Ok(test_run_metadata())
            },
            |_| {
                root_calls.set(root_calls.get() + 1);
                bail!("root creation must not be reached")
            },
        )
        .expect_err("directory report target must fail before creating a root");

        assert!(format!("{error:#}").contains("report target"));
        assert_eq!(metadata_calls.get(), 0);
        assert_eq!(root_calls.get(), 0);
        assert!(!std::env::temp_dir()
            .join("purewall-performance")
            .join("invalid-report-target")
            .exists());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn metadata_failure_writes_failed_report_without_creating_a_root() {
        let directory = unique_runner_test_directory("metadata-failure");
        let report_path = directory.join("failed.json");
        let metadata_calls = Cell::new(0);
        let root_calls = Cell::new(0);

        let error = initialize_run(
            DatasetScale::Ci,
            &report_path,
            "metadata-failure",
            || {
                let call = metadata_calls.get();
                metadata_calls.set(call + 1);
                if call == 0 {
                    bail!("injected primary metadata failure")
                }
                Ok(test_run_metadata())
            },
            |_| {
                root_calls.set(root_calls.get() + 1);
                bail!("root creation must not be reached")
            },
        )
        .expect_err("metadata collection must stop initialization");

        assert!(format!("{error:#}").contains("injected primary metadata failure"));
        assert_eq!(metadata_calls.get(), 2);
        assert_eq!(root_calls.get(), 0);
        let failed = report::read_benchmark_report(&report_path).unwrap();
        assert_eq!(failed.run_status, RunStatus::Failed);
        assert!(failed.scenarios.is_empty());
        assert!(failed.dataset.manifest_digest.is_none());
        assert!(failed.errors[0].contains("metadata collection failed"));
        assert!(failed.errors[0].contains("injected primary metadata failure"));
        assert!(report_path.with_extension("md").is_file());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn owned_root_overlap_writes_failed_report_and_preserves_original_error() {
        let sequence = RUNNER_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let run_id = format!("task7-overlap-{}-{sequence}", std::process::id());
        let run = OwnedRunRoot::create_for_test(&run_id).unwrap();
        let root_path = run.root().to_path_buf();
        let report_path = root_path
            .parent()
            .unwrap()
            .join(format!("{run_id}-overlap.json"));
        report::preflight_benchmark_report_target(&report_path).unwrap();
        let original = validate_report_outside_owned_root(&report_path, &run)
            .expect_err("owned root ancestor must overlap");

        let error = fail_owned_run_before_generation_with(
            &report_path,
            test_run_metadata(),
            DatasetScale::Ci,
            run,
            "report target validation",
            original,
            |run| {
                let path = run.root().to_path_buf();
                run.cleanup().unwrap();
                path
            },
        )
        .expect_err("overlap must stop the run");

        let error_chain = format!("{error:#}");
        assert!(error_chain.contains("outside the owned temporary data root"));
        assert!(error_chain.contains("performance data retained at"));
        let failed = report::read_benchmark_report(&report_path).unwrap();
        assert_eq!(failed.run_status, RunStatus::Failed);
        assert!(failed.scenarios.is_empty());
        assert!(failed.dataset.manifest_digest.is_none());
        assert!(failed.errors[0].contains("report target validation"));
        assert!(failed.errors[0].contains("outside the owned temporary data root"));
        assert!(!root_path.exists());

        std::fs::remove_file(report_path.with_extension("md")).unwrap();
        std::fs::remove_file(report_path).unwrap();
    }

    #[test]
    fn scenario_failure_preserves_completed_records_and_marks_only_timeout() {
        let digest = "e".repeat(64);
        let completed = protocol_test_report(&digest, 100, 100, 1_000).scenarios;
        let dataset = DatasetFingerprint {
            schema_version: 1,
            seed: dataset::DATASET_SEED,
            item_count: 120,
            source_count: 2,
            manifest_digest: Some(digest),
        };
        for (kind, expected_timeout) in [
            (
                ScenarioFailureKind::Timeout {
                    timeout: Duration::from_millis(25),
                },
                1,
            ),
            (ScenarioFailureKind::Error, 0),
        ] {
            let failure = ScenarioRunFailure {
                scenario_id: "query.page.deep".into(),
                kind,
                error: anyhow::anyhow!("low-level failure").context("scenario operation failed"),
                completed_records: completed.clone(),
            };
            let report = failed_scenario_report(test_run_metadata(), dataset.clone(), failure);
            assert_eq!(report.run_status, RunStatus::Failed);
            assert_eq!(report.scenarios.len(), 2);
            assert_eq!(report.scenarios[0], completed[0]);
            let failed = &report.scenarios[1];
            assert_eq!(failed.id, "query.page.deep");
            assert_eq!(failed.status, RunStatus::Failed);
            assert_eq!(failed.timeout_count, expected_timeout);
            assert!(failed.samples.elapsed_micros.is_empty());
            assert!(failed.errors[0].contains("scenario operation failed"));
            assert!(failed.errors[0].contains("low-level failure"));
            assert!(report.errors[0].contains("query.page.deep"));
        }
    }

    #[test]
    fn orchestration_failure_preserves_only_actually_completed_records() {
        let digest = "d".repeat(64);
        let completed = protocol_test_report(&digest, 100, 100, 1_000).scenarios;
        let dataset = DatasetFingerprint {
            schema_version: 1,
            seed: dataset::DATASET_SEED,
            item_count: 120,
            source_count: 2,
            manifest_digest: Some(digest),
        };
        let report = failed_orchestration_report(
            test_run_metadata(),
            dataset,
            completed.clone(),
            "restore dataset baseline",
            &anyhow::anyhow!("unsafe restore target").context("baseline restore failed"),
        );

        assert_eq!(report.run_status, RunStatus::Failed);
        assert_eq!(report.scenarios, completed);
        assert_eq!(report.scenarios.len(), 1);
        assert!(report.errors[0].contains("restore dataset baseline"));
        assert!(report.errors[0].contains("baseline restore failed"));
        assert!(report.errors[0].contains("unsafe restore target"));
    }

    #[test]
    fn compare_command_writes_output_and_fails_closed_on_regression() {
        let directory = std::env::temp_dir().join(format!(
            "purewall-performance-compare-tests-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let baseline_path = directory.join("baseline.json");
        let candidate_path = directory.join("candidate.json");
        let output_path = directory.join("comparison.json");
        let digest = "f".repeat(64);
        let baseline = protocol_test_report(&digest, 100, 100, 1_000);
        let candidate = protocol_test_report(&digest, 106, 106, 1_060);
        report::write_benchmark_report(&baseline_path, &baseline).unwrap();
        report::write_benchmark_report(&candidate_path, &candidate).unwrap();

        assert!(execute_compare(&baseline_path, &candidate_path, &output_path).is_err());
        assert!(output_path.is_file());
        assert!(output_path.with_extension("md").is_file());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cleanup_failures_replace_passed_output_with_failed_reports_and_retain_the_path() {
        for (label, state, injected_error, retained_message) in [
            (
                "cleanup-validation-report",
                owned_temp::CleanupFailureState::BeforeRemoval,
                "injected cleanup safety validation failure",
                "performance data retained intact at",
            ),
            (
                "cleanup-removal-report",
                owned_temp::CleanupFailureState::RemovalMayBePartial,
                "injected remove_dir_all failure",
                "remaining performance data may be partial at",
            ),
        ] {
            let sequence = RUNNER_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let run_id = format!("{label}-{}-{sequence}", std::process::id());
            let run = OwnedRunRoot::create_for_test(&run_id).unwrap();
            let root_path = run.root().to_path_buf();
            let report_directory = unique_runner_test_directory(label);
            let report_path = report_directory.join("result.json");
            report::preflight_benchmark_report_target(&report_path).unwrap();
            let digest = "9".repeat(64);
            let scenarios = protocol_test_report(&digest, 100, 100, 1_000).scenarios;
            let dataset = DatasetFingerprint {
                schema_version: DATASET_SCHEMA_VERSION,
                seed: DATASET_SEED,
                item_count: 120,
                source_count: 2,
                manifest_digest: Some(digest),
            };

            let error = finish_run_with_cleanup(
                &report_path,
                CompletedRun {
                    metadata: test_run_metadata(),
                    dataset,
                    scenarios,
                },
                run,
                false,
                |run| {
                    run.cleanup_with_test_operations(
                        |owned| {
                            if state == owned_temp::CleanupFailureState::BeforeRemoval {
                                bail!(injected_error)
                            }
                            Ok(owned.root().to_path_buf())
                        },
                        |_| bail!(injected_error),
                    )
                },
                |run| {
                    let path = run.root().to_path_buf();
                    run.cleanup().unwrap();
                    path
                },
            )
            .expect_err("cleanup failure must fail the run");

            let error_chain = format!("{error:#}");
            assert!(error_chain.contains(injected_error));
            assert!(error_chain.contains(retained_message));
            assert!(error_chain.contains(&root_path.display().to_string()));
            let failed = report::read_benchmark_report(&report_path).unwrap();
            assert_eq!(failed.run_status, RunStatus::Failed);
            assert!(failed.errors[0].contains("cleanup owned run root"));
            assert!(failed.errors[0].contains(injected_error));
            let markdown = std::fs::read_to_string(report_path.with_extension("md")).unwrap();
            assert!(markdown.contains("- Run status: Failed"));
            assert!(markdown.contains(injected_error));
            assert!(!root_path.exists());

            std::fs::remove_dir_all(report_directory).unwrap();
        }
    }

    #[test]
    fn ci_run_writes_passed_reports_and_cleans_owned_data() {
        let sequence = RUNNER_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let run_id = format!("task7-ci-{}-{sequence}", std::process::id());
        let report_directory = std::env::temp_dir()
            .join("purewall-performance-runner-tests")
            .join(&run_id);
        std::fs::create_dir_all(&report_directory).unwrap();
        let report_path = report_directory.join("ci.json");

        execute_run_with_id(DatasetScale::Ci, &report_path, false, false, &run_id).unwrap();

        let report = report::read_benchmark_report(&report_path).unwrap();
        assert_eq!(report.run_status, RunStatus::Passed);
        assert_eq!(report.scenarios.len(), 19);
        assert!(report.scenarios.iter().all(|scenario| {
            scenario.status == RunStatus::Passed && scenario.errors.is_empty()
        }));
        assert!(report_path.with_extension("md").is_file());
        assert!(!std::env::temp_dir()
            .join("purewall-performance")
            .join(&run_id)
            .exists());

        std::fs::remove_dir_all(report_directory).unwrap();
    }
}
