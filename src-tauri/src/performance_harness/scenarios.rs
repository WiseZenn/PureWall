use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context, Result};

use super::dataset::GeneratedDataset;
use super::metrics::{
    measure_operation_validated, measure_operation_with_setup_validated, run_with_deadline,
    DeadlineError,
};
use super::owned_temp::OwnedRunRoot;
use super::protocol::{CacheState, RunStatus, ScenarioRecord, ScenarioSamples};
use crate::db::{
    Database, Stats, WallpaperEntry, WallpaperPage, WatchedFolderSummary, DEFAULT_TAG_COLOR,
};
use crate::media_queue::{MediaJob, MediaJobKind, MediaJobQueue, MediaNeeds, MediaQueueSnapshot};
use crate::scanner::{scan_files, scan_folder, ImageInfo};
use crate::thumbnails::{DerivativePaths, ThumbnailCache};
use crate::ReconciliationOutcome;

const SOURCE_LABEL: &str = "mounted";
const PAGE_LIMIT: i64 = 120;
const TAG_NAME: &str = "风景";
const COLLECTION_NAME: &str = "Benchmark Collection";
const MEDIA_PATH_COUNT: usize = 36;
const ACTIVE_CLAIM_THUMBNAILS: usize = 256;
const SCENARIO_IDS: [&str; 19] = [
    "scan.full",
    "import.atomic",
    "reconcile.no-change",
    "reconcile.incremental",
    "reconcile.recovery",
    "startup.open-first-page-stats",
    "query.page.first",
    "query.page.deep",
    "query.search.unicode",
    "query.search.none",
    "query.filter.tag",
    "query.filter.collection",
    "query.sort.plays",
    "playback.next.single",
    "playback.next.multi",
    "media.thumbnail.cold",
    "media.thumbnail.warm",
    "media.preview.cold",
    "media.preview.active-claim",
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct SamplePolicy {
    pub(crate) heavy: usize,
    pub(crate) short: usize,
    pub(crate) scenario_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScenarioFailureKind {
    Error,
    Timeout { timeout: Duration },
}

#[derive(Debug)]
pub(crate) struct ScenarioRunFailure {
    pub(crate) scenario_id: String,
    pub(crate) kind: ScenarioFailureKind,
    pub(crate) error: anyhow::Error,
    pub(crate) completed_records: Vec<ScenarioRecord>,
}

pub(crate) type ScenarioRunResult<T> = std::result::Result<T, ScenarioRunFailure>;

fn run_scenario_with_deadline<T, F>(
    completed_records: &mut Vec<ScenarioRecord>,
    scenario_id: &str,
    timeout: Duration,
    operation: F,
) -> ScenarioRunResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    run_with_deadline(timeout, operation).map_err(|error| {
        let kind = match &error {
            DeadlineError::Timeout { timeout } => {
                ScenarioFailureKind::Timeout { timeout: *timeout }
            }
            DeadlineError::Failed(_) => ScenarioFailureKind::Error,
        };
        ScenarioRunFailure {
            scenario_id: scenario_id.to_string(),
            kind,
            error: anyhow::Error::new(error),
            completed_records: std::mem::take(completed_records),
        }
    })
}

fn scenario_setup<T>(
    completed_records: &mut Vec<ScenarioRecord>,
    scenario_id: &str,
    result: Result<T>,
) -> ScenarioRunResult<T> {
    result.map_err(|error| ScenarioRunFailure {
        scenario_id: scenario_id.to_string(),
        kind: ScenarioFailureKind::Error,
        error,
        completed_records: std::mem::take(completed_records),
    })
}

fn complete_scenario(
    completed_records: &mut Vec<ScenarioRecord>,
    scenario_id: &str,
    record: Result<ScenarioRecord>,
) -> ScenarioRunResult<()> {
    let record = scenario_setup(completed_records, scenario_id, record)?;
    completed_records.push(record);
    Ok(())
}

impl SamplePolicy {
    pub(crate) fn ci() -> Self {
        Self {
            heavy: 1,
            short: 3,
            scenario_timeout: Duration::from_secs(60),
        }
    }

    pub(crate) fn standard() -> Self {
        Self {
            heavy: 3,
            short: 20,
            scenario_timeout: Duration::from_secs(15 * 60),
        }
    }
}

fn measure_each_sample<T>(
    count: usize,
    mut operation: impl FnMut(usize) -> Result<T>,
    mut consume: impl FnMut(usize, &T) -> Result<()>,
) -> Result<ScenarioSamples> {
    let (samples, value) = measure_operation_validated(count, &mut operation, &mut consume)?;
    drop(value);
    Ok(samples)
}

pub(crate) fn performance_config_descriptor(policy: SamplePolicy) -> String {
    format!(
        "scenarios={};sample-heavy={};sample-short={};scenario-timeout-ms={};media-paths={MEDIA_PATH_COUNT};active-claim-thumbnails={ACTIVE_CLAIM_THUMBNAILS}",
        SCENARIO_IDS.join(","),
        policy.heavy,
        policy.short,
        policy.scenario_timeout.as_millis(),
    )
}

struct LibrarySample {
    database_path: PathBuf,
    fixture: Option<QueryFixture>,
}

#[derive(Debug, Clone)]
struct QueryFixture {
    tag_id: i64,
    collection_id: i64,
    tagged: HashSet<String>,
    collected: HashSet<String>,
    liked: HashSet<String>,
    unicode_matches: HashSet<String>,
    played: Vec<(String, i32)>,
}

#[derive(Clone)]
struct IncrementalRoot {
    additions: Vec<ImageInfo>,
    removals: Vec<String>,
}

#[derive(Debug)]
struct StartupSnapshot {
    summaries: Vec<WatchedFolderSummary>,
    page: WallpaperPage,
    stats: Stats,
}

struct StartupMeasurement {
    _database: Database,
    snapshot: StartupSnapshot,
}

pub(crate) fn run_library_scenarios(
    dataset: &GeneratedDataset,
    run: &OwnedRunRoot,
    policy: SamplePolicy,
) -> ScenarioRunResult<Vec<ScenarioRecord>> {
    let mut records = Vec::with_capacity(13);
    scenario_setup(
        &mut records,
        "scan.full",
        (|| {
            ensure!(
                policy.heavy > 0 && policy.short > 0 && policy.scenario_timeout > Duration::ZERO,
                "sample counts and scenario timeout must be positive"
            );
            validate_dataset_contract(dataset)
        })(),
    )?;

    let total = dataset.manifest.item_count;
    let total_u64 = scenario_setup(
        &mut records,
        "scan.full",
        usize_to_u64(total, "dataset item count"),
    )?;
    let source_roots = scenario_setup(&mut records, "scan.full", canonical_source_roots(dataset))?;
    let scan_roots = source_roots.clone();
    let scan_expected_paths = dataset.absolute_paths.clone();
    let source_count = dataset.source_roots.len();
    let (scan_samples, scanned_count) = run_scenario_with_deadline(
        &mut records,
        "scan.full",
        policy.scenario_timeout,
        move || {
            let mut observed = None;
            let samples = measure_each_sample(
                policy.heavy,
                |_| scan_all_sources(&scan_roots),
                |_, images| {
                    ensure!(
                        images.len() == source_count,
                        "scan result source count mismatch"
                    );
                    validate_scanned_paths(images, &scan_expected_paths, "full scan")?;
                    remember_count(&mut observed, image_count(images), "full scan image count")
                },
            )?;
            Ok((samples, observed.context("full scan produced no count")?))
        },
    )?;
    let scanned_u64 = scenario_setup(
        &mut records,
        "scan.full",
        usize_to_u64(scanned_count, "scanned image count"),
    )?;
    complete_scenario(
        &mut records,
        "scan.full",
        record("scan.full", true, scan_samples, total_u64, scanned_u64),
    )?;

    let scanned_by_root = scenario_setup(
        &mut records,
        "import.atomic",
        scan_all_sources(&source_roots),
    )?;
    scenario_setup(
        &mut records,
        "import.atomic",
        validate_full_scan(dataset, &scanned_by_root),
    )?;
    let database_dir = run.database_dir();
    let mut libraries = (0..policy.heavy)
        .map(|sample_index| LibrarySample {
            database_path: database_dir.join(format!("import-atomic-{sample_index:04}.db")),
            fixture: None,
        })
        .collect::<Vec<_>>();
    let import_paths = libraries
        .iter()
        .map(|library| library.database_path.clone())
        .collect::<Vec<_>>();
    let import_roots = source_roots.clone();
    let import_images = scanned_by_root.clone();
    let expected_imported = scanned_by_root.iter().map(Vec::len).collect::<Vec<_>>();
    let import_expected = expected_imported.clone();
    let (import_samples, imported_count) = run_scenario_with_deadline(
        &mut records,
        "import.atomic",
        policy.scenario_timeout,
        move || {
            let databases = import_paths
                .iter()
                .map(|path| {
                    let database = Database::new(path).with_context(|| {
                        format!("open isolated import database {}", path.display())
                    })?;
                    register_sources(&database, &import_roots)?;
                    Ok(database)
                })
                .collect::<Result<Vec<_>>>()?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.heavy,
                |sample_index| {
                    reconcile_full_snapshot(&databases[sample_index], &import_roots, &import_images)
                },
                |_, outcomes| {
                    let imported = validate_reconciliation_outcomes(
                        outcomes,
                        &import_expected,
                        "atomic import",
                    )?;
                    remember_count(&mut observed, imported, "atomic import count")
                },
            )?;
            Ok((
                samples,
                observed.context("atomic import produced no count")?,
            ))
        },
    )?;
    let initial_source_counts = scenario_setup(
        &mut records,
        "import.atomic",
        source_ownership_counts(dataset, &dataset.absolute_paths),
    )?;
    let no_unavailable = vec![0; dataset.source_roots.len()];
    for library in &libraries {
        let database = scenario_setup(
            &mut records,
            "import.atomic",
            Database::new(&library.database_path),
        )?;
        scenario_setup(
            &mut records,
            "import.atomic",
            validate_available_library(&database, &dataset.absolute_paths, &[]),
        )?;
        scenario_setup(
            &mut records,
            "import.atomic",
            validate_source_ownership(&database, dataset, &initial_source_counts, &no_unavailable),
        )?;
    }
    let imported_u64 = scenario_setup(
        &mut records,
        "import.atomic",
        usize_to_u64(imported_count, "imported image count"),
    )?;
    complete_scenario(
        &mut records,
        "import.atomic",
        record_with_database(
            "import.atomic",
            true,
            import_samples,
            total_u64,
            imported_u64,
            &libraries[0].database_path,
        ),
    )?;
    for library in &mut libraries {
        let database = scenario_setup(
            &mut records,
            "reconcile.no-change",
            Database::new(&library.database_path),
        )?;
        library.fixture = Some(scenario_setup(
            &mut records,
            "reconcile.no-change",
            prepare_query_fixture(&database, dataset),
        )?);
    }

    let no_change_paths = libraries
        .iter()
        .map(|library| library.database_path.clone())
        .collect::<Vec<_>>();
    let no_change_roots = source_roots.clone();
    let no_change_images = scanned_by_root.clone();
    let no_change_expected = expected_imported.clone();
    let no_change_samples = run_scenario_with_deadline(
        &mut records,
        "reconcile.no-change",
        policy.scenario_timeout,
        move || {
            let databases = no_change_paths
                .iter()
                .map(Database::new)
                .collect::<Result<Vec<_>>>()?;
            measure_each_sample(
                policy.heavy,
                |sample_index| {
                    reconcile_full_snapshot(
                        &databases[sample_index % databases.len()],
                        &no_change_roots,
                        &no_change_images,
                    )
                },
                |_, outcomes| {
                    validate_reconciliation_outcomes(
                        outcomes,
                        &no_change_expected,
                        "no-change reconciliation",
                    )?;
                    Ok(())
                },
            )
        },
    )?;
    for library in &libraries {
        let database = scenario_setup(
            &mut records,
            "reconcile.no-change",
            Database::new(&library.database_path),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.no-change",
            validate_available_library(&database, &dataset.absolute_paths, &[]),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.no-change",
            validate_source_ownership(&database, dataset, &initial_source_counts, &no_unavailable),
        )?;
        let fixture = scenario_setup(
            &mut records,
            "reconcile.no-change",
            library.fixture.as_ref().context("missing query fixture"),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.no-change",
            validate_preserved_metadata(&database, fixture, &[]),
        )?;
    }
    let no_change_database = scenario_setup(
        &mut records,
        "reconcile.no-change",
        Database::new(&libraries[0].database_path),
    )?;
    let no_change_count = scenario_setup(
        &mut records,
        "reconcile.no-change",
        no_change_database.get_wallpapers_page("all", "created", "", 0, 1),
    )?
    .total;
    let no_change_u64 = scenario_setup(
        &mut records,
        "reconcile.no-change",
        i64_to_u64(no_change_count, "no-change available count"),
    )?;
    complete_scenario(
        &mut records,
        "reconcile.no-change",
        record_with_database(
            "reconcile.no-change",
            true,
            no_change_samples,
            total_u64,
            no_change_u64,
            &libraries[0].database_path,
        ),
    )?;

    let mutations = scenario_setup(
        &mut records,
        "reconcile.incremental",
        dataset.apply_incremental_mutations(),
    )?;
    scenario_setup(
        &mut records,
        "reconcile.incremental",
        (|| {
            ensure!(
                mutations.additions == dataset.mutations.additions
                    && mutations.removals == dataset.mutations.removals,
                "dataset mutation helper returned an unexpected mutation set"
            );
            Ok(())
        })(),
    )?;
    let incremental_roots = scenario_setup(
        &mut records,
        "reconcile.incremental",
        prepare_incremental_roots(dataset, &mutations),
    )?;
    let incremental_paths = paths_after_incremental(dataset, &mutations);
    let incremental_expected = incremental_paths.len();
    let unavailable_by_root = scenario_setup(
        &mut records,
        "reconcile.incremental",
        source_ownership_counts(dataset, &mutations.removals),
    )?;
    let addition_counts = scenario_setup(
        &mut records,
        "reconcile.incremental",
        source_ownership_counts(dataset, &mutations.additions),
    )?;
    let incremental_source_counts = scenario_setup(
        &mut records,
        "reconcile.incremental",
        source_counts_after_mutations(
            &initial_source_counts,
            &addition_counts,
            &unavailable_by_root,
        ),
    )?;
    let incremental_paths_for_worker = libraries
        .iter()
        .map(|library| library.database_path.clone())
        .collect::<Vec<_>>();
    let incremental_source_roots = source_roots.clone();
    let incremental_work = incremental_roots.clone();
    let expected_incremental = incremental_roots
        .iter()
        .map(|root| root.additions.len())
        .collect::<Vec<_>>();
    let incremental_samples = run_scenario_with_deadline(
        &mut records,
        "reconcile.incremental",
        policy.scenario_timeout,
        move || {
            let databases = incremental_paths_for_worker
                .iter()
                .map(Database::new)
                .collect::<Result<Vec<_>>>()?;
            measure_each_sample(
                policy.heavy,
                |sample_index| {
                    reconcile_incremental(
                        &databases[sample_index],
                        &incremental_source_roots,
                        &incremental_work,
                    )
                },
                |_, outcomes| {
                    validate_reconciliation_outcomes(
                        outcomes,
                        &expected_incremental,
                        "incremental reconciliation",
                    )?;
                    Ok(())
                },
            )
        },
    )?;
    for library in &libraries {
        let database = scenario_setup(
            &mut records,
            "reconcile.incremental",
            Database::new(&library.database_path),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.incremental",
            validate_available_library(&database, &incremental_paths, &mutations.removals),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.incremental",
            validate_source_ownership(
                &database,
                dataset,
                &incremental_source_counts,
                &unavailable_by_root,
            ),
        )?;
        let fixture = scenario_setup(
            &mut records,
            "reconcile.incremental",
            library.fixture.as_ref().context("missing query fixture"),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.incremental",
            validate_preserved_metadata(&database, fixture, &mutations.removals),
        )?;
    }
    let incremental_database = scenario_setup(
        &mut records,
        "reconcile.incremental",
        Database::new(&libraries[0].database_path),
    )?;
    let incremental_count = scenario_setup(
        &mut records,
        "reconcile.incremental",
        incremental_database.get_wallpapers_page("all", "created", "", 0, 1),
    )?
    .total;
    let incremental_expected_u64 = scenario_setup(
        &mut records,
        "reconcile.incremental",
        usize_to_u64(incremental_expected, "incremental expected count"),
    )?;
    let incremental_actual_u64 = scenario_setup(
        &mut records,
        "reconcile.incremental",
        i64_to_u64(incremental_count, "incremental available count"),
    )?;
    complete_scenario(
        &mut records,
        "reconcile.incremental",
        record_with_database(
            "reconcile.incremental",
            true,
            incremental_samples,
            incremental_expected_u64,
            incremental_actual_u64,
            &libraries[0].database_path,
        ),
    )?;

    let recovered = scenario_setup(
        &mut records,
        "reconcile.recovery",
        dataset.recover_removed(),
    )?;
    scenario_setup(
        &mut records,
        "reconcile.recovery",
        (|| {
            ensure!(
                recovered == mutations.removals,
                "dataset recovery helper restored an unexpected path set"
            );
            Ok(())
        })(),
    )?;
    let recovered_images = scenario_setup(
        &mut records,
        "reconcile.recovery",
        scan_all_sources(&source_roots),
    )?;
    let recovery_paths = dataset
        .absolute_paths
        .iter()
        .chain(mutations.additions.iter())
        .cloned()
        .collect::<Vec<_>>();
    scenario_setup(
        &mut records,
        "reconcile.recovery",
        validate_scanned_paths(&recovered_images, &recovery_paths, "recovery scan"),
    )?;
    let recovery_expected = recovery_paths.len();
    let recovery_sources = scenario_setup(
        &mut records,
        "reconcile.recovery",
        source_ownership_counts(dataset, &recovery_paths),
    )?;
    let recovery_database_paths = libraries
        .iter()
        .map(|library| library.database_path.clone())
        .collect::<Vec<_>>();
    let recovery_roots = source_roots.clone();
    let recovery_images = recovered_images.clone();
    let expected_recovery = recovered_images.iter().map(Vec::len).collect::<Vec<_>>();
    let recovery_samples = run_scenario_with_deadline(
        &mut records,
        "reconcile.recovery",
        policy.scenario_timeout,
        move || {
            let databases = recovery_database_paths
                .iter()
                .map(Database::new)
                .collect::<Result<Vec<_>>>()?;
            measure_each_sample(
                policy.heavy,
                |sample_index| {
                    reconcile_full_snapshot(
                        &databases[sample_index],
                        &recovery_roots,
                        &recovery_images,
                    )
                },
                |_, outcomes| {
                    validate_reconciliation_outcomes(
                        outcomes,
                        &expected_recovery,
                        "recovery reconciliation",
                    )?;
                    Ok(())
                },
            )
        },
    )?;
    for library in &libraries {
        let database = scenario_setup(
            &mut records,
            "reconcile.recovery",
            Database::new(&library.database_path),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.recovery",
            validate_available_library(&database, &recovery_paths, &[]),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.recovery",
            validate_source_ownership(&database, dataset, &recovery_sources, &no_unavailable),
        )?;
        let fixture = scenario_setup(
            &mut records,
            "reconcile.recovery",
            library.fixture.as_ref().context("missing query fixture"),
        )?;
        scenario_setup(
            &mut records,
            "reconcile.recovery",
            validate_preserved_metadata(&database, fixture, &[]),
        )?;
    }
    let recovery_database = scenario_setup(
        &mut records,
        "reconcile.recovery",
        Database::new(&libraries[0].database_path),
    )?;
    let recovery_count = scenario_setup(
        &mut records,
        "reconcile.recovery",
        recovery_database.get_wallpapers_page("all", "created", "", 0, 1),
    )?
    .total;
    let recovery_expected_u64 = scenario_setup(
        &mut records,
        "reconcile.recovery",
        usize_to_u64(recovery_expected, "recovery expected count"),
    )?;
    let recovery_actual_u64 = scenario_setup(
        &mut records,
        "reconcile.recovery",
        i64_to_u64(recovery_count, "recovery available count"),
    )?;
    complete_scenario(
        &mut records,
        "reconcile.recovery",
        record_with_database(
            "reconcile.recovery",
            true,
            recovery_samples,
            recovery_expected_u64,
            recovery_actual_u64,
            &libraries[0].database_path,
        ),
    )?;

    let primary = libraries.remove(0);
    let fixture = scenario_setup(
        &mut records,
        "startup.open-first-page-stats",
        primary.fixture.context("missing primary query fixture"),
    )?;
    let database_path = primary.database_path;
    for path in
        std::iter::once(&database_path).chain(libraries.iter().map(|item| &item.database_path))
    {
        let database = scenario_setup(
            &mut records,
            "startup.open-first-page-stats",
            Database::new(path),
        )?;
        scenario_setup(
            &mut records,
            "startup.open-first-page-stats",
            database.checkpoint_wal(),
        )?;
    }
    drop(libraries);

    let startup_path = database_path.clone();
    let startup_source_roots = dataset.source_roots.clone();
    let startup_fixture = fixture.clone();
    let startup_recovery_sources = recovery_sources.clone();
    let (startup_samples, startup_total) = run_scenario_with_deadline(
        &mut records,
        "startup.open-first-page-stats",
        policy.scenario_timeout,
        move || {
            let mut observed = None;
            let samples = measure_each_sample(
                policy.heavy,
                |_| {
                    let database = Database::new(&startup_path).with_context(|| {
                        format!("reopen startup database {}", startup_path.display())
                    })?;
                    let summaries = database.get_watched_folder_summaries()?;
                    let page = database.get_wallpapers_page("all", "created", "", 0, PAGE_LIMIT)?;
                    let stats = database.get_stats()?;
                    Ok(StartupMeasurement {
                        _database: database,
                        snapshot: StartupSnapshot {
                            summaries,
                            page,
                            stats,
                        },
                    })
                },
                |_, measurement| {
                    validate_startup_snapshot(
                        &startup_source_roots,
                        recovery_expected,
                        &startup_recovery_sources,
                        &startup_fixture,
                        &measurement.snapshot,
                    )?;
                    remember_count(
                        &mut observed,
                        measurement.snapshot.page.total,
                        "startup page total",
                    )
                },
            )?;
            Ok((samples, observed.context("startup produced no page total")?))
        },
    )?;
    let startup_total_u64 = scenario_setup(
        &mut records,
        "startup.open-first-page-stats",
        i64_to_u64(startup_total, "startup page total"),
    )?;
    complete_scenario(
        &mut records,
        "startup.open-first-page-stats",
        record_with_database(
            "startup.open-first-page-stats",
            true,
            startup_samples,
            recovery_expected_u64,
            startup_total_u64,
            &database_path,
        ),
    )?;

    let first_path = database_path.clone();
    let (first_samples, first_total) = run_scenario_with_deadline(
        &mut records,
        "query.page.first",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&first_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| database.get_wallpapers_page("all", "created", "", 0, PAGE_LIMIT),
                |_, page| {
                    validate_page(
                        page,
                        recovery_expected,
                        0,
                        PAGE_LIMIT,
                        recovery_expected > PAGE_LIMIT as usize,
                    )?;
                    remember_count(&mut observed, page.total, "first-page total")
                },
            )?;
            Ok((
                samples,
                observed.context("first-page query produced no total")?,
            ))
        },
    )?;
    let first_total_u64 = scenario_setup(
        &mut records,
        "query.page.first",
        i64_to_u64(first_total, "first-page total"),
    )?;
    complete_scenario(
        &mut records,
        "query.page.first",
        record_with_database(
            "query.page.first",
            true,
            first_samples,
            recovery_expected_u64,
            first_total_u64,
            &database_path,
        ),
    )?;

    let deep_offset = scenario_setup(
        &mut records,
        "query.page.deep",
        usize_to_i64(recovery_expected, "deep page total"),
    )?
    .saturating_sub(PAGE_LIMIT)
    .max(0);
    let deep_path = database_path.clone();
    let (deep_samples, deep_total) = run_scenario_with_deadline(
        &mut records,
        "query.page.deep",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&deep_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| database.get_wallpapers_page("all", "created", "", deep_offset, PAGE_LIMIT),
                |_, page| {
                    validate_page(page, recovery_expected, deep_offset, PAGE_LIMIT, false)?;
                    remember_count(&mut observed, page.total, "deep-page total")
                },
            )?;
            Ok((
                samples,
                observed.context("deep-page query produced no total")?,
            ))
        },
    )?;
    let deep_total_u64 = scenario_setup(
        &mut records,
        "query.page.deep",
        i64_to_u64(deep_total, "deep-page total"),
    )?;
    complete_scenario(
        &mut records,
        "query.page.deep",
        record_with_database(
            "query.page.deep",
            true,
            deep_samples,
            recovery_expected_u64,
            deep_total_u64,
            &database_path,
        ),
    )?;

    let unicode_path = database_path.clone();
    let unicode_fixture = fixture.clone();
    let (unicode_samples, unicode_total) = run_scenario_with_deadline(
        &mut records,
        "query.search.unicode",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&unicode_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| database.get_wallpapers_page("all", "created", TAG_NAME, 0, PAGE_LIMIT),
                |_, page| {
                    validate_membership_page(page, &unicode_fixture.unicode_matches, TAG_NAME)?;
                    remember_count(&mut observed, page.total, "Unicode search total")
                },
            )?;
            validate_complete_query_membership(
                &database,
                "all",
                "created",
                TAG_NAME,
                &unicode_fixture.unicode_matches,
                "Unicode search",
            )?;
            Ok((
                samples,
                observed.context("Unicode search produced no total")?,
            ))
        },
    )?;
    let unicode_expected_u64 = scenario_setup(
        &mut records,
        "query.search.unicode",
        usize_to_u64(
            fixture.unicode_matches.len(),
            "Unicode search expected count",
        ),
    )?;
    let unicode_total_u64 = scenario_setup(
        &mut records,
        "query.search.unicode",
        i64_to_u64(unicode_total, "Unicode search total"),
    )?;
    complete_scenario(
        &mut records,
        "query.search.unicode",
        record_with_database(
            "query.search.unicode",
            true,
            unicode_samples,
            unicode_expected_u64,
            unicode_total_u64,
            &database_path,
        ),
    )?;

    let none_path = database_path.clone();
    let (none_samples, none_total) = run_scenario_with_deadline(
        &mut records,
        "query.search.none",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&none_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| {
                    database.get_wallpapers_page(
                        "all",
                        "created",
                        "purewall-benchmark-no-match-7a1",
                        0,
                        PAGE_LIMIT,
                    )
                },
                |_, page| {
                    validate_page(page, 0, 0, PAGE_LIMIT, false)?;
                    remember_count(&mut observed, page.total, "absent search total")
                },
            )?;
            Ok((
                samples,
                observed.context("absent search produced no total")?,
            ))
        },
    )?;
    let none_total_u64 = scenario_setup(
        &mut records,
        "query.search.none",
        i64_to_u64(none_total, "absent search total"),
    )?;
    complete_scenario(
        &mut records,
        "query.search.none",
        record_with_database(
            "query.search.none",
            true,
            none_samples,
            0,
            none_total_u64,
            &database_path,
        ),
    )?;

    let tag_path = database_path.clone();
    let tag_fixture = fixture.clone();
    let tag_filter = format!("tag:{}", fixture.tag_id);
    let tag_filter_for_worker = tag_filter.clone();
    let (tag_samples, tag_total) = run_scenario_with_deadline(
        &mut records,
        "query.filter.tag",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&tag_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| {
                    database.get_wallpapers_page(&tag_filter_for_worker, "liked", "", 0, PAGE_LIMIT)
                },
                |_, page| {
                    validate_membership_page(page, &tag_fixture.tagged, "tag filter")?;
                    remember_count(&mut observed, page.total, "tag-filter total")
                },
            )?;
            validate_complete_query_membership(
                &database,
                &tag_filter_for_worker,
                "liked",
                "",
                &tag_fixture.tagged,
                "tag filter",
            )?;
            Ok((
                samples,
                observed.context("tag-filter query produced no total")?,
            ))
        },
    )?;
    let tag_expected_u64 = scenario_setup(
        &mut records,
        "query.filter.tag",
        usize_to_u64(fixture.tagged.len(), "tag-filter expected count"),
    )?;
    let tag_total_u64 = scenario_setup(
        &mut records,
        "query.filter.tag",
        i64_to_u64(tag_total, "tag-filter total"),
    )?;
    complete_scenario(
        &mut records,
        "query.filter.tag",
        record_with_database(
            "query.filter.tag",
            true,
            tag_samples,
            tag_expected_u64,
            tag_total_u64,
            &database_path,
        ),
    )?;

    let collection_path = database_path.clone();
    let collection_fixture = fixture.clone();
    let collection_filter = format!("collection:{}", fixture.collection_id);
    let collection_filter_for_worker = collection_filter.clone();
    let (collection_samples, collection_total) = run_scenario_with_deadline(
        &mut records,
        "query.filter.collection",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&collection_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| {
                    database.get_wallpapers_page(
                        &collection_filter_for_worker,
                        "recent",
                        "",
                        0,
                        PAGE_LIMIT,
                    )
                },
                |_, page| {
                    validate_membership_page(
                        page,
                        &collection_fixture.collected,
                        "collection filter",
                    )?;
                    remember_count(&mut observed, page.total, "collection-filter total")
                },
            )?;
            validate_complete_query_membership(
                &database,
                &collection_filter_for_worker,
                "recent",
                "",
                &collection_fixture.collected,
                "collection filter",
            )?;
            Ok((
                samples,
                observed.context("collection-filter query produced no total")?,
            ))
        },
    )?;
    let collection_expected_u64 = scenario_setup(
        &mut records,
        "query.filter.collection",
        usize_to_u64(fixture.collected.len(), "collection-filter expected count"),
    )?;
    let collection_total_u64 = scenario_setup(
        &mut records,
        "query.filter.collection",
        i64_to_u64(collection_total, "collection-filter total"),
    )?;
    complete_scenario(
        &mut records,
        "query.filter.collection",
        record_with_database(
            "query.filter.collection",
            true,
            collection_samples,
            collection_expected_u64,
            collection_total_u64,
            &database_path,
        ),
    )?;

    let plays_path = database_path.clone();
    let plays_fixture = fixture;
    let (plays_samples, plays_total) = run_scenario_with_deadline(
        &mut records,
        "query.sort.plays",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&plays_path)?;
            let mut observed = None;
            let samples = measure_each_sample(
                policy.short,
                |_| database.get_wallpapers_page("all", "plays", "", 0, PAGE_LIMIT),
                |_, page| {
                    validate_plays_page(page, recovery_expected, &plays_fixture)?;
                    remember_count(&mut observed, page.total, "plays-sort total")
                },
            )?;
            database.checkpoint_wal()?;
            Ok((
                samples,
                observed.context("plays-sort query produced no total")?,
            ))
        },
    )?;
    let plays_total_u64 = scenario_setup(
        &mut records,
        "query.sort.plays",
        i64_to_u64(plays_total, "plays-sort total"),
    )?;
    complete_scenario(
        &mut records,
        "query.sort.plays",
        record_with_database(
            "query.sort.plays",
            true,
            plays_samples,
            recovery_expected_u64,
            plays_total_u64,
            &database_path,
        ),
    )?;
    Ok(records)
}

struct ActiveClaimSetup {
    queue: MediaJobQueue,
    snapshot: MediaQueueSnapshot,
}

struct ActiveClaimOutcome {
    queue: MediaJobQueue,
    setup_snapshot: MediaQueueSnapshot,
    enqueue_accepted: bool,
    claimed_job: Option<MediaJob>,
}

pub(crate) fn run_playback_and_media_scenarios(
    dataset: &GeneratedDataset,
    run: &OwnedRunRoot,
    policy: SamplePolicy,
) -> ScenarioRunResult<Vec<ScenarioRecord>> {
    let mut records = Vec::with_capacity(6);
    scenario_setup(
        &mut records,
        "playback.next.single",
        (|| {
            ensure!(
                policy.heavy > 0 && policy.short > 0 && policy.scenario_timeout > Duration::ZERO,
                "sample counts and scenario timeout must be positive"
            );
            validate_dataset_contract(dataset)?;
            ensure!(
                dataset.absolute_paths.len() >= MEDIA_PATH_COUNT,
                "performance media dataset requires at least {MEDIA_PATH_COUNT} images"
            );
            Ok(())
        })(),
    )?;

    let source_roots = scenario_setup(
        &mut records,
        "playback.next.single",
        canonical_source_roots(dataset),
    )?;
    let scanned = scenario_setup(
        &mut records,
        "playback.next.single",
        scan_all_sources(&source_roots),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        validate_full_scan(dataset, &scanned),
    )?;
    let database_path = run.database_dir().join("playback-media.db");
    let database = scenario_setup(
        &mut records,
        "playback.next.single",
        Database::new(&database_path).with_context(|| {
            format!(
                "open isolated playback database {}",
                database_path.display()
            )
        }),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        register_sources(&database, &source_roots),
    )?;
    let expected_imported = scanned.iter().map(Vec::len).collect::<Vec<_>>();
    let outcomes = scenario_setup(
        &mut records,
        "playback.next.single",
        reconcile_full_snapshot(&database, &source_roots, &scanned),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        validate_reconciliation_outcomes(&outcomes, &expected_imported, "playback import"),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        prepare_query_fixture(&database, dataset),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        validate_available_library(&database, &dataset.absolute_paths, &[]),
    )?;
    scenario_setup(
        &mut records,
        "playback.next.single",
        database.checkpoint_wal(),
    )?;
    drop(database);

    let eligible = dataset
        .absolute_paths
        .iter()
        .map(|path| path_text(path, "playback eligible path"))
        .collect::<Result<HashSet<_>>>();
    let eligible = scenario_setup(&mut records, "playback.next.single", eligible)?;
    let total = scenario_setup(
        &mut records,
        "playback.next.single",
        usize_to_u64(eligible.len(), "playback eligible count"),
    )?;
    let (database_bytes, wal_bytes) = scenario_setup(
        &mut records,
        "playback.next.single",
        database_sizes(&database_path),
    )?;

    let single_database_path = database_path.clone();
    let single_eligible = eligible.clone();
    let single_samples = run_scenario_with_deadline(
        &mut records,
        "playback.next.single",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&single_database_path).with_context(|| {
                format!(
                    "open playback single database {}",
                    single_database_path.display()
                )
            })?;
            let (samples, value) = measure_operation_validated(
                policy.short,
                |_| database.get_next_wallpaper(),
                |_, path| validate_selected_path(path, &single_eligible),
            )?;
            drop(value);
            Ok(samples)
        },
    )?;
    let mut single = scenario_setup(
        &mut records,
        "playback.next.single",
        record("playback.next.single", true, single_samples, 1, 1),
    )?;
    single.database_bytes = Some(database_bytes);
    single.wal_bytes = wal_bytes;
    records.push(single);

    let multi_database_path = database_path.clone();
    let multi_eligible = eligible;
    let multi_samples = run_scenario_with_deadline(
        &mut records,
        "playback.next.multi",
        policy.scenario_timeout,
        move || {
            let database = Database::new(&multi_database_path).with_context(|| {
                format!(
                    "open playback multi database {}",
                    multi_database_path.display()
                )
            })?;
            let (samples, value) = measure_operation_validated(
                policy.short,
                |_| database.get_next_wallpapers(4),
                |_, paths| validate_selected_paths(paths, 4, &multi_eligible),
            )?;
            drop(value);
            Ok(samples)
        },
    )?;
    let mut multi = scenario_setup(
        &mut records,
        "playback.next.multi",
        record("playback.next.multi", true, multi_samples, 4, 4),
    )?;
    multi.database_bytes = Some(database_bytes);
    multi.wal_bytes = wal_bytes;
    records.push(multi);

    let media_paths = dataset
        .absolute_paths
        .iter()
        .take(MEDIA_PATH_COUNT)
        .map(|path| path_text(path, "performance media path"))
        .collect::<Result<Vec<_>>>();
    let media_paths = scenario_setup(&mut records, "media.thumbnail.cold", media_paths)?;
    let cache_root = run.cache_dir();

    let cold_paths = media_paths.clone();
    let cold_roots = (0..policy.heavy)
        .map(|index| cache_root.join(format!("thumbnail-cold-{index:04}")))
        .collect::<Vec<_>>();
    let cold_samples = run_scenario_with_deadline(
        &mut records,
        "media.thumbnail.cold",
        policy.scenario_timeout,
        move || {
            let caches = cold_roots
                .iter()
                .map(|root| ThumbnailCache::new(root))
                .collect::<Result<Vec<_>>>()?;
            let (samples, value) = measure_operation_validated(
                policy.heavy,
                |sample_index| {
                    Ok(cold_paths
                        .iter()
                        .map(|path| {
                            caches[sample_index].generate_derivatives(
                                path,
                                MediaNeeds {
                                    thumbnail: true,
                                    preview: false,
                                },
                            )
                        })
                        .collect::<Vec<_>>())
                },
                |_, derivatives| validate_thumbnail_derivatives(derivatives, MEDIA_PATH_COUNT),
            )?;
            drop(value);
            Ok(samples)
        },
    )?;
    let mut cold = scenario_setup(
        &mut records,
        "media.thumbnail.cold",
        record(
            "media.thumbnail.cold",
            true,
            cold_samples,
            MEDIA_PATH_COUNT as u64,
            MEDIA_PATH_COUNT as u64,
        ),
    )?;
    cold.cache_state = CacheState::ColdDerivative;
    cold.cache_hits = Some(0);
    cold.cache_misses = Some(MEDIA_PATH_COUNT as u64);
    cold.generated_derivatives = Some(MEDIA_PATH_COUNT as u64);
    records.push(cold);

    let warm_paths = media_paths.clone();
    let warm_root = cache_root.join("thumbnail-warm");
    let warm_samples = run_scenario_with_deadline(
        &mut records,
        "media.thumbnail.warm",
        policy.scenario_timeout,
        move || {
            let cache = ThumbnailCache::new(&warm_root)?;
            for path in &warm_paths {
                let generated = cache.generate_derivatives(
                    path,
                    MediaNeeds {
                        thumbnail: true,
                        preview: false,
                    },
                );
                validate_nonempty_file(
                    generated.thumbnail.as_deref(),
                    "warm thumbnail precondition",
                )?;
            }
            let (samples, value) = measure_operation_validated(
                policy.short,
                |_| {
                    Ok(warm_paths
                        .iter()
                        .map(|path| cache.cached_path(path))
                        .collect::<Vec<_>>())
                },
                |_, hits| {
                    ensure!(
                        hits.len() == MEDIA_PATH_COUNT,
                        "warm thumbnail lookup count mismatch"
                    );
                    for hit in hits {
                        validate_nonempty_file(hit.as_deref(), "warm thumbnail hit")?;
                    }
                    Ok(())
                },
            )?;
            drop(value);
            Ok(samples)
        },
    )?;
    let mut warm = scenario_setup(
        &mut records,
        "media.thumbnail.warm",
        record(
            "media.thumbnail.warm",
            true,
            warm_samples,
            MEDIA_PATH_COUNT as u64,
            MEDIA_PATH_COUNT as u64,
        ),
    )?;
    warm.cache_state = CacheState::WarmDerivative;
    warm.cache_hits = Some(MEDIA_PATH_COUNT as u64);
    warm.cache_misses = Some(0);
    warm.generated_derivatives = Some(0);
    records.push(warm);

    let active_path = scenario_setup(
        &mut records,
        "media.preview.cold",
        media_paths
            .first()
            .cloned()
            .context("performance media path set is empty"),
    )?;
    let preview_path = active_path.clone();
    let preview_roots = (0..policy.heavy)
        .map(|index| cache_root.join(format!("preview-cold-{index:04}")))
        .collect::<Vec<_>>();
    let preview_samples = run_scenario_with_deadline(
        &mut records,
        "media.preview.cold",
        policy.scenario_timeout,
        move || {
            let caches = preview_roots
                .iter()
                .map(|root| ThumbnailCache::new(root))
                .collect::<Result<Vec<_>>>()?;
            let (samples, value) = measure_operation_validated(
                policy.heavy,
                |sample_index| {
                    Ok(caches[sample_index].generate_derivatives(
                        &preview_path,
                        MediaNeeds {
                            thumbnail: true,
                            preview: true,
                        },
                    ))
                },
                |_, derivatives| {
                    validate_nonempty_file(
                        derivatives.thumbnail.as_deref(),
                        "cold preview thumbnail",
                    )?;
                    validate_nonempty_file(derivatives.preview.as_deref(), "cold preview output")
                },
            )?;
            drop(value);
            Ok(samples)
        },
    )?;
    let mut preview = scenario_setup(
        &mut records,
        "media.preview.cold",
        record("media.preview.cold", true, preview_samples, 2, 2),
    )?;
    preview.cache_state = CacheState::ColdDerivative;
    preview.cache_hits = Some(0);
    preview.cache_misses = Some(2);
    preview.generated_derivatives = Some(2);
    records.push(preview);

    let claim_active_path = active_path;
    let claim_samples = run_scenario_with_deadline(
        &mut records,
        "media.preview.active-claim",
        policy.scenario_timeout,
        move || {
            let mut queue_peak_pending = 0_u64;
            let (samples, value) = measure_operation_with_setup_validated(
                policy.short,
                |_| {
                    let queue = MediaJobQueue::new();
                    for index in 0..ACTIVE_CLAIM_THUMBNAILS {
                        ensure!(
                            queue.enqueue(
                                MediaJobKind::Thumbnail,
                                format!("{claim_active_path}#performance-thumbnail-{index:04}"),
                            ),
                            "active-claim thumbnail enqueue was rejected"
                        );
                    }
                    let snapshot = queue.performance_snapshot();
                    validate_active_claim_setup(snapshot)?;
                    Ok(ActiveClaimSetup { queue, snapshot })
                },
                |_, setup| {
                    let enqueue_accepted =
                        queue_enqueue_active_preview(&setup.queue, &claim_active_path);
                    let claimed_job = setup.queue.try_pop_preview();
                    Ok(ActiveClaimOutcome {
                        queue: setup.queue,
                        setup_snapshot: setup.snapshot,
                        enqueue_accepted,
                        claimed_job,
                    })
                },
                |_, outcome| {
                    let sample_peak = validate_active_claim(outcome, &claim_active_path)?;
                    queue_peak_pending = queue_peak_pending.max(sample_peak);
                    Ok(())
                },
            )?;
            drop(value);
            Ok((samples, queue_peak_pending))
        },
    )?;
    let mut claim = scenario_setup(
        &mut records,
        "media.preview.active-claim",
        record(
            "media.preview.active-claim",
            true,
            claim_samples.0,
            (ACTIVE_CLAIM_THUMBNAILS + 1) as u64,
            (ACTIVE_CLAIM_THUMBNAILS + 1) as u64,
        ),
    )?;
    claim.queue_peak_pending = Some(claim_samples.1);
    records.push(claim);

    let final_validation = (|| {
        ensure!(
            records.iter().all(|record| {
                record.timeout_count == 0
                    && record.status == RunStatus::Passed
                    && record.errors.is_empty()
            }),
            "playback/media scenario reported a failure"
        );
        ensure!(
            total == dataset.manifest.item_count as u64,
            "playback eligible set does not cover the CI dataset"
        );
        Ok(())
    })();
    scenario_setup(&mut records, "media.preview.active-claim", final_validation)?;
    Ok(records)
}

fn validate_selected_path(path: &str, eligible: &HashSet<String>) -> Result<()> {
    ensure!(
        eligible.contains(path),
        "playback selected an ineligible path"
    );
    ensure!(
        Path::new(path).is_file(),
        "playback selected a missing file"
    );
    Ok(())
}

fn validate_selected_paths(
    paths: &[String],
    expected_count: usize,
    eligible: &HashSet<String>,
) -> Result<()> {
    ensure!(
        paths.len() == expected_count,
        "playback selected {} paths instead of {expected_count}",
        paths.len()
    );
    let unique = paths.iter().collect::<HashSet<_>>();
    ensure!(
        unique.len() == expected_count,
        "playback multi-selection repeated a path despite sufficient eligible files"
    );
    for path in paths {
        validate_selected_path(path, eligible)?;
    }
    Ok(())
}

fn validate_thumbnail_derivatives(
    derivatives: &[DerivativePaths],
    expected_count: usize,
) -> Result<()> {
    ensure!(
        derivatives.len() == expected_count,
        "cold thumbnail derivative count mismatch"
    );
    for derivative in derivatives {
        validate_nonempty_file(derivative.thumbnail.as_deref(), "cold thumbnail")?;
        ensure!(
            derivative.preview.is_none(),
            "thumbnail-only generation unexpectedly produced a preview"
        );
    }
    Ok(())
}

fn validate_nonempty_file(path: Option<&Path>, label: &str) -> Result<()> {
    let path = path.with_context(|| format!("{label} path is missing"))?;
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("read {label} metadata at {}", path.display()))?;
    ensure!(metadata.is_file(), "{label} output is not a regular file");
    ensure!(metadata.len() > 0, "{label} output is empty");
    Ok(())
}

fn queue_enqueue_active_preview(queue: &MediaJobQueue, active_path: &str) -> bool {
    queue.enqueue_active_preview(active_path.to_owned())
}

fn validate_active_claim_setup(snapshot: MediaQueueSnapshot) -> Result<()> {
    ensure!(
        snapshot.pending_paths == ACTIVE_CLAIM_THUMBNAILS,
        "active-claim setup path backlog mismatch"
    );
    ensure!(
        snapshot.running_paths == 0,
        "active-claim setup unexpectedly had running work"
    );
    ensure!(
        snapshot.pending_thumbnails == ACTIVE_CLAIM_THUMBNAILS,
        "active-claim setup thumbnail backlog mismatch"
    );
    ensure!(
        snapshot.pending_previews == 0,
        "active-claim setup unexpectedly had pending previews"
    );
    Ok(())
}

fn validate_active_claim(outcome: &ActiveClaimOutcome, active_path: &str) -> Result<u64> {
    ensure!(
        outcome.enqueue_accepted,
        "active preview enqueue was rejected"
    );
    let job = outcome
        .claimed_job
        .as_ref()
        .context("active preview was not claimable from reserved lane")?;
    ensure!(
        job.path == active_path,
        "reserved preview lane claimed the wrong path"
    );
    ensure!(
        job.needs.thumbnail && job.needs.preview,
        "active preview claim did not merge both derivative needs"
    );
    ensure!(
        job.priority == crate::media_queue::MediaPriority::ActivePreview,
        "active preview claim lost its production priority"
    );
    let after_claim = outcome.queue.performance_snapshot();
    ensure!(
        after_claim.pending_paths == ACTIVE_CLAIM_THUMBNAILS
            && after_claim.running_paths == 1
            && after_claim.pending_previews == 0,
        "active preview claim did not transition queue state correctly"
    );
    Ok(outcome
        .setup_snapshot
        .pending_paths
        .max(after_claim.pending_paths) as u64)
}

fn database_sizes(path: &Path) -> Result<(u64, Option<u64>)> {
    let database_bytes = std::fs::metadata(path)
        .with_context(|| format!("read performance database metadata {}", path.display()))?
        .len();
    let mut wal_name = path.as_os_str().to_os_string();
    wal_name.push("-wal");
    let wal_path = PathBuf::from(wal_name);
    let wal_bytes = match std::fs::metadata(&wal_path) {
        Ok(metadata) => Some(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read performance WAL metadata {}", wal_path.display()))
        }
    };
    Ok((database_bytes, wal_bytes))
}

fn validate_dataset_contract(dataset: &GeneratedDataset) -> Result<()> {
    ensure!(
        dataset.source_roots.len() == dataset.manifest.source_count,
        "dataset source-root count does not match its manifest"
    );
    ensure!(
        dataset.absolute_paths.len() == dataset.manifest.item_count,
        "dataset path count does not match its manifest"
    );
    ensure!(
        dataset.source_roots.iter().all(|root| root.is_absolute()),
        "dataset source roots must be explicit absolute paths"
    );
    ensure!(
        dataset.absolute_paths.iter().all(|path| path.is_absolute()),
        "dataset image paths must be explicit absolute paths"
    );
    Ok(())
}

fn canonical_source_roots(dataset: &GeneratedDataset) -> Result<Vec<String>> {
    dataset
        .source_roots
        .iter()
        .map(|root| {
            let canonical = std::fs::canonicalize(root)
                .with_context(|| format!("canonicalize source root {}", root.display()))?;
            path_text(&canonical, "canonical source root")
        })
        .collect()
}

fn scan_all_sources(source_roots: &[String]) -> Result<Vec<Vec<ImageInfo>>> {
    source_roots
        .iter()
        .map(|root| scan_folder(root).with_context(|| format!("scan production source {root}")))
        .collect()
}

fn image_count(images_by_root: &[Vec<ImageInfo>]) -> usize {
    images_by_root.iter().map(Vec::len).sum()
}

fn validate_full_scan(dataset: &GeneratedDataset, images_by_root: &[Vec<ImageInfo>]) -> Result<()> {
    ensure!(
        images_by_root.len() == dataset.source_roots.len(),
        "scan result source count mismatch"
    );
    validate_scanned_paths(images_by_root, &dataset.absolute_paths, "full scan")
}

fn validate_scanned_paths(
    images_by_root: &[Vec<ImageInfo>],
    expected_paths: &[PathBuf],
    label: &str,
) -> Result<()> {
    let scanned = images_by_root
        .iter()
        .flatten()
        .map(|image| path_identity(&image.path))
        .collect::<HashSet<_>>();
    let expected = expected_paths
        .iter()
        .map(|path| path_text(path, label).map(|path| path_identity(&path)))
        .collect::<Result<HashSet<_>>>()?;
    ensure!(
        image_count(images_by_root) == expected_paths.len()
            && scanned.len() == expected_paths.len(),
        "{label} count mismatch: expected {}, got {}",
        expected_paths.len(),
        image_count(images_by_root)
    );
    ensure!(
        scanned == expected,
        "{label} paths differ from the expected set"
    );
    Ok(())
}

fn register_sources(database: &Database, source_roots: &[String]) -> Result<()> {
    for root in source_roots {
        database.upsert_watched_folder(root, SOURCE_LABEL)?;
    }
    Ok(())
}

fn reconcile_full_snapshot(
    database: &Database,
    source_roots: &[String],
    images_by_root: &[Vec<ImageInfo>],
) -> Result<Vec<ReconciliationOutcome>> {
    source_roots
        .iter()
        .zip(images_by_root)
        .map(|(root, images)| {
            database.reconcile_watched_paths_atomically(root, SOURCE_LABEL, &[], images, true)
        })
        .collect()
}

fn validate_reconciliation_outcomes(
    outcomes: &[ReconciliationOutcome],
    expected_scanned: &[usize],
    label: &str,
) -> Result<usize> {
    ensure!(
        outcomes.len() == expected_scanned.len(),
        "{label} source-result count mismatch"
    );
    let mut imported = 0usize;
    for (outcome, expected) in outcomes.iter().zip(expected_scanned) {
        match outcome {
            ReconciliationOutcome::Applied(result) => {
                ensure!(
                    result.scanned == *expected && result.imported == *expected,
                    "{label} returned incorrect production counts"
                );
                imported = imported
                    .checked_add(result.imported)
                    .with_context(|| format!("{label} imported count overflow"))?;
            }
            ReconciliationOutcome::SkippedStale => {
                anyhow::bail!("{label} was rejected as stale")
            }
        }
    }
    Ok(imported)
}

fn remember_count<T: Copy + Eq>(observed: &mut Option<T>, value: T, label: &str) -> Result<()> {
    ensure!(
        observed.is_none_or(|previous| previous == value),
        "{label} changed between samples"
    );
    observed.get_or_insert(value);
    Ok(())
}

fn prepare_incremental_roots(
    dataset: &GeneratedDataset,
    mutations: &super::dataset::MutationSet,
) -> Result<Vec<IncrementalRoot>> {
    let addition_strings = mutations
        .additions
        .iter()
        .map(|path| path_text(path, "incremental addition"))
        .collect::<Result<Vec<_>>>()?;
    let additions = scan_files(&addition_strings)?;
    ensure!(
        additions.len() == mutations.additions.len(),
        "incremental scan did not return every added image"
    );
    let scanned = additions
        .iter()
        .map(|image| path_identity(&image.path))
        .collect::<HashSet<_>>();
    let expected = addition_strings
        .iter()
        .map(|path| path_identity(path))
        .collect::<HashSet<_>>();
    ensure!(
        scanned == expected,
        "incremental scan returned unexpected paths"
    );

    dataset
        .source_roots
        .iter()
        .map(|root| {
            let root_additions = additions
                .iter()
                .filter(|image| path_belongs_to_root(Path::new(&image.path), root))
                .cloned()
                .collect::<Vec<_>>();
            let root_removals = mutations
                .removals
                .iter()
                .filter(|path| path_belongs_to_root(path, root))
                .map(|path| path_text(path, "incremental removal"))
                .collect::<Result<Vec<_>>>()?;
            Ok(IncrementalRoot {
                additions: root_additions,
                removals: root_removals,
            })
        })
        .collect()
}

fn reconcile_incremental(
    database: &Database,
    source_roots: &[String],
    roots: &[IncrementalRoot],
) -> Result<Vec<ReconciliationOutcome>> {
    source_roots
        .iter()
        .zip(roots)
        .map(|(source_root, root)| {
            database.reconcile_watched_paths_atomically(
                source_root,
                SOURCE_LABEL,
                &root.removals,
                &root.additions,
                false,
            )
        })
        .collect()
}

fn paths_after_incremental(
    dataset: &GeneratedDataset,
    mutations: &super::dataset::MutationSet,
) -> Vec<PathBuf> {
    let removals = mutations.removals.iter().collect::<HashSet<_>>();
    dataset
        .absolute_paths
        .iter()
        .filter(|path| !removals.contains(path))
        .chain(mutations.additions.iter())
        .cloned()
        .collect()
}

fn prepare_query_fixture(database: &Database, dataset: &GeneratedDataset) -> Result<QueryFixture> {
    let tag = database.create_tag(TAG_NAME, DEFAULT_TAG_COLOR)?;
    let collection = database.create_collection(COLLECTION_NAME, "#7c3aed")?;
    let tagged = dataset
        .absolute_paths
        .iter()
        .step_by(5)
        .map(|path| path_text(path, "tagged fixture path"))
        .collect::<Result<Vec<_>>>()?;
    let collected = dataset
        .absolute_paths
        .iter()
        .step_by(7)
        .map(|path| path_text(path, "collection fixture path"))
        .collect::<Result<Vec<_>>>()?;
    let liked = dataset
        .absolute_paths
        .iter()
        .step_by(4)
        .map(|path| path_text(path, "liked fixture path"))
        .collect::<Result<Vec<_>>>()?;

    ensure!(
        database.batch_assign_tag(&tagged, tag.id)? == tagged.len(),
        "tag setup did not assign every path"
    );
    ensure!(
        database.batch_assign_collection(&collected, collection.id)? == collected.len(),
        "collection setup did not assign every path"
    );
    ensure!(
        database.batch_set_rating(&liked, 1)? == liked.len(),
        "rating setup did not update every path"
    );

    let tagged_set = tagged.into_iter().collect::<HashSet<_>>();
    let unicode_matches = dataset
        .absolute_paths
        .iter()
        .chain(dataset.mutations.additions.iter())
        .map(|path| path_text(path, "Unicode search fixture path"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|path| path.contains(TAG_NAME) || tagged_set.contains(path))
        .collect();
    let removal_identities = path_identities(&dataset.mutations.removals, "play removal path")?;
    let played = dataset
        .absolute_paths
        .iter()
        .map(|path| path_text(path, "played fixture path"))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|path| !removal_identities.contains(&path_identity(path)))
        .take(2)
        .zip([3, 1])
        .collect::<Vec<_>>();
    ensure!(played.len() == 2, "query fixture requires two stable paths");
    for (path, play_count) in &played {
        for _ in 0..*play_count {
            database.record_play(path)?;
        }
    }

    Ok(QueryFixture {
        tag_id: tag.id,
        collection_id: collection.id,
        tagged: tagged_set,
        collected: collected.into_iter().collect(),
        liked: liked.into_iter().collect(),
        unicode_matches,
        played,
    })
}

fn validate_startup_snapshot(
    source_roots: &[PathBuf],
    expected_total: usize,
    expected_source_counts: &[usize],
    fixture: &QueryFixture,
    startup: &StartupSnapshot,
) -> Result<()> {
    validate_source_summaries(
        &startup.summaries,
        source_roots,
        expected_source_counts,
        &vec![0; source_roots.len()],
    )?;
    let available_count = startup
        .summaries
        .iter()
        .map(|summary| summary.available_count)
        .sum::<usize>();
    let unavailable_count = startup
        .summaries
        .iter()
        .map(|summary| summary.unavailable_count)
        .sum::<usize>();
    ensure!(
        available_count == expected_total && unavailable_count == 0,
        "startup source summaries contain incorrect availability counts"
    );
    validate_page(
        &startup.page,
        expected_total,
        0,
        PAGE_LIMIT,
        expected_total > PAGE_LIMIT as usize,
    )?;
    ensure!(
        startup.stats.total == usize_to_i64(expected_total, "startup stats total")?,
        "startup stats total mismatch"
    );
    ensure!(
        startup.stats.liked == usize_to_i64(fixture.liked.len(), "startup liked count")?,
        "startup liked count mismatch"
    );
    ensure!(
        startup.stats.disliked == 0 && startup.stats.blacklisted == 0,
        "startup stats contain unexpected hidden ratings"
    );
    let expected_plays = fixture
        .played
        .iter()
        .map(|(_, count)| i64::from(*count))
        .sum::<i64>();
    ensure!(
        startup.stats.total_plays == expected_plays,
        "startup play count mismatch"
    );
    Ok(())
}

fn validate_available_library(
    database: &Database,
    expected_available: &[PathBuf],
    expected_unavailable: &[PathBuf],
) -> Result<()> {
    let entries = collect_query_entries(database, "all", "created", "")?;
    let actual = entries
        .iter()
        .map(|entry| path_identity(&entry.path))
        .collect::<HashSet<_>>();
    let expected = path_identities(expected_available, "available library path")?;
    ensure!(
        actual.len() == entries.len(),
        "available library contains duplicate paths"
    );
    ensure!(
        actual == expected,
        "available library paths differ from the expected set"
    );
    for path in expected_unavailable {
        ensure!(
            !path.is_file(),
            "expected unavailable path still exists: {}",
            path.display()
        );
        let path = path_text(path, "unavailable library path")?;
        let entry = database
            .get_wallpaper_by_path(&path)?
            .with_context(|| format!("unavailable row disappeared: {path}"))?;
        ensure!(
            entry.source == SOURCE_LABEL && !actual.contains(&path_identity(&entry.path)),
            "unavailable row has incorrect source or remained query-visible"
        );
    }
    Ok(())
}

fn validate_source_ownership(
    database: &Database,
    dataset: &GeneratedDataset,
    expected_available: &[usize],
    expected_unavailable: &[usize],
) -> Result<()> {
    ensure!(
        expected_available.len() == dataset.source_roots.len()
            && expected_unavailable.len() == dataset.source_roots.len(),
        "source ownership expectation length mismatch"
    );
    let summaries = database.get_watched_folder_summaries()?;
    validate_source_summaries(
        &summaries,
        &dataset.source_roots,
        expected_available,
        expected_unavailable,
    )
}

fn validate_source_summaries(
    summaries: &[WatchedFolderSummary],
    source_roots: &[PathBuf],
    expected_available: &[usize],
    expected_unavailable: &[usize],
) -> Result<()> {
    ensure!(
        expected_available.len() == source_roots.len()
            && expected_unavailable.len() == source_roots.len(),
        "source summary expectation length mismatch"
    );
    ensure!(
        summaries.len() == source_roots.len(),
        "watched source summary count mismatch"
    );
    for (index, root) in source_roots.iter().enumerate() {
        let root_text = path_text(root, "source ownership root")?;
        let summary = summaries
            .iter()
            .find(|summary| path_identity(&summary.entry.path) == path_identity(&root_text))
            .with_context(|| format!("missing source summary for {}", root.display()))?;
        ensure!(
            summary.entry.source == SOURCE_LABEL
                && summary.available_count == expected_available[index]
                && summary.unavailable_count == expected_unavailable[index],
            "source ownership counts differ for {}",
            root.display()
        );
    }
    Ok(())
}

fn validate_preserved_metadata(
    database: &Database,
    expected: &QueryFixture,
    unavailable: &[PathBuf],
) -> Result<()> {
    let unavailable_identities = path_identities(unavailable, "unavailable metadata path")?;
    let visible_liked = visible_members(&expected.liked, &unavailable_identities);
    let liked = collect_query_entries(database, "liked", "liked", "")?;
    let liked_paths = liked
        .iter()
        .map(|entry| path_identity(&entry.path))
        .collect::<HashSet<_>>();
    ensure!(
        liked_paths == path_identities_from_strings(&visible_liked)
            && liked.iter().all(|entry| entry.rating == 1),
        "ratings were not preserved"
    );
    let visible_tagged = visible_members(&expected.tagged, &unavailable_identities);
    validate_complete_query_membership(
        database,
        &format!("tag:{}", expected.tag_id),
        "created",
        "",
        &visible_tagged,
        "preserved tag",
    )?;
    let visible_collected = visible_members(&expected.collected, &unavailable_identities);
    validate_complete_query_membership(
        database,
        &format!("collection:{}", expected.collection_id),
        "created",
        "",
        &visible_collected,
        "preserved collection",
    )?;
    let collection = database
        .get_collections()?
        .into_iter()
        .find(|collection| collection.id == expected.collection_id)
        .context("preserved collection disappeared")?;
    ensure!(
        collection.wallpaper_count
            == usize_to_i64(visible_collected.len(), "preserved collection count")?,
        "collection membership count was not preserved"
    );

    for path in unavailable {
        let path = path_text(path, "unavailable metadata row")?;
        let identity = path_identity(&path);
        let entry = database
            .get_wallpaper_by_path(&path)?
            .with_context(|| format!("metadata row disappeared: {path}"))?;
        if expected
            .liked
            .iter()
            .any(|liked| path_identity(liked) == identity)
        {
            ensure!(entry.rating == 1, "rating was lost from unavailable row");
        }
        if expected
            .tagged
            .iter()
            .any(|tagged| path_identity(tagged) == identity)
        {
            ensure!(
                entry.tags.iter().any(|tag| tag.id == expected.tag_id),
                "tag was lost from unavailable row"
            );
        }
    }
    for (path, expected_count) in &expected.played {
        let entry = database
            .get_wallpaper_by_path(path)?
            .with_context(|| format!("played metadata row disappeared: {path}"))?;
        ensure!(
            entry.play_count == *expected_count,
            "play count was not preserved"
        );
    }
    Ok(())
}

fn visible_members(members: &HashSet<String>, unavailable: &HashSet<String>) -> HashSet<String> {
    members
        .iter()
        .filter(|path| !unavailable.contains(&path_identity(path)))
        .cloned()
        .collect()
}

fn validate_complete_query_membership(
    database: &Database,
    filter: &str,
    sort: &str,
    search: &str,
    expected_paths: &HashSet<String>,
    label: &str,
) -> Result<()> {
    let entries = collect_query_entries(database, filter, sort, search)?;
    let actual = entries
        .iter()
        .map(|entry| path_identity(&entry.path))
        .collect::<HashSet<_>>();
    ensure!(
        actual.len() == entries.len(),
        "{label} returned duplicate paths"
    );
    ensure!(
        actual == path_identities_from_strings(expected_paths),
        "{label} membership mismatch"
    );
    Ok(())
}

fn collect_query_entries(
    database: &Database,
    filter: &str,
    sort: &str,
    search: &str,
) -> Result<Vec<WallpaperEntry>> {
    let mut entries = Vec::new();
    let mut expected_total = None;
    loop {
        let offset = usize_to_i64(entries.len(), "validation page offset")?;
        let page = database.get_wallpapers_page(filter, sort, search, offset, PAGE_LIMIT)?;
        let total = *expected_total.get_or_insert(page.total);
        ensure!(
            page.total == total && page.offset == offset && page.limit == PAGE_LIMIT,
            "validation page metadata changed during pagination"
        );
        validate_existing_items(&page.items)?;
        let returned = page.items.len();
        entries.extend(page.items);
        let has_more = usize_to_i64(entries.len(), "validation page size")? < total;
        ensure!(
            page.has_more == has_more,
            "validation page has_more mismatch"
        );
        if !has_more {
            break;
        }
        ensure!(returned > 0, "validation pagination made no progress");
    }
    ensure!(
        usize_to_i64(entries.len(), "validation result size")?
            == expected_total.unwrap_or_default(),
        "validation result total mismatch"
    );
    Ok(entries)
}

fn validate_page(
    page: &WallpaperPage,
    expected_total: usize,
    expected_offset: i64,
    expected_limit: i64,
    expected_has_more: bool,
) -> Result<()> {
    ensure!(
        page.total == usize_to_i64(expected_total, "page total")?
            && page.offset == expected_offset
            && page.limit == expected_limit
            && page.has_more == expected_has_more,
        "page metadata mismatch"
    );
    let remaining = usize_to_i64(expected_total, "page expected item count")?
        .saturating_sub(expected_offset)
        .max(0);
    let expected_items = remaining.min(expected_limit) as usize;
    ensure!(
        page.items.len() == expected_items,
        "page item count mismatch: expected {expected_items}, got {}",
        page.items.len()
    );
    validate_existing_items(&page.items)
}

fn validate_plays_page(
    page: &WallpaperPage,
    expected_total: usize,
    fixture: &QueryFixture,
) -> Result<()> {
    validate_page(
        page,
        expected_total,
        0,
        PAGE_LIMIT,
        expected_total > PAGE_LIMIT as usize,
    )?;
    ensure!(
        page.items
            .windows(2)
            .all(|pair| pair[0].play_count >= pair[1].play_count),
        "plays sort is not descending"
    );
    for (entry, (expected_path, expected_count)) in page.items.iter().zip(&fixture.played) {
        ensure!(
            path_identity(&entry.path) == path_identity(expected_path)
                && entry.play_count == *expected_count,
            "plays sort did not return the deterministic played paths first"
        );
    }
    ensure!(
        page.items
            .iter()
            .skip(fixture.played.len())
            .all(|entry| entry.play_count == 0),
        "plays sort returned unexpected play history"
    );
    Ok(())
}

fn validate_membership_page(
    page: &WallpaperPage,
    expected_paths: &HashSet<String>,
    label: &str,
) -> Result<()> {
    ensure!(
        page.total == usize_to_i64(expected_paths.len(), label)?,
        "{label} total mismatch"
    );
    ensure!(
        page.items.len() == expected_paths.len().min(PAGE_LIMIT as usize),
        "{label} page size mismatch"
    );
    ensure!(
        page.offset == 0
            && page.limit == PAGE_LIMIT
            && page.has_more == (expected_paths.len() > PAGE_LIMIT as usize),
        "{label} page metadata mismatch"
    );
    let expected_identities = path_identities_from_strings(expected_paths);
    ensure!(
        page.items
            .iter()
            .all(|entry| expected_identities.contains(&path_identity(&entry.path))),
        "{label} returned a non-member path"
    );
    validate_existing_items(&page.items)
}

fn validate_existing_items(items: &[WallpaperEntry]) -> Result<()> {
    ensure!(
        items
            .iter()
            .all(|entry| Path::new(&entry.path).is_file() && entry.source == SOURCE_LABEL),
        "page returned a missing path or incorrect source ownership"
    );
    Ok(())
}

fn source_ownership_counts(dataset: &GeneratedDataset, paths: &[PathBuf]) -> Result<Vec<usize>> {
    let mut counts = vec![0usize; dataset.source_roots.len()];
    for path in paths {
        let source_index = dataset
            .source_roots
            .iter()
            .position(|root| path_belongs_to_root(path, root))
            .with_context(|| format!("path is outside dataset sources: {}", path.display()))?;
        counts[source_index] = counts[source_index]
            .checked_add(1)
            .context("source ownership count overflow")?;
    }
    Ok(counts)
}

fn source_counts_after_mutations(
    initial: &[usize],
    additions: &[usize],
    removals: &[usize],
) -> Result<Vec<usize>> {
    ensure!(
        initial.len() == additions.len() && initial.len() == removals.len(),
        "source mutation count vectors differ in length"
    );
    initial
        .iter()
        .zip(additions)
        .zip(removals)
        .map(|((initial, additions), removals)| {
            initial
                .checked_add(*additions)
                .and_then(|value| value.checked_sub(*removals))
                .context("source mutation count overflow")
        })
        .collect()
}

fn path_belongs_to_root(path: &Path, root: &Path) -> bool {
    crate::paths::path_is_same_or_descendant(path, root)
}

fn path_text(path: &Path, label: &str) -> Result<String> {
    path.to_str()
        .map(str::to_owned)
        .with_context(|| format!("{label} is not valid UTF-8: {}", path.display()))
}

fn path_identities(paths: &[PathBuf], label: &str) -> Result<HashSet<String>> {
    paths
        .iter()
        .map(|path| path_text(path, label).map(|path| path_identity(&path)))
        .collect()
}

fn path_identities_from_strings(paths: &HashSet<String>) -> HashSet<String> {
    paths.iter().map(|path| path_identity(path)).collect()
}

#[cfg(windows)]
fn path_identity(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    let normalized = normalized
        .strip_prefix("\\\\?\\UNC\\")
        .map(|rest| format!("\\\\{rest}"))
        .or_else(|| normalized.strip_prefix("\\\\?\\").map(ToOwned::to_owned))
        .unwrap_or(normalized);
    normalized.trim_end_matches('\\').to_ascii_lowercase()
}

#[cfg(not(windows))]
fn path_identity(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn record(
    id: &str,
    critical: bool,
    samples: ScenarioSamples,
    expected_count: u64,
    actual_count: u64,
) -> Result<ScenarioRecord> {
    Ok(ScenarioRecord {
        id: id.to_string(),
        critical,
        selected_hotspot: false,
        cache_state: CacheState::NotApplicable,
        status: RunStatus::Passed,
        samples,
        expected_count: Some(expected_count),
        actual_count: Some(actual_count),
        database_bytes: None,
        wal_bytes: None,
        cache_hits: None,
        cache_misses: None,
        generated_derivatives: None,
        queue_peak_pending: None,
        timeout_count: 0,
        errors: Vec::new(),
    })
}

fn record_with_database(
    id: &str,
    critical: bool,
    samples: ScenarioSamples,
    expected_count: u64,
    actual_count: u64,
    database_path: &Path,
) -> Result<ScenarioRecord> {
    let mut record = record(id, critical, samples, expected_count, actual_count)?;
    let (database_bytes, wal_bytes) = database_sizes(database_path)?;
    record.database_bytes = Some(database_bytes);
    record.wal_bytes = wal_bytes;
    Ok(record)
}

fn usize_to_u64(value: usize, label: &str) -> Result<u64> {
    value
        .try_into()
        .with_context(|| format!("{label} does not fit in u64"))
}

fn usize_to_i64(value: usize, label: &str) -> Result<i64> {
    value
        .try_into()
        .with_context(|| format!("{label} does not fit in i64"))
}

fn i64_to_u64(value: i64, label: &str) -> Result<u64> {
    value
        .try_into()
        .with_context(|| format!("{label} is negative or too large"))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        measure_each_sample, run_library_scenarios, run_scenario_with_deadline, SamplePolicy,
        ScenarioFailureKind,
    };
    use crate::performance_harness::dataset::{generate_dataset, DatasetScale, DATASET_SEED};
    use crate::performance_harness::owned_temp::OwnedRunRoot;
    use crate::performance_harness::protocol::RunStatus;

    #[test]
    fn scenario_timeout_preserves_completed_records_and_stops_the_sequence() {
        let completed = super::record(
            "completed",
            true,
            crate::performance_harness::protocol::ScenarioSamples::from_elapsed_micros(vec![1], 1)
                .unwrap(),
            1,
            1,
        )
        .unwrap();
        let later_ran = Arc::new(AtomicBool::new(false));

        let failure = (|| -> super::ScenarioRunResult<()> {
            let mut records = vec![completed.clone()];
            run_scenario_with_deadline(
                &mut records,
                "timed-out",
                Duration::from_millis(10),
                || {
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(())
                },
            )?;
            later_ran.store(true, Ordering::Release);
            Ok(())
        })()
        .unwrap_err();

        assert_eq!(failure.scenario_id, "timed-out");
        assert_eq!(
            failure.kind,
            ScenarioFailureKind::Timeout {
                timeout: Duration::from_millis(10)
            }
        );
        assert_eq!(
            failure.error.to_string(),
            "performance scenario timed out after 10 ms"
        );
        assert_eq!(failure.completed_records, vec![completed]);
        assert!(!later_ran.load(Ordering::Acquire));
    }

    #[test]
    fn scenario_error_preserves_its_id_original_error_and_completed_records() {
        let completed = super::record(
            "completed",
            true,
            crate::performance_harness::protocol::ScenarioSamples::from_elapsed_micros(vec![1], 1)
                .unwrap(),
            1,
            1,
        )
        .unwrap();
        let mut records = vec![completed.clone()];

        let failure = run_scenario_with_deadline(
            &mut records,
            "failed-scenario",
            Duration::from_secs(1),
            || -> anyhow::Result<()> { anyhow::bail!("sentinel production error") },
        )
        .unwrap_err();

        assert_eq!(failure.scenario_id, "failed-scenario");
        assert_eq!(failure.kind, ScenarioFailureKind::Error);
        assert!(failure
            .error
            .chain()
            .any(|source| source.to_string() == "sentinel production error"));
        assert_eq!(failure.completed_records, vec![completed]);
    }

    #[test]
    fn ci_playback_and_media_scenarios_are_safe_and_bounded() {
        let run = OwnedRunRoot::create_for_test("playback-media").unwrap();
        let dataset = generate_dataset(&run, DatasetScale::Ci, DATASET_SEED).unwrap();
        let records =
            super::run_playback_and_media_scenarios(&dataset, &run, SamplePolicy::ci()).unwrap();
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "playback.next.single",
                "playback.next.multi",
                "media.thumbnail.cold",
                "media.thumbnail.warm",
                "media.preview.cold",
                "media.preview.active-claim",
            ]
        );
        assert!(records
            .iter()
            .all(|record| record.status == RunStatus::Passed));
        assert!(records.iter().all(|record| record.errors.is_empty()));
        assert!(records
            .iter()
            .filter_map(|record| record.queue_peak_pending)
            .all(|peak| peak <= 257));
        assert!(records
            .iter()
            .all(|record| record.samples.peak_working_set_bytes > 0));
        run.cleanup().unwrap();
    }

    #[test]
    fn ci_library_scenarios_use_production_paths_and_preserve_counts() {
        let run = OwnedRunRoot::create_for_test("library-scenarios").unwrap();
        let dataset = generate_dataset(&run, DatasetScale::Ci, DATASET_SEED).unwrap();
        let records = run_library_scenarios(&dataset, &run, SamplePolicy::ci()).unwrap();
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "scan.full",
                "import.atomic",
                "reconcile.no-change",
                "reconcile.incremental",
                "reconcile.recovery",
                "startup.open-first-page-stats",
                "query.page.first",
                "query.page.deep",
                "query.search.unicode",
                "query.search.none",
                "query.filter.tag",
                "query.filter.collection",
                "query.sort.plays",
            ]
        );
        let by_id = records
            .iter()
            .map(|record| (record.id.as_str(), record))
            .collect::<HashMap<_, _>>();
        assert_eq!(by_id["scan.full"].actual_count, Some(120));
        assert_eq!(by_id["import.atomic"].actual_count, Some(120));
        assert_eq!(by_id["query.page.first"].status, RunStatus::Passed);
        assert_eq!(by_id["query.search.unicode"].status, RunStatus::Passed);
        assert_eq!(by_id["query.filter.tag"].status, RunStatus::Passed);
        assert_eq!(by_id["query.filter.collection"].status, RunStatus::Passed);
        for (id, expected) in [
            ("scan.full", 120),
            ("import.atomic", 120),
            ("reconcile.no-change", 120),
            ("reconcile.incremental", 120),
            ("reconcile.recovery", 124),
            ("startup.open-first-page-stats", 124),
            ("query.page.first", 124),
            ("query.page.deep", 124),
            ("query.search.unicode", 74),
            ("query.search.none", 0),
            ("query.filter.tag", 24),
            ("query.filter.collection", 18),
            ("query.sort.plays", 124),
        ] {
            assert_eq!(by_id[id].expected_count, Some(expected), "{id}");
            assert_eq!(by_id[id].actual_count, Some(expected), "{id}");
        }
        for id in [
            "scan.full",
            "import.atomic",
            "reconcile.no-change",
            "reconcile.incremental",
            "reconcile.recovery",
            "startup.open-first-page-stats",
        ] {
            assert_eq!(by_id[id].samples.elapsed_micros.len(), 1, "{id}");
        }
        for id in [
            "query.page.first",
            "query.page.deep",
            "query.search.unicode",
            "query.search.none",
            "query.filter.tag",
            "query.filter.collection",
            "query.sort.plays",
        ] {
            assert_eq!(by_id[id].samples.elapsed_micros.len(), 3, "{id}");
        }
        assert!(records.iter().all(|record| record.critical));
        assert!(records.iter().all(|record| record.errors.is_empty()));
        assert!(records
            .iter()
            .all(|record| record.samples.peak_working_set_bytes > 0));
        assert!(records
            .iter()
            .filter(|record| record.id != "scan.full")
            .all(|record| record.database_bytes.is_some_and(|bytes| bytes > 0)));
        run.cleanup().unwrap();
    }

    #[test]
    fn sample_policy_and_working_set_metrics_keep_the_declared_sample_contract() {
        let ci = SamplePolicy::ci();
        assert_eq!(ci.heavy, 1);
        assert_eq!(ci.short, 3);
        assert_eq!(ci.scenario_timeout.as_secs(), 60);

        let standard = SamplePolicy::standard();
        assert_eq!(standard.heavy, 3);
        assert_eq!(standard.short, 20);
        assert_eq!(standard.scenario_timeout.as_secs(), 15 * 60);

        let consumed = Cell::new(0);
        let samples = measure_each_sample(3, Ok, |index, value| {
            assert_eq!(*value, index);
            consumed.set(consumed.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(samples.elapsed_micros.len(), 3);
        assert_eq!(consumed.get(), 3);
        assert!(measure_each_sample::<()>(0, |_| Ok(()), |_, _| Ok(())).is_err());
    }

    #[test]
    fn every_sample_is_validated_and_released_before_the_next_operation() {
        struct SampleProbe<'a> {
            valid: bool,
            drops: &'a Cell<usize>,
        }

        impl Drop for SampleProbe<'_> {
            fn drop(&mut self) {
                self.drops.set(self.drops.get() + 1);
            }
        }

        let operations = Cell::new(0);
        let validations = Cell::new(0);
        let drops = Cell::new(0);
        let error = measure_each_sample(
            3,
            |index| {
                assert_eq!(drops.get(), index);
                operations.set(operations.get() + 1);
                Ok(SampleProbe {
                    valid: index != 1,
                    drops: &drops,
                })
            },
            |index, sample| {
                validations.set(validations.get() + 1);
                anyhow::ensure!(sample.valid, "sample {index} is invalid");
                Ok(())
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("sample 1 is invalid"));
        assert_eq!(operations.get(), 2);
        assert_eq!(validations.get(), 2);
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn query_fixture_assigns_nonzero_distinct_play_counts() {
        let run = OwnedRunRoot::create_for_test("library-play-counts").unwrap();
        let result = (|| -> anyhow::Result<(i32, i32)> {
            let dataset = generate_dataset(&run, DatasetScale::Ci, DATASET_SEED)?;
            let source_roots = super::canonical_source_roots(&dataset)?;
            let images = super::scan_all_sources(&source_roots)?;
            let database_path = run.database_dir().join("play-counts.db");
            let database = crate::db::Database::new(&database_path)?;
            super::register_sources(&database, &source_roots)?;
            let outcomes = super::reconcile_full_snapshot(&database, &source_roots, &images)?;
            super::validate_reconciliation_outcomes(
                &outcomes,
                &images.iter().map(Vec::len).collect::<Vec<_>>(),
                "play-count fixture import",
            )?;
            super::prepare_query_fixture(&database, &dataset)?;

            let page = database.get_wallpapers_page("all", "plays", "", 0, 2)?;
            anyhow::ensure!(
                page.items.len() == 2,
                "play-count fixture page is incomplete"
            );
            Ok((page.items[0].play_count, page.items[1].play_count))
        })();
        run.cleanup().unwrap();

        let (highest, next) = result.unwrap();
        assert!(highest > next, "play counts must be different");
        assert!(next > 0, "at least two play counts must be nonzero");
    }
}
