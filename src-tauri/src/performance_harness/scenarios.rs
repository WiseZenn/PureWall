use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{ensure, Context, Result};

use super::dataset::GeneratedDataset;
use super::owned_temp::OwnedRunRoot;
use super::protocol::{CacheState, RunStatus, ScenarioRecord, ScenarioSamples};
use crate::db::{
    Database, Stats, WallpaperEntry, WallpaperPage, WatchedFolderSummary, DEFAULT_TAG_COLOR,
};
use crate::scanner::{scan_files, scan_folder, ImageInfo};
use crate::ReconciliationOutcome;

const SOURCE_LABEL: &str = "mounted";
const PAGE_LIMIT: i64 = 120;
const TAG_NAME: &str = "风景";
const COLLECTION_NAME: &str = "Benchmark Collection";

#[derive(Debug, Clone, Copy)]
pub(crate) struct SamplePolicy {
    pub(crate) heavy: usize,
    pub(crate) short: usize,
    pub(crate) scenario_timeout: Duration,
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
) -> Result<Vec<u64>> {
    ensure!(count > 0, "sample count must be positive");
    let mut samples = Vec::with_capacity(count);
    for sample_index in 0..count {
        let started = std::time::Instant::now();
        let value = operation(sample_index)?;
        samples.push(started.elapsed().as_micros().try_into().unwrap_or(u64::MAX));
        consume(sample_index, &value)?;
        drop(value);
    }
    Ok(samples)
}

struct LibrarySample {
    database_path: PathBuf,
    database: Database,
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
) -> Result<Vec<ScenarioRecord>> {
    ensure!(
        policy.heavy > 0 && policy.short > 0 && policy.scenario_timeout > Duration::ZERO,
        "sample counts and scenario timeout must be positive"
    );
    validate_dataset_contract(dataset)?;

    let total = dataset.manifest.item_count;
    let total_u64 = usize_to_u64(total, "dataset item count")?;
    let mut records = Vec::with_capacity(13);
    let source_roots = canonical_source_roots(dataset)?;

    let mut scanned_count = None;
    let scan_samples = measure_each_sample(
        policy.heavy,
        |_| scan_all_sources(&source_roots),
        |_, images| {
            validate_full_scan(dataset, images)?;
            remember_count(
                &mut scanned_count,
                image_count(images),
                "full scan image count",
            )
        },
    )?;
    let scanned_count = scanned_count.context("full scan produced no count")?;
    records.push(record(
        "scan.full",
        true,
        scan_samples,
        total_u64,
        usize_to_u64(scanned_count, "scanned image count")?,
    )?);

    let scanned_by_root = scan_all_sources(&source_roots)?;
    validate_full_scan(dataset, &scanned_by_root)?;
    let database_dir = run.database_dir();
    let mut libraries = Vec::with_capacity(policy.heavy);
    for sample_index in 0..policy.heavy {
        let path = database_dir.join(format!("import-atomic-{sample_index:04}.db"));
        let database = Database::new(&path)
            .with_context(|| format!("open isolated import database {}", path.display()))?;
        register_sources(&database, &source_roots)?;
        libraries.push(LibrarySample {
            database_path: path,
            database,
            fixture: None,
        });
    }
    let expected_imported = scanned_by_root.iter().map(Vec::len).collect::<Vec<_>>();
    let mut imported_count = None;
    let import_samples = measure_each_sample(
        policy.heavy,
        |sample_index| {
            reconcile_full_snapshot(
                &libraries[sample_index].database,
                &source_roots,
                &scanned_by_root,
            )
        },
        |_, outcomes| {
            let imported =
                validate_reconciliation_outcomes(outcomes, &expected_imported, "atomic import")?;
            remember_count(&mut imported_count, imported, "atomic import count")
        },
    )?;
    let imported_count = imported_count.context("atomic import produced no count")?;
    let initial_source_counts = source_ownership_counts(dataset, &dataset.absolute_paths)?;
    let no_unavailable = vec![0; dataset.source_roots.len()];
    for library in &libraries {
        validate_available_library(&library.database, &dataset.absolute_paths, &[])?;
        validate_source_ownership(
            &library.database,
            dataset,
            &initial_source_counts,
            &no_unavailable,
        )?;
    }
    records.push(record(
        "import.atomic",
        true,
        import_samples,
        total_u64,
        usize_to_u64(imported_count, "imported image count")?,
    )?);

    for library in &mut libraries {
        library.fixture = Some(prepare_query_fixture(&library.database, dataset)?);
    }

    let no_change_samples = measure_each_sample(
        policy.heavy,
        |sample_index| {
            let library = &libraries[sample_index % libraries.len()];
            reconcile_full_snapshot(&library.database, &source_roots, &scanned_by_root)
        },
        |_, outcomes| {
            validate_reconciliation_outcomes(
                outcomes,
                &expected_imported,
                "no-change reconciliation",
            )?;
            Ok(())
        },
    )?;
    for library in &libraries {
        validate_available_library(&library.database, &dataset.absolute_paths, &[])?;
        validate_source_ownership(
            &library.database,
            dataset,
            &initial_source_counts,
            &no_unavailable,
        )?;
        validate_preserved_metadata(
            &library.database,
            library.fixture.as_ref().context("missing query fixture")?,
            &[],
        )?;
    }
    let no_change_count = libraries[0]
        .database
        .get_wallpapers_page("all", "created", "", 0, 1)?
        .total;
    records.push(record(
        "reconcile.no-change",
        true,
        no_change_samples,
        total_u64,
        i64_to_u64(no_change_count, "no-change available count")?,
    )?);

    let mutations = dataset.apply_incremental_mutations()?;
    ensure!(
        mutations.additions == dataset.mutations.additions
            && mutations.removals == dataset.mutations.removals,
        "dataset mutation helper returned an unexpected mutation set"
    );
    let incremental_roots = prepare_incremental_roots(dataset, &mutations)?;
    let incremental_paths = paths_after_incremental(dataset, &mutations);
    let incremental_expected = incremental_paths.len();
    let unavailable_by_root = source_ownership_counts(dataset, &mutations.removals)?;
    let incremental_source_counts = source_counts_after_mutations(
        &initial_source_counts,
        &source_ownership_counts(dataset, &mutations.additions)?,
        &unavailable_by_root,
    )?;

    let expected_incremental = incremental_roots
        .iter()
        .map(|root| root.additions.len())
        .collect::<Vec<_>>();
    let incremental_samples = measure_each_sample(
        policy.heavy,
        |sample_index| {
            reconcile_incremental(
                &libraries[sample_index].database,
                &source_roots,
                &incremental_roots,
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
    )?;
    for library in &libraries {
        validate_available_library(&library.database, &incremental_paths, &mutations.removals)?;
        validate_source_ownership(
            &library.database,
            dataset,
            &incremental_source_counts,
            &unavailable_by_root,
        )?;
        validate_preserved_metadata(
            &library.database,
            library.fixture.as_ref().context("missing query fixture")?,
            &mutations.removals,
        )?;
    }
    let incremental_count = libraries[0]
        .database
        .get_wallpapers_page("all", "created", "", 0, 1)?
        .total;
    records.push(record(
        "reconcile.incremental",
        true,
        incremental_samples,
        usize_to_u64(incremental_expected, "incremental expected count")?,
        i64_to_u64(incremental_count, "incremental available count")?,
    )?);

    let recovered = dataset.recover_removed()?;
    ensure!(
        recovered == mutations.removals,
        "dataset recovery helper restored an unexpected path set"
    );
    let recovered_images = scan_all_sources(&source_roots)?;
    let recovery_paths = dataset
        .absolute_paths
        .iter()
        .chain(mutations.additions.iter())
        .cloned()
        .collect::<Vec<_>>();
    validate_scanned_paths(&recovered_images, &recovery_paths, "recovery scan")?;
    let recovery_expected = recovery_paths.len();
    let recovery_sources = source_ownership_counts(dataset, &recovery_paths)?;
    let expected_recovery = recovered_images.iter().map(Vec::len).collect::<Vec<_>>();
    let recovery_samples = measure_each_sample(
        policy.heavy,
        |sample_index| {
            reconcile_full_snapshot(
                &libraries[sample_index].database,
                &source_roots,
                &recovered_images,
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
    )?;
    for library in &libraries {
        validate_available_library(&library.database, &recovery_paths, &[])?;
        validate_source_ownership(
            &library.database,
            dataset,
            &recovery_sources,
            &no_unavailable,
        )?;
        validate_preserved_metadata(
            &library.database,
            library.fixture.as_ref().context("missing query fixture")?,
            &[],
        )?;
    }
    let recovery_count = libraries[0]
        .database
        .get_wallpapers_page("all", "created", "", 0, 1)?
        .total;
    records.push(record(
        "reconcile.recovery",
        true,
        recovery_samples,
        usize_to_u64(recovery_expected, "recovery expected count")?,
        i64_to_u64(recovery_count, "recovery available count")?,
    )?);

    let mut primary = libraries.remove(0);
    let fixture = primary
        .fixture
        .take()
        .context("missing primary query fixture")?;
    for library in &libraries {
        library.database.checkpoint_wal()?;
    }
    drop(libraries);
    primary.database.checkpoint_wal()?;
    let database_path = primary.database_path.clone();
    drop(primary.database);

    let mut startup_total = None;
    let startup_samples = measure_each_sample(
        policy.heavy,
        |_| {
            let database = Database::new(&database_path)
                .with_context(|| format!("reopen startup database {}", database_path.display()))?;
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
                dataset,
                recovery_expected,
                &recovery_sources,
                &fixture,
                &measurement.snapshot,
            )?;
            remember_count(
                &mut startup_total,
                measurement.snapshot.page.total,
                "startup page total",
            )
        },
    )?;
    let startup_total = startup_total.context("startup produced no page total")?;
    records.push(record(
        "startup.open-first-page-stats",
        true,
        startup_samples,
        usize_to_u64(recovery_expected, "startup expected count")?,
        i64_to_u64(startup_total, "startup page total")?,
    )?);

    let database = Database::new(&database_path)
        .with_context(|| format!("open query database {}", database_path.display()))?;

    let mut first_total = None;
    let first_samples = measure_each_sample(
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
            remember_count(&mut first_total, page.total, "first-page total")
        },
    )?;
    let first_total = first_total.context("first-page query produced no total")?;
    records.push(record(
        "query.page.first",
        true,
        first_samples,
        usize_to_u64(recovery_expected, "first-page expected total")?,
        i64_to_u64(first_total, "first-page total")?,
    )?);

    let deep_offset = (usize_to_i64(recovery_expected, "deep page total")? - PAGE_LIMIT).max(0);
    let mut deep_total = None;
    let deep_samples = measure_each_sample(
        policy.short,
        |_| database.get_wallpapers_page("all", "created", "", deep_offset, PAGE_LIMIT),
        |_, page| {
            validate_page(page, recovery_expected, deep_offset, PAGE_LIMIT, false)?;
            remember_count(&mut deep_total, page.total, "deep-page total")
        },
    )?;
    let deep_total = deep_total.context("deep-page query produced no total")?;
    records.push(record(
        "query.page.deep",
        true,
        deep_samples,
        usize_to_u64(recovery_expected, "deep-page expected total")?,
        i64_to_u64(deep_total, "deep-page total")?,
    )?);

    let mut unicode_total = None;
    let unicode_samples = measure_each_sample(
        policy.short,
        |_| database.get_wallpapers_page("all", "created", TAG_NAME, 0, PAGE_LIMIT),
        |_, page| {
            validate_membership_page(page, &fixture.unicode_matches, TAG_NAME)?;
            remember_count(&mut unicode_total, page.total, "Unicode search total")
        },
    )?;
    validate_complete_query_membership(
        &database,
        "all",
        "created",
        TAG_NAME,
        &fixture.unicode_matches,
        "Unicode search",
    )?;
    let unicode_total = unicode_total.context("Unicode search produced no total")?;
    records.push(record(
        "query.search.unicode",
        true,
        unicode_samples,
        usize_to_u64(
            fixture.unicode_matches.len(),
            "Unicode search expected count",
        )?,
        i64_to_u64(unicode_total, "Unicode search total")?,
    )?);

    let mut none_total = None;
    let none_samples = measure_each_sample(
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
            remember_count(&mut none_total, page.total, "absent search total")
        },
    )?;
    let none_total = none_total.context("absent search produced no total")?;
    records.push(record(
        "query.search.none",
        true,
        none_samples,
        0,
        i64_to_u64(none_total, "absent search total")?,
    )?);

    let tag_filter = format!("tag:{}", fixture.tag_id);
    let mut tag_total = None;
    let tag_samples = measure_each_sample(
        policy.short,
        |_| database.get_wallpapers_page(&tag_filter, "liked", "", 0, PAGE_LIMIT),
        |_, page| {
            validate_membership_page(page, &fixture.tagged, "tag filter")?;
            remember_count(&mut tag_total, page.total, "tag-filter total")
        },
    )?;
    validate_complete_query_membership(
        &database,
        &tag_filter,
        "liked",
        "",
        &fixture.tagged,
        "tag filter",
    )?;
    let tag_total = tag_total.context("tag-filter query produced no total")?;
    records.push(record(
        "query.filter.tag",
        true,
        tag_samples,
        usize_to_u64(fixture.tagged.len(), "tag-filter expected count")?,
        i64_to_u64(tag_total, "tag-filter total")?,
    )?);

    let collection_filter = format!("collection:{}", fixture.collection_id);
    let mut collection_total = None;
    let collection_samples = measure_each_sample(
        policy.short,
        |_| database.get_wallpapers_page(&collection_filter, "recent", "", 0, PAGE_LIMIT),
        |_, page| {
            validate_membership_page(page, &fixture.collected, "collection filter")?;
            remember_count(&mut collection_total, page.total, "collection-filter total")
        },
    )?;
    validate_complete_query_membership(
        &database,
        &collection_filter,
        "recent",
        "",
        &fixture.collected,
        "collection filter",
    )?;
    let collection_total = collection_total.context("collection-filter query produced no total")?;
    records.push(record(
        "query.filter.collection",
        true,
        collection_samples,
        usize_to_u64(fixture.collected.len(), "collection-filter expected count")?,
        i64_to_u64(collection_total, "collection-filter total")?,
    )?);

    let mut plays_total = None;
    let plays_samples = measure_each_sample(
        policy.short,
        |_| database.get_wallpapers_page("all", "plays", "", 0, PAGE_LIMIT),
        |_, page| {
            validate_plays_page(page, recovery_expected, &fixture)?;
            remember_count(&mut plays_total, page.total, "plays-sort total")
        },
    )?;
    let plays_total = plays_total.context("plays-sort query produced no total")?;
    records.push(record(
        "query.sort.plays",
        true,
        plays_samples,
        usize_to_u64(recovery_expected, "plays-sort expected total")?,
        i64_to_u64(plays_total, "plays-sort total")?,
    )?);

    database.checkpoint_wal()?;
    drop(database);
    Ok(records)
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
    dataset: &GeneratedDataset,
    expected_total: usize,
    expected_source_counts: &[usize],
    fixture: &QueryFixture,
    startup: &StartupSnapshot,
) -> Result<()> {
    validate_source_summaries(
        &startup.summaries,
        dataset,
        expected_source_counts,
        &vec![0; dataset.source_roots.len()],
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
        dataset,
        expected_available,
        expected_unavailable,
    )
}

fn validate_source_summaries(
    summaries: &[WatchedFolderSummary],
    dataset: &GeneratedDataset,
    expected_available: &[usize],
    expected_unavailable: &[usize],
) -> Result<()> {
    ensure!(
        expected_available.len() == dataset.source_roots.len()
            && expected_unavailable.len() == dataset.source_roots.len(),
        "source summary expectation length mismatch"
    );
    ensure!(
        summaries.len() == dataset.source_roots.len(),
        "watched source summary count mismatch"
    );
    for (index, root) in dataset.source_roots.iter().enumerate() {
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
    elapsed_micros: Vec<u64>,
    expected_count: u64,
    actual_count: u64,
) -> Result<ScenarioRecord> {
    Ok(ScenarioRecord {
        id: id.to_string(),
        critical,
        selected_hotspot: false,
        cache_state: CacheState::NotApplicable,
        status: RunStatus::Passed,
        samples: ScenarioSamples::from_elapsed_micros(elapsed_micros, 0)?,
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

    use super::{measure_each_sample, run_library_scenarios, SamplePolicy};
    use crate::performance_harness::dataset::{generate_dataset, DatasetScale, DATASET_SEED};
    use crate::performance_harness::owned_temp::OwnedRunRoot;
    use crate::performance_harness::protocol::RunStatus;

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
        run.cleanup().unwrap();
    }

    #[test]
    fn sample_policy_and_temporary_timer_keep_the_declared_sample_contract() {
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
        assert_eq!(samples.len(), 3);
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
