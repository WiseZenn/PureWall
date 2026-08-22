use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use anyhow::{bail, Context, Result};

use super::compare::ComparisonReport;
use super::dataset::DATASET_SCHEMA_VERSION;
use super::protocol::{
    BenchmarkReport, RunStatus, REPORT_SCHEMA_VERSION, SCENARIO_CONTRACT_VERSION,
};

pub(crate) fn preflight_benchmark_report_target(path: &Path) -> Result<()> {
    validate_json_target(path)?;
    preflight_atomic_target(&path.with_extension("md"))?;
    preflight_atomic_target(path)
}

pub(crate) fn write_benchmark_report(path: &Path, report: &BenchmarkReport) -> Result<()> {
    validate_json_target(path)?;
    let markdown_path = path.with_extension("md");
    write_atomic_text(&markdown_path, &benchmark_markdown(report))?;
    let mut json = serde_json::to_string_pretty(report).context("serialize benchmark report")?;
    json.push('\n');
    write_atomic_text(path, &json)
}

pub(crate) fn read_benchmark_report(path: &Path) -> Result<BenchmarkReport> {
    let bytes =
        fs::read(path).with_context(|| format!("read benchmark report {}", path.display()))?;
    let report: BenchmarkReport =
        serde_json::from_slice(&bytes).context("parse benchmark report JSON")?;
    validate_benchmark_report(&report)?;
    Ok(report)
}

pub(crate) fn write_comparison_report(path: &Path, report: &ComparisonReport) -> Result<()> {
    validate_json_target(path)?;
    write_atomic_text(&path.with_extension("md"), &comparison_markdown(report))?;
    let mut json = serde_json::to_string_pretty(report).context("serialize comparison report")?;
    json.push('\n');
    write_atomic_text(path, &json)
}

fn validate_json_target(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("report target must be absolute: {}", path.display());
    }
    if path.is_dir() {
        bail!("report target must not be a directory: {}", path.display());
    }
    if path.extension().and_then(|value| value.to_str()) != Some("json") {
        bail!("report target must use the .json extension");
    }
    if path.parent().is_none_or(|parent| !parent.is_dir()) {
        bail!("report parent directory does not exist");
    }
    Ok(())
}

fn preflight_atomic_target(path: &Path) -> Result<()> {
    if path.is_dir() {
        bail!("report target must not be a directory: {}", path.display());
    }
    if path.exists() {
        let file = OpenOptions::new()
            .write(true)
            .open(path)
            .with_context(|| format!("report target is not writable: {}", path.display()))?;
        drop(file);
    }

    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("report target must have a UTF-8 file name")?;
    let temporary = path.with_file_name(format!("{file_name}.tmp"));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("report target is not writable: {}", path.display()))?;
    let sync_result = file
        .sync_all()
        .with_context(|| format!("sync report preflight probe {}", temporary.display()));
    drop(file);
    let cleanup_result = fs::remove_file(&temporary)
        .with_context(|| format!("remove report preflight probe {}", temporary.display()));
    match (sync_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(sync_error), Ok(())) => Err(sync_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(sync_error), Err(cleanup_error)) => Err(sync_error.context(format!(
            "also failed to remove report preflight probe: {cleanup_error:#}"
        ))),
    }
}

fn write_atomic_text(path: &Path, contents: &str) -> Result<()> {
    if path.is_dir() {
        bail!("report target must not be a directory: {}", path.display());
    }
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("report target must have a UTF-8 file name")?;
    let temporary = path.with_file_name(format!("{file_name}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("create temporary report {}", temporary.display()))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("write temporary report {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("sync temporary report {}", temporary.display()))?;
    drop(file);
    fs::rename(&temporary, path).with_context(|| {
        format!(
            "publish temporary report {} to {}",
            temporary.display(),
            path.display()
        )
    })
}

fn validate_benchmark_report(report: &BenchmarkReport) -> Result<()> {
    if report.schema_version != REPORT_SCHEMA_VERSION {
        bail!(
            "unsupported benchmark report schema version {}",
            report.schema_version
        );
    }
    if report.scenario_contract_version != SCENARIO_CONTRACT_VERSION {
        bail!(
            "unsupported scenario contract version {}",
            report.scenario_contract_version
        );
    }
    if report.dataset.schema_version != DATASET_SCHEMA_VERSION {
        bail!(
            "unsupported dataset schema version {}",
            report.dataset.schema_version
        );
    }
    if !is_hex(&report.git_commit, 40) {
        bail!("benchmark report git_commit must be 40 hexadecimal characters");
    }
    if chrono::DateTime::parse_from_rfc3339(&report.generated_at_utc).is_err() {
        bail!("benchmark report generated_at_utc must be RFC 3339");
    }
    match report.dataset.manifest_digest.as_deref() {
        Some(digest) if is_hex(digest, 64) => {}
        None if report.run_status == RunStatus::Failed && report.scenarios.is_empty() => {}
        _ => bail!("benchmark report manifest_digest must be 64 hexadecimal characters"),
    }
    if report.run_status == RunStatus::Failed
        && report.errors.is_empty()
        && report
            .scenarios
            .iter()
            .all(|scenario| scenario.errors.is_empty())
    {
        bail!("failed benchmark report must include report-level or scenario-level errors");
    }
    Ok(())
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn benchmark_markdown(report: &BenchmarkReport) -> String {
    let manifest = report.dataset.manifest_digest.as_deref().unwrap_or("null");
    let mut markdown = format!(
        "# PureWall Performance Report\n\n\
         These results describe one recorded environment. They are not a universal Windows performance claim.\n\n\
         - Run status: {:?}\n\n\
         ## Environment\n\n\
         - OS: {}\n\
         - Architecture: {}\n\
         - CPU: {}\n\
         - Logical cores: {}\n\
         - Installed memory: {} bytes\n\
         - Rust profile: {}\n\
         - Configuration digest: {}\n\n\
         ## Dataset\n\n\
         - Manifest digest: {}\n\
         - Item count: {}\n\
         - Source count: {}\n\n\
         ## Scenarios\n\n\
         | Scenario | Cache | Median | P95 | Peak working set | Status |\n\
         | --- | --- | ---: | ---: | ---: | --- |\n",
        report.run_status,
        report.environment.os,
        report.environment.arch,
        report.environment.cpu,
        report.environment.logical_cores,
        report.environment.installed_memory_bytes,
        report.environment.rust_profile,
        report.environment.config_digest,
        manifest,
        report.dataset.item_count,
        report.dataset.source_count,
    );
    for scenario in &report.scenarios {
        markdown.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {:?} |\n",
            scenario.id,
            scenario.cache_state,
            scenario.samples.median_micros,
            scenario.samples.p95_micros,
            scenario.samples.peak_working_set_bytes,
            scenario.status,
        ));
        markdown.push_str(&format!(
            "\n- `{}` raw elapsed micros: {}\n",
            scenario.id,
            scenario
                .samples
                .elapsed_micros
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        markdown.push_str(&format!(
            "- Counts: expected={:?}, actual={:?}; database={:?}; WAL={:?}; cache hits={:?}; cache misses={:?}; generated derivatives={:?}; queue peak={:?}; timeouts={}\n",
            scenario.expected_count,
            scenario.actual_count,
            scenario.database_bytes,
            scenario.wal_bytes,
            scenario.cache_hits,
            scenario.cache_misses,
            scenario.generated_derivatives,
            scenario.queue_peak_pending,
            scenario.timeout_count,
        ));
        if !scenario.errors.is_empty() {
            markdown.push_str(&format!("- Errors: {}\n", scenario.errors.join(" | ")));
        }
    }
    if !report.errors.is_empty() {
        markdown.push_str(&format!(
            "\n## Report errors\n\n{}\n",
            report.errors.join("\n")
        ));
    }
    markdown
}

fn comparison_markdown(report: &ComparisonReport) -> String {
    let mut markdown = format!(
        "# PureWall Performance Comparison\n\n\
         - Baseline commit: {}\n\
         - Candidate commit: {}\n\
         - Overall: {:?}\n\n\
         | Scenario | Cache | Median change | P95 change | Peak working set change | Verdict |\n\
         | --- | --- | ---: | ---: | ---: | --- |\n",
        report.baseline_commit, report.candidate_commit, report.overall,
    );
    for scenario in &report.scenarios {
        markdown.push_str(&format!(
            "| {} | {:?} | {} | {} | {} | {:?} |\n",
            scenario.id,
            scenario.cache_state,
            format_change(scenario.median_change),
            format_change(scenario.p95_change),
            format_change(scenario.peak_working_set_change),
            scenario.verdict,
        ));
        if !scenario.reasons.is_empty() {
            markdown.push_str(&format!(
                "\n- `{}`: {}\n",
                scenario.id,
                scenario.reasons.join(" | ")
            ));
        }
    }
    if !report.errors.is_empty() {
        markdown.push_str(&format!(
            "\n## Comparison errors\n\n{}\n",
            report.errors.join("\n")
        ));
    }
    markdown
}

fn format_change(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::performance_harness::{
        compare::compare_reports,
        dataset::DATASET_SCHEMA_VERSION,
        protocol::{protocol_test_report, RunStatus, REPORT_SCHEMA_VERSION},
    };

    use super::{read_benchmark_report, write_benchmark_report, write_comparison_report};

    static REPORT_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_report_test_directory(label: &str) -> PathBuf {
        let directory = std::env::temp_dir()
            .join("purewall-performance-report-tests")
            .join(format!(
                "{label}-{}-{}",
                std::process::id(),
                REPORT_TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&directory).expect("create report test directory");
        directory
    }

    #[test]
    fn report_round_trips_and_emits_markdown_disclaimer() {
        let directory = unique_report_test_directory("report-roundtrip");
        let json = directory.join("report.json");
        let digest = "a".repeat(64);
        let report = protocol_test_report(&digest, 100, 100, 1_000);
        write_benchmark_report(&json, &report).unwrap();
        assert_eq!(read_benchmark_report(&json).unwrap(), report);
        let markdown = fs::read_to_string(directory.join("report.md")).unwrap();
        assert!(
            markdown.contains("| Scenario | Cache | Median | P95 | Peak working set | Status |")
        );
        assert!(markdown.contains("not a universal Windows performance claim"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reader_rejects_invalid_versions_commit_digest_and_failed_diagnostics() {
        let directory = unique_report_test_directory("report-validation");
        let json = directory.join("report.json");
        let digest = "b".repeat(64);

        let mut invalid_schema = protocol_test_report(&digest, 100, 100, 1_000);
        invalid_schema.schema_version = REPORT_SCHEMA_VERSION + 1;
        fs::write(&json, serde_json::to_vec(&invalid_schema).unwrap()).unwrap();
        assert!(read_benchmark_report(&json).is_err());

        let mut invalid_commit = protocol_test_report(&digest, 100, 100, 1_000);
        invalid_commit.git_commit = "not-a-commit".into();
        fs::write(&json, serde_json::to_vec(&invalid_commit).unwrap()).unwrap();
        assert!(read_benchmark_report(&json).is_err());

        let missing_digest = protocol_test_report("", 100, 100, 1_000);
        fs::write(&json, serde_json::to_vec(&missing_digest).unwrap()).unwrap();
        assert!(read_benchmark_report(&json).is_err());

        let mut failed_without_diagnostics = protocol_test_report(&digest, 100, 100, 1_000);
        failed_without_diagnostics.run_status = RunStatus::Failed;
        fs::write(
            &json,
            serde_json::to_vec(&failed_without_diagnostics).unwrap(),
        )
        .unwrap();
        assert!(read_benchmark_report(&json).is_err());

        let mut failed_pre_generation = protocol_test_report(&digest, 100, 100, 1_000);
        failed_pre_generation.run_status = RunStatus::Failed;
        failed_pre_generation.dataset.manifest_digest = None;
        failed_pre_generation.scenarios.clear();
        failed_pre_generation
            .errors
            .push("generation failed".into());
        fs::write(&json, serde_json::to_vec(&failed_pre_generation).unwrap()).unwrap();
        assert_eq!(read_benchmark_report(&json).unwrap(), failed_pre_generation);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reader_rejects_unknown_dataset_schema_version() {
        let directory = unique_report_test_directory("dataset-schema-validation");
        let json = directory.join("report.json");
        let digest = "e".repeat(64);
        let mut report = protocol_test_report(&digest, 100, 100, 1_000);
        report.dataset.schema_version = DATASET_SCHEMA_VERSION + 1;
        fs::write(&json, serde_json::to_vec(&report).unwrap()).unwrap();

        let error = read_benchmark_report(&json).expect_err("dataset schema 2 must be rejected");
        assert!(error.to_string().contains("dataset schema version"));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reader_rejects_null_manifest_digest_after_any_scenario_was_recorded() {
        let directory = unique_report_test_directory("failed-scenario-null-digest");
        let json = directory.join("report.json");
        let digest = "f".repeat(64);
        let mut report = protocol_test_report(&digest, 100, 100, 1_000);
        report.run_status = RunStatus::Failed;
        report.dataset.manifest_digest = None;
        report.errors.push("scenario failed".into());
        fs::write(&json, serde_json::to_vec(&report).unwrap()).unwrap();

        let error = read_benchmark_report(&json)
            .expect_err("a failed report with scenarios still requires a digest");
        assert!(error.to_string().contains("manifest_digest"));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn writer_rejects_directory_targets_and_emits_atomic_lf_utf8() {
        let directory = unique_report_test_directory("report-atomic");
        let digest = "c".repeat(64);
        let report = protocol_test_report(&digest, 100, 120, 1_000);
        assert!(write_benchmark_report(&directory, &report).is_err());

        let json = directory.join("atomic.json");
        write_benchmark_report(&json, &report).unwrap();
        let bytes = fs::read(&json).unwrap();
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(!bytes.windows(2).any(|window| window == b"\r\n"));
        assert!(!directory.join("atomic.json.tmp").exists());

        let markdown = fs::read_to_string(directory.join("atomic.md")).unwrap();
        for expected in [
            "test-cpu", &digest, "120", "2", "100, 120", "100", "120", "1000",
        ] {
            assert!(markdown.contains(expected), "missing {expected}");
        }

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn comparison_writer_emits_adjacent_json_and_markdown() {
        let directory = unique_report_test_directory("comparison-report");
        let json = directory.join("comparison.json");
        let digest = "d".repeat(64);
        let baseline = protocol_test_report(&digest, 100, 100, 1_000);
        let candidate = protocol_test_report(&digest, 100, 100, 1_000);
        let comparison = compare_reports(&baseline, &candidate);

        write_comparison_report(&json, &comparison).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&fs::read(&json).unwrap()).unwrap();
        assert_eq!(parsed["overall"], "stable");
        let markdown = fs::read_to_string(directory.join("comparison.md")).unwrap();
        assert!(markdown.contains(
            "| Scenario | Cache | Median change | P95 change | Peak working set change | Verdict |"
        ));
        assert!(markdown.contains("Stable"));

        fs::remove_dir_all(directory).unwrap();
    }
}
