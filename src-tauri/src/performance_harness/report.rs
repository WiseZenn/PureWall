use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{bail, Context, Result};

use super::compare::ComparisonReport;
use super::dataset::DATASET_SCHEMA_VERSION;
use super::protocol::{
    BenchmarkReport, RunStatus, REPORT_SCHEMA_VERSION, SCENARIO_CONTRACT_VERSION,
};

const PUBLICATION_MARKER_SUFFIX: &str = ".publishing";
const PUBLICATION_MARKER_CONTENTS: &[u8] = b"PureWall report publication is incomplete\n";

pub(crate) fn preflight_benchmark_report_target(path: &Path) -> Result<()> {
    validate_json_target(path)?;
    preflight_publication_marker(path)?;
    preflight_atomic_target(path)?;
    preflight_atomic_target(&path.with_extension("md"))
}

pub(crate) fn write_benchmark_report(path: &Path, report: &BenchmarkReport) -> Result<()> {
    validate_json_target(path)?;
    let mut json = serde_json::to_string_pretty(report).context("serialize benchmark report")?;
    json.push('\n');
    publish_report_pair(path, &json, &benchmark_markdown(report))
}

pub(crate) fn read_benchmark_report(path: &Path) -> Result<BenchmarkReport> {
    ensure_publication_complete(path)?;
    let bytes =
        fs::read(path).with_context(|| format!("read benchmark report {}", path.display()))?;
    ensure_publication_complete(path)?;
    let report: BenchmarkReport =
        serde_json::from_slice(&bytes).context("parse benchmark report JSON")?;
    validate_benchmark_report(&report)?;
    ensure_publication_complete(path)?;
    Ok(report)
}

pub(crate) fn write_comparison_report(path: &Path, report: &ComparisonReport) -> Result<()> {
    validate_json_target(path)?;
    let mut json = serde_json::to_string_pretty(report).context("serialize comparison report")?;
    json.push('\n');
    publish_report_pair(path, &json, &comparison_markdown(report))
}

fn publish_report_pair(path: &Path, json: &str, markdown: &str) -> Result<()> {
    let publication_marker = begin_publication(path)?;
    write_atomic_text(path, json)?;
    write_atomic_text(&path.with_extension("md"), markdown)?;
    let marker_file = open_owned_publication_marker(&publication_marker, false)?;
    drop(marker_file);
    fs::remove_file(&publication_marker).with_context(|| {
        format!(
            "remove report publication marker {}",
            publication_marker.display()
        )
    })
}

fn publication_marker_path(path: &Path) -> Result<std::path::PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .context("report target must have a UTF-8 file name")?;
    Ok(path.with_file_name(format!("{file_name}{PUBLICATION_MARKER_SUFFIX}")))
}

fn preflight_publication_marker(path: &Path) -> Result<()> {
    let marker = publication_marker_path(path)?;
    match fs::symlink_metadata(&marker) {
        Ok(metadata) => {
            validate_publication_marker_metadata(&marker, &metadata)?;
            let file = open_owned_publication_marker(&marker, true).with_context(|| {
                format!(
                    "report publication marker is not writable: {}",
                    marker.display()
                )
            })?;
            file.sync_all()
                .with_context(|| format!("sync report publication marker {}", marker.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            preflight_atomic_target(&marker)
        }
        Err(error) => Err(error)
            .with_context(|| format!("inspect report publication marker {}", marker.display())),
    }
}

fn begin_publication(path: &Path) -> Result<std::path::PathBuf> {
    let marker = publication_marker_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    configure_publication_marker_open(&mut options);
    let mut file = match options.open(&marker) {
        Ok(file) => {
            file.set_len(0).with_context(|| {
                format!("initialize report publication marker {}", marker.display())
            })?;
            file
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let file = open_owned_publication_marker(&marker, true)?;
            file.sync_all().with_context(|| {
                format!(
                    "sync existing report publication marker {}",
                    marker.display()
                )
            })?;
            drop(file);
            return Ok(marker);
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("create report publication marker {}", marker.display()));
        }
    };
    file.write_all(PUBLICATION_MARKER_CONTENTS)
        .with_context(|| format!("write report publication marker {}", marker.display()))?;
    file.sync_all()
        .with_context(|| format!("sync report publication marker {}", marker.display()))?;
    drop(file);
    Ok(marker)
}

fn open_owned_publication_marker(path: &Path, writable: bool) -> Result<File> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("inspect report publication marker {}", path.display()))?;
    validate_publication_marker_metadata(path, &metadata)?;

    let mut options = OpenOptions::new();
    options.read(true).write(writable);
    configure_publication_marker_open(&mut options);
    let mut file = options
        .open(path)
        .with_context(|| format!("open report publication marker {}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .with_context(|| format!("inspect open report publication marker {}", path.display()))?;
    validate_publication_marker_metadata(path, &opened_metadata)?;
    validate_publication_marker_contents(path, &mut file)?;
    Ok(file)
}

fn validate_publication_marker_metadata(path: &Path, metadata: &Metadata) -> Result<()> {
    if !metadata.file_type().is_file() {
        bail!(
            "report publication marker must be a regular file: {}",
            path.display()
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            bail!(
                "report publication marker must not be a reparse point: {}",
                path.display()
            );
        }
    }
    if metadata.len() != PUBLICATION_MARKER_CONTENTS.len() as u64 {
        bail!(
            "report publication marker is not owned by PureWall: {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_publication_marker_contents(path: &Path, file: &mut File) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("seek report publication marker {}", path.display()))?;
    let mut contents = vec![0_u8; PUBLICATION_MARKER_CONTENTS.len()];
    file.read_exact(&mut contents)
        .with_context(|| format!("read report publication marker {}", path.display()))?;
    let mut trailing = [0_u8; 1];
    let trailing_length = file
        .read(&mut trailing)
        .with_context(|| format!("read report publication marker {}", path.display()))?;
    if contents != PUBLICATION_MARKER_CONTENTS || trailing_length != 0 {
        bail!(
            "report publication marker is not owned by PureWall: {}",
            path.display()
        );
    }
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("rewind report publication marker {}", path.display()))?;
    Ok(())
}

fn configure_publication_marker_open(options: &mut OpenOptions) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}

fn ensure_publication_complete(path: &Path) -> Result<()> {
    let marker = publication_marker_path(path)?;
    match fs::symlink_metadata(&marker) {
        Ok(_) => bail!(
            "report publication is incomplete while marker exists: {}",
            marker.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("inspect report publication marker {}", marker.display())),
    }
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
    let write_result = file
        .write_all(contents.as_bytes())
        .with_context(|| format!("write temporary report {}", temporary.display()))
        .and_then(|()| {
            file.sync_all()
                .with_context(|| format!("sync temporary report {}", temporary.display()))
        });
    drop(file);
    if let Err(error) = write_result {
        return Err(cleanup_owned_temporary_after_error(&temporary, error));
    }
    if let Err(error) = fs::rename(&temporary, path).with_context(|| {
        format!(
            "publish temporary report {} to {}",
            temporary.display(),
            path.display()
        )
    }) {
        return Err(cleanup_owned_temporary_after_error(&temporary, error));
    }
    Ok(())
}

fn cleanup_owned_temporary_after_error(temporary: &Path, error: anyhow::Error) -> anyhow::Error {
    match fs::remove_file(temporary) {
        Ok(()) => error,
        Err(cleanup_error) if cleanup_error.kind() == std::io::ErrorKind::NotFound => error,
        Err(cleanup_error) => error.context(format!(
            "also failed to remove owned temporary report {}: {cleanup_error}",
            temporary.display()
        )),
    }
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

    use super::{
        preflight_benchmark_report_target, read_benchmark_report, write_benchmark_report,
        write_comparison_report, PUBLICATION_MARKER_CONTENTS,
    };

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

    #[test]
    fn preflight_rejects_foreign_publication_marker_without_mutating_it() {
        let directory = unique_report_test_directory("foreign-marker-preflight");
        let json = directory.join("report.json");
        let publication_marker = directory.join("report.json.publishing");
        let foreign_contents = vec![b'X'; PUBLICATION_MARKER_CONTENTS.len()];
        fs::write(&publication_marker, &foreign_contents).unwrap();

        let error = preflight_benchmark_report_target(&json)
            .expect_err("a foreign publication marker must fail preflight");
        assert!(format!("{error:#}").contains("publication marker"));
        assert_eq!(fs::read(&publication_marker).unwrap(), foreign_contents);
        assert!(!json.exists());
        assert!(!directory.join("report.md").exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn writer_rejects_foreign_publication_marker_without_mutating_it() {
        let directory = unique_report_test_directory("foreign-marker-write");
        let json = directory.join("report.json");
        let publication_marker = directory.join("report.json.publishing");
        let foreign_contents = vec![b'Y'; PUBLICATION_MARKER_CONTENTS.len()];
        fs::write(&publication_marker, &foreign_contents).unwrap();
        let digest = "6".repeat(64);
        let report = protocol_test_report(&digest, 100, 100, 1_000);

        let error = write_benchmark_report(&json, &report)
            .expect_err("a foreign publication marker must block publication");
        assert!(format!("{error:#}").contains("publication marker"));
        assert_eq!(fs::read(&publication_marker).unwrap(), foreign_contents);
        assert!(!json.exists());
        assert!(!directory.join("report.md").exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn interrupted_second_artifact_publication_blocks_reads_until_retry_completes() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = unique_report_test_directory("interrupted-pair-publication");
        let json = directory.join("report.json");
        let markdown = directory.join("report.md");
        let publication_marker = directory.join("report.json.publishing");
        let digest = "8".repeat(64);
        let passed = protocol_test_report(&digest, 100, 100, 1_000);
        write_benchmark_report(&json, &passed).unwrap();

        let markdown_lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(&markdown)
            .unwrap();
        let mut failed = passed.clone();
        failed.run_status = RunStatus::Failed;
        failed.errors.push("cleanup failed".into());

        let write_error = write_benchmark_report(&json, &failed)
            .expect_err("the locked second artifact must interrupt publication");
        assert!(format!("{write_error:#}").contains("publish temporary report"));
        assert!(publication_marker.is_file());
        let read_error =
            read_benchmark_report(&json).expect_err("an incomplete report pair must fail closed");
        assert!(read_error.to_string().contains("publication is incomplete"));
        assert!(!directory.join("report.json.tmp").exists());
        assert!(!directory.join("report.md.tmp").exists());

        drop(markdown_lock);
        write_benchmark_report(&json, &failed).expect("a complete retry must succeed");
        assert!(!publication_marker.exists());
        assert_eq!(read_benchmark_report(&json).unwrap(), failed);
        assert!(!directory.join("report.json.tmp").exists());
        assert!(!directory.join("report.md.tmp").exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn marker_removal_failure_keeps_the_published_pair_unreadable_until_retry() {
        use std::os::windows::fs::OpenOptionsExt;

        let directory = unique_report_test_directory("locked-publication-marker");
        let json = directory.join("report.json");
        let publication_marker = directory.join("report.json.publishing");
        let digest = "7".repeat(64);
        let passed = protocol_test_report(&digest, 100, 100, 1_000);
        write_benchmark_report(&json, &passed).unwrap();
        fs::write(&publication_marker, PUBLICATION_MARKER_CONTENTS).unwrap();
        let marker_lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0x1 | 0x2)
            .open(&publication_marker)
            .unwrap();

        let mut failed = passed;
        failed.run_status = RunStatus::Failed;
        failed.errors.push("cleanup failed".into());
        let write_error = write_benchmark_report(&json, &failed)
            .expect_err("failure to remove the publication marker must fail the write");
        assert!(format!("{write_error:#}").contains("publication marker"));
        assert!(publication_marker.is_file());
        assert!(read_benchmark_report(&json).is_err());

        drop(marker_lock);
        write_benchmark_report(&json, &failed).expect("retry must clear the stale marker");
        assert!(!publication_marker.exists());
        assert_eq!(read_benchmark_report(&json).unwrap(), failed);

        fs::remove_dir_all(directory).unwrap();
    }
}
