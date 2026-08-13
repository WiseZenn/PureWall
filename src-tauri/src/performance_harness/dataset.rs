use super::owned_temp::OwnedRunRoot;
use anyhow::{anyhow, bail, Context, Result};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};

pub(crate) const DATASET_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_LINKS_PER_BACKING_FILE: usize = 512;
pub(crate) const DATASET_SEED: u64 = 0x5055_5245_5741_4c4c;

const MANIFEST_FILE_NAME: &str = "dataset-manifest.json";
const MAX_SCANNED_LINKS_PER_BACKING_FILE: usize = MAX_LINKS_PER_BACKING_FILE - 1;
const MAX_MUTATIONS_PER_KIND: usize = 16;
const DATABASE_BYTES_PER_ITEM: u64 = 16 * 1024;
const CACHE_BYTES_PER_ITEM: u64 = 2 * 1024 * 1024;
const FREE_SPACE_RESERVE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DatasetScale {
    Ci,
    Standard,
    Stress,
}

impl DatasetScale {
    pub(crate) fn layout(self) -> (usize, usize) {
        match self {
            Self::Ci => (120, 2),
            Self::Standard => (10_000, 1),
            Self::Stress => (100_000, 10),
        }
    }

    pub(crate) fn requires_explicit_opt_in(self) -> bool {
        matches!(self, Self::Stress)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DatasetImageFormat {
    Jpeg,
    Png,
    Webp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum MutationRole {
    Baseline,
    AdditionSource,
    Removal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DatasetManifestEntry {
    pub(crate) source_index: usize,
    pub(crate) relative_path: String,
    pub(crate) format: DatasetImageFormat,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) mutation_role: MutationRole,
}

impl DatasetManifestEntry {
    pub(crate) fn directory_depth(&self) -> usize {
        self.relative_path.split('/').count().saturating_sub(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DatasetManifest {
    pub(crate) schema_version: u32,
    pub(crate) scale: DatasetScale,
    pub(crate) seed: u64,
    pub(crate) item_count: usize,
    pub(crate) source_count: usize,
    pub(crate) entries: Vec<DatasetManifestEntry>,
    pub(crate) logical_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MutationSet {
    pub(crate) additions: Vec<PathBuf>,
    pub(crate) removals: Vec<PathBuf>,
}

#[derive(Debug)]
pub(crate) struct GeneratedDataset {
    pub(crate) manifest: DatasetManifest,
    pub(crate) source_roots: Vec<PathBuf>,
    pub(crate) absolute_paths: Vec<PathBuf>,
    pub(crate) backing_dir: PathBuf,
    pub(crate) backing_lookup: BTreeMap<PathBuf, PathBuf>,
    pub(crate) mutations: MutationSet,
}

impl GeneratedDataset {
    pub(crate) fn path_for_entry(&self, entry: &DatasetManifestEntry) -> Result<PathBuf> {
        let root = self
            .source_roots
            .get(entry.source_index)
            .ok_or_else(|| anyhow!("performance dataset manifest source index is invalid"))?;
        Ok(root.join(&entry.relative_path))
    }

    pub(crate) fn contains_source_path(&self, path: &Path) -> bool {
        path.is_absolute()
            && !has_path_alias(path)
            && self
                .source_roots
                .iter()
                .any(|root| is_strict_descendant(path, root))
    }

    pub(crate) fn validate_mutation_paths(&self, paths: &[PathBuf]) -> Result<()> {
        if paths.iter().all(|path| self.contains_source_path(path)) {
            Ok(())
        } else {
            bail!("performance dataset mutation refused path outside owned source roots")
        }
    }

    pub(crate) fn apply_incremental_mutations(&self) -> Result<MutationSet> {
        self.validate_mutation_paths(&self.mutations.additions)?;
        self.validate_mutation_paths(&self.mutations.removals)?;

        for path in &self.mutations.additions {
            if path.exists() {
                bail!("performance dataset mutation addition already exists");
            }
            let backing = self
                .backing_lookup
                .get(path)
                .ok_or_else(|| anyhow!("performance dataset mutation backing lookup is missing"))?;
            if !backing.is_file() || !backing.starts_with(&self.backing_dir) {
                bail!("performance dataset mutation backing path is invalid");
            }
        }
        for path in &self.mutations.removals {
            if !path.is_file() {
                bail!("performance dataset mutation removal is missing");
            }
        }

        for path in &self.mutations.additions {
            let backing = self.backing_lookup.get(path).expect("validated lookup");
            hard_link(backing, path)?;
        }
        for path in &self.mutations.removals {
            fs::remove_file(path).with_context(|| {
                format!(
                    "performance dataset mutation failed to remove {}",
                    path.display()
                )
            })?;
        }
        Ok(self.mutations.clone())
    }

    pub(crate) fn recover_removed(&self) -> Result<Vec<PathBuf>> {
        self.validate_mutation_paths(&self.mutations.removals)?;
        for path in &self.mutations.removals {
            if path.exists() {
                bail!("performance dataset recovery target already exists");
            }
            let backing = self
                .backing_lookup
                .get(path)
                .ok_or_else(|| anyhow!("performance dataset recovery backing lookup is missing"))?;
            if !backing.is_file() || !backing.starts_with(&self.backing_dir) {
                bail!("performance dataset recovery backing path is invalid");
            }
        }
        for path in &self.mutations.removals {
            hard_link(
                self.backing_lookup.get(path).expect("validated lookup"),
                path,
            )?;
        }
        Ok(self.mutations.removals.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpaceEstimate {
    pub(crate) backing_bytes: u64,
    pub(crate) database_bytes: u64,
    pub(crate) cache_bytes: u64,
    pub(crate) reserve_bytes: u64,
}

impl SpaceEstimate {
    pub(crate) fn total_bytes(self) -> u64 {
        self.backing_bytes
            .saturating_add(self.database_bytes)
            .saturating_add(self.cache_bytes)
            .saturating_add(self.reserve_bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ImageClass {
    LandscapeJpeg,
    PortraitPng,
    SquareWebp,
    UltraJpeg,
}

impl ImageClass {
    const ALL: [Self; 4] = [
        Self::LandscapeJpeg,
        Self::PortraitPng,
        Self::SquareWebp,
        Self::UltraJpeg,
    ];

    fn dimensions(self) -> (u32, u32) {
        match self {
            Self::LandscapeJpeg => (320, 180),
            Self::PortraitPng => (180, 320),
            Self::SquareWebp => (512, 512),
            Self::UltraJpeg => (3840, 2160),
        }
    }

    fn format(self) -> DatasetImageFormat {
        match self {
            Self::LandscapeJpeg | Self::UltraJpeg => DatasetImageFormat::Jpeg,
            Self::PortraitPng => DatasetImageFormat::Png,
            Self::SquareWebp => DatasetImageFormat::Webp,
        }
    }

    fn image_format(self) -> ImageFormat {
        match self.format() {
            DatasetImageFormat::Jpeg => ImageFormat::Jpeg,
            DatasetImageFormat::Png => ImageFormat::Png,
            DatasetImageFormat::Webp => ImageFormat::WebP,
        }
    }

    fn extension(self) -> &'static str {
        match self.format() {
            DatasetImageFormat::Jpeg => "jpg",
            DatasetImageFormat::Png => "png",
            DatasetImageFormat::Webp => "webp",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::LandscapeJpeg => "landscape",
            Self::PortraitPng => "portrait",
            Self::SquareWebp => "square",
            Self::UltraJpeg => "ultra",
        }
    }

    fn ordinal(self) -> u64 {
        match self {
            Self::LandscapeJpeg => 0,
            Self::PortraitPng => 1,
            Self::SquareWebp => 2,
            Self::UltraJpeg => 3,
        }
    }
}

#[derive(Debug)]
struct EntryPlan {
    class: ImageClass,
    entry: DatasetManifestEntry,
    absolute_path: PathBuf,
}

pub(crate) fn generate_dataset(
    run: &OwnedRunRoot,
    scale: DatasetScale,
    seed: u64,
) -> Result<GeneratedDataset> {
    if scale.requires_explicit_opt_in() {
        bail!("performance dataset preflight failed: stress generation requires explicit opt-in");
    }

    preflight_layout(scale)?;
    let available = available_space(run.root())?;
    preflight_space_with_available(scale, available)?;

    let backing_dir = run.root().join("backing");
    create_new_directory(&backing_dir, "performance dataset backing directory")?;
    preflight_hard_links(&backing_dir)?;

    let (item_count, source_count) = scale.layout();
    let source_roots = (0..source_count)
        .map(|index| run.sources_dir().join(format!("source-{index:04}")))
        .collect::<Vec<_>>();
    for root in &source_roots {
        create_new_directory(root, "performance dataset source root")?;
    }

    let mutation_count = mutation_count(item_count);
    let mut plans = Vec::with_capacity(item_count);
    for index in 0..item_count {
        let class = ImageClass::ALL[index % ImageClass::ALL.len()];
        let group = index / ImageClass::ALL.len();
        let source_index = (group + index % ImageClass::ALL.len()) % source_count;
        let depth = (group + index % ImageClass::ALL.len()) % 4;
        let relative_path = relative_path(index, class, depth);
        let role = if index < mutation_count {
            MutationRole::AdditionSource
        } else if index < mutation_count * 2 {
            MutationRole::Removal
        } else {
            MutationRole::Baseline
        };
        let entry = DatasetManifestEntry {
            source_index,
            relative_path,
            format: class.format(),
            width: class.dimensions().0,
            height: class.dimensions().1,
            mutation_role: role,
        };
        let absolute_path = source_roots[source_index].join(&entry.relative_path);
        plans.push(EntryPlan {
            class,
            entry,
            absolute_path,
        });
    }
    plans.sort_by(|left, right| left.entry.relative_path.cmp(&right.entry.relative_path));

    let mut targets_per_class = class_counts(item_count);
    for plan in plans
        .iter()
        .filter(|plan| plan.entry.mutation_role == MutationRole::AdditionSource)
    {
        *targets_per_class.entry(plan.class).or_default() += 1;
    }
    let backing_files = write_backing_files(&backing_dir, seed, &targets_per_class)?;

    let mut backing_loads = BTreeMap::<PathBuf, usize>::new();
    let mut backing_lookup = BTreeMap::new();
    for plan in &plans {
        let backing = claim_backing(plan.class, &backing_files, &mut backing_loads)?;
        if let Some(parent) = plan.absolute_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("create performance dataset directory {}", parent.display())
            })?;
        }
        hard_link(&backing, &plan.absolute_path)?;
        backing_lookup.insert(plan.absolute_path.clone(), backing);
    }

    let mut additions = Vec::with_capacity(mutation_count);
    let mut removals = Vec::with_capacity(mutation_count);
    for plan in &plans {
        match plan.entry.mutation_role {
            MutationRole::AdditionSource => {
                let addition = addition_path(&plan.absolute_path, additions.len())?;
                let backing = claim_backing(plan.class, &backing_files, &mut backing_loads)?;
                backing_lookup.insert(addition.clone(), backing);
                additions.push(addition);
            }
            MutationRole::Removal => removals.push(plan.absolute_path.clone()),
            MutationRole::Baseline => {}
        }
    }

    let mut manifest = DatasetManifest {
        schema_version: DATASET_SCHEMA_VERSION,
        scale,
        seed,
        item_count,
        source_count,
        entries: plans.iter().map(|plan| plan.entry.clone()).collect(),
        logical_digest: String::new(),
    };
    manifest.logical_digest = compute_manifest_digest(&manifest)?;
    persist_manifest(run, &manifest)?;

    let dataset = GeneratedDataset {
        manifest,
        source_roots,
        absolute_paths: plans.into_iter().map(|plan| plan.absolute_path).collect(),
        backing_dir,
        backing_lookup,
        mutations: MutationSet {
            additions,
            removals,
        },
    };
    verify_generated_dataset(&dataset)?;
    Ok(dataset)
}

fn preflight_layout(scale: DatasetScale) -> Result<()> {
    let (total, sources) = scale.layout();
    if sources == 0 || total == 0 || total % sources != 0 {
        bail!("performance dataset preflight failed: invalid scale layout");
    }
    let per_root = total / sources;
    if per_root >= crate::scanner::MAX_SCAN_IMAGES || per_root >= crate::scanner::MAX_SCAN_ENTRIES {
        bail!("performance dataset preflight failed: source root exceeds scanner limits");
    }
    Ok(())
}

pub(crate) fn estimate_required_space(scale: DatasetScale) -> Result<SpaceEstimate> {
    let (item_count, _) = scale.layout();
    let mutation_count = mutation_count(item_count);
    let mut target_counts = class_counts(item_count);
    for index in 0..mutation_count {
        let class = ImageClass::ALL[index % ImageClass::ALL.len()];
        *target_counts.entry(class).or_default() += 1;
    }

    let mut backing_bytes = 0u64;
    for class in ImageClass::ALL {
        let targets = target_counts.get(&class).copied().unwrap_or_default();
        let copies = targets.div_ceil(MAX_SCANNED_LINKS_PER_BACKING_FILE) as u64;
        let (width, height) = class.dimensions();
        let raw_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(3))
            .ok_or_else(|| {
                anyhow!("performance dataset preflight failed: size estimate overflow")
            })?;
        backing_bytes = backing_bytes
            .checked_add(raw_bytes.checked_mul(copies).ok_or_else(|| {
                anyhow!("performance dataset preflight failed: size estimate overflow")
            })?)
            .ok_or_else(|| {
                anyhow!("performance dataset preflight failed: size estimate overflow")
            })?;
    }

    let item_count = item_count as u64;
    let estimate = SpaceEstimate {
        backing_bytes,
        database_bytes: item_count
            .checked_mul(DATABASE_BYTES_PER_ITEM)
            .ok_or_else(|| {
                anyhow!("performance dataset preflight failed: size estimate overflow")
            })?,
        cache_bytes: item_count
            .checked_mul(CACHE_BYTES_PER_ITEM)
            .ok_or_else(|| {
                anyhow!("performance dataset preflight failed: size estimate overflow")
            })?,
        reserve_bytes: FREE_SPACE_RESERVE_BYTES,
    };
    estimate
        .backing_bytes
        .checked_add(estimate.database_bytes)
        .and_then(|total| total.checked_add(estimate.cache_bytes))
        .and_then(|total| total.checked_add(estimate.reserve_bytes))
        .ok_or_else(|| anyhow!("performance dataset preflight failed: size estimate overflow"))?;
    Ok(estimate)
}

pub(crate) fn preflight_space_with_available(
    scale: DatasetScale,
    available_bytes: u64,
) -> Result<SpaceEstimate> {
    let estimate = estimate_required_space(scale)?;
    if available_bytes < estimate.total_bytes() {
        bail!(
            "performance dataset preflight failed: insufficient free space (required {}, available {})",
            estimate.total_bytes(),
            available_bytes
        );
    }
    Ok(estimate)
}

#[cfg(windows)]
fn available_space(path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut available), None, None).map_err(
            |error| anyhow!("performance dataset preflight failed: query free space: {error}"),
        )?;
    }
    Ok(available)
}

#[cfg(not(windows))]
fn available_space(_path: &Path) -> Result<u64> {
    bail!("performance dataset preflight failed: free-space query is unsupported on this platform")
}

fn preflight_hard_links(backing_dir: &Path) -> Result<()> {
    let original = backing_dir.join(".hard-link-probe-original");
    let linked = backing_dir.join(".hard-link-probe-linked");
    fs::write(&original, b"PureWall hard-link preflight").with_context(|| {
        format!(
            "performance dataset preflight failed: write hard-link probe {}",
            original.display()
        )
    })?;
    let link_result = fs::hard_link(&original, &linked);
    let _ = fs::remove_file(&linked);
    let _ = fs::remove_file(&original);
    link_result.map_err(|error| {
        anyhow!("performance dataset preflight failed: hard links unsupported: {error}")
    })
}

fn create_new_directory(path: &Path, label: &str) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!("{label} already exists")
        }
        Err(error) => Err(error).with_context(|| format!("create {label} {}", path.display())),
    }
}

fn mutation_count(item_count: usize) -> usize {
    (item_count / 30).clamp(1, MAX_MUTATIONS_PER_KIND)
}

fn class_counts(item_count: usize) -> BTreeMap<ImageClass, usize> {
    let mut counts = BTreeMap::new();
    for index in 0..item_count {
        *counts
            .entry(ImageClass::ALL[index % ImageClass::ALL.len()])
            .or_default() += 1;
    }
    counts
}

fn relative_path(index: usize, class: ImageClass, depth: usize) -> String {
    let file_name = match index % 3 {
        0 => format!("{} image {index:06}.{}", class.slug(), class.extension()),
        1 => format!("风景-{}-{index:06}.{}", class.slug(), class.extension()),
        _ => format!("{}-{index:06}.{}", class.slug(), class.extension()),
    };
    let directories: &[&str] = match depth {
        0 => &[],
        1 => &["ascii"],
        2 => &["space folder", "level-two"],
        3 => &["风景", "deep layer", "level-three"],
        _ => unreachable!("dataset uses four fixed directory depths"),
    };
    directories
        .iter()
        .copied()
        .chain(std::iter::once(file_name.as_str()))
        .collect::<Vec<_>>()
        .join("/")
}

fn write_backing_files(
    backing_dir: &Path,
    seed: u64,
    target_counts: &BTreeMap<ImageClass, usize>,
) -> Result<BTreeMap<ImageClass, Vec<PathBuf>>> {
    let mut files = BTreeMap::new();
    for class in ImageClass::ALL {
        let targets = target_counts.get(&class).copied().unwrap_or_default();
        let copies = targets.div_ceil(MAX_SCANNED_LINKS_PER_BACKING_FILE);
        let bytes = encode_image(seed, class)?;
        let mut class_files = Vec::with_capacity(copies);
        for copy_index in 0..copies {
            let path = backing_dir.join(format!(
                "{}-{copy_index:04}.{}",
                class.slug(),
                class.extension()
            ));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .with_context(|| {
                    format!(
                        "performance dataset generation failed: create backing {}",
                        path.display()
                    )
                })?;
            file.write_all(&bytes).with_context(|| {
                format!(
                    "performance dataset generation failed: write backing {}",
                    path.display()
                )
            })?;
            file.flush().with_context(|| {
                format!(
                    "performance dataset generation failed: flush backing {}",
                    path.display()
                )
            })?;
            file.sync_all().with_context(|| {
                format!(
                    "performance dataset generation failed: sync backing {}",
                    path.display()
                )
            })?;
            class_files.push(path);
        }
        files.insert(class, class_files);
    }
    Ok(files)
}

fn encode_image(seed: u64, class: ImageClass) -> Result<Vec<u8>> {
    let (width, height) = class.dimensions();
    let image = ImageBuffer::from_fn(width, height, |x, y| {
        let value = seed
            .wrapping_add(class.ordinal().wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add(u64::from(x).wrapping_mul(0xbf58_476d_1ce4_e5b9))
            .wrapping_add(u64::from(y).wrapping_mul(0x94d0_49bb_1331_11eb));
        let mixed = value ^ value.rotate_left(17) ^ value.rotate_right(23);
        Rgb([mixed as u8, (mixed >> 19) as u8, (mixed >> 41) as u8])
    });
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image)
        .write_to(&mut cursor, class.image_format())
        .with_context(|| {
            format!(
                "performance dataset generation failed: encode {}",
                class.slug()
            )
        })?;
    Ok(cursor.into_inner())
}

fn claim_backing(
    class: ImageClass,
    backing_files: &BTreeMap<ImageClass, Vec<PathBuf>>,
    loads: &mut BTreeMap<PathBuf, usize>,
) -> Result<PathBuf> {
    for backing in backing_files.get(&class).into_iter().flatten() {
        let load = loads.entry(backing.clone()).or_default();
        if *load < MAX_SCANNED_LINKS_PER_BACKING_FILE {
            *load += 1;
            return Ok(backing.clone());
        }
    }
    bail!("performance dataset generation failed: bounded backing fanout exhausted")
}

fn hard_link(existing: &Path, linked: &Path) -> Result<()> {
    hard_link_with(existing, linked, |source, target| {
        fs::hard_link(source, target)
    })
}

fn hard_link_with<F>(existing: &Path, linked: &Path, link: F) -> Result<()>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    link(existing, linked).map_err(|error| {
        anyhow!("performance dataset generation failed: hard links unsupported: {error}")
    })
}

fn addition_path(source: &Path, mutation_index: usize) -> Result<PathBuf> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("performance dataset mutation source extension is invalid"))?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow!("performance dataset mutation source parent is invalid"))?;
    Ok(parent.join(format!("mutation-added-{mutation_index:04}.{extension}")))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogicalManifest<'a> {
    schema_version: u32,
    scale: DatasetScale,
    seed: u64,
    item_count: usize,
    source_count: usize,
    entries: &'a [DatasetManifestEntry],
}

pub(crate) fn compute_manifest_digest(manifest: &DatasetManifest) -> Result<String> {
    let bytes = serde_json::to_vec(&LogicalManifest {
        schema_version: manifest.schema_version,
        scale: manifest.scale,
        seed: manifest.seed,
        item_count: manifest.item_count,
        source_count: manifest.source_count,
        entries: &manifest.entries,
    })
    .context("serialize logical performance dataset manifest")?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

fn persist_manifest(run: &OwnedRunRoot, manifest: &DatasetManifest) -> Result<()> {
    let path = run.root().join(MANIFEST_FILE_NAME);
    let bytes = serde_json::to_vec(manifest).context("serialize performance dataset manifest")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("create performance dataset manifest {}", path.display()))?;
    file.write_all(&bytes)
        .context("write performance dataset manifest")?;
    file.flush().context("flush performance dataset manifest")?;
    file.sync_all()
        .context("sync performance dataset manifest")?;
    Ok(())
}

fn verify_generated_dataset(dataset: &GeneratedDataset) -> Result<()> {
    let expected = dataset.manifest.item_count;
    if dataset.absolute_paths.len() != expected
        || dataset.manifest.entries.len() != expected
        || dataset.source_roots.len() != dataset.manifest.source_count
    {
        bail!("performance dataset generation failed: exact dataset counts do not match");
    }
    if compute_manifest_digest(&dataset.manifest)? != dataset.manifest.logical_digest {
        bail!("performance dataset generation failed: manifest digest mismatch");
    }
    if dataset
        .manifest
        .entries
        .windows(2)
        .any(|pair| pair[0].relative_path > pair[1].relative_path)
    {
        bail!("performance dataset generation failed: manifest entries are not sorted");
    }

    let expected_per_root = expected / dataset.source_roots.len();
    let mut scanned_total = 0usize;
    for root in &dataset.source_roots {
        if !root.is_absolute()
            || !root.is_dir()
            || dataset.backing_dir.starts_with(root)
            || expected_per_root >= crate::scanner::MAX_SCAN_IMAGES
            || expected_per_root >= crate::scanner::MAX_SCAN_ENTRIES
        {
            bail!("performance dataset generation failed: source root boundary is invalid");
        }
        let scanned = crate::scanner::scan_folder(root.to_str().ok_or_else(|| {
            anyhow!("performance dataset generation failed: source root is not UTF-8")
        })?)?;
        if scanned.len() != expected_per_root {
            bail!("performance dataset generation failed: source root count mismatch");
        }
        scanned_total += scanned.len();
    }
    if scanned_total != expected {
        bail!("performance dataset generation failed: total scan count mismatch");
    }
    if dataset
        .absolute_paths
        .iter()
        .any(|path| !dataset.contains_source_path(path) || !path.is_file())
    {
        bail!("performance dataset generation failed: generated image path is invalid");
    }
    dataset.validate_mutation_paths(&dataset.mutations.additions)?;
    dataset.validate_mutation_paths(&dataset.mutations.removals)?;
    if dataset.mutations.additions.iter().any(|path| path.exists())
        || dataset
            .mutations
            .removals
            .iter()
            .any(|path| !path.is_file())
    {
        bail!("performance dataset generation failed: mutation initial state is invalid");
    }

    let mut references = BTreeMap::<&PathBuf, usize>::new();
    for backing in dataset.backing_lookup.values() {
        if !backing.is_file()
            || !backing.starts_with(&dataset.backing_dir)
            || dataset
                .source_roots
                .iter()
                .any(|root| backing.starts_with(root))
        {
            bail!("performance dataset generation failed: backing path is invalid");
        }
        *references.entry(backing).or_insert(1) += 1;
    }
    if references
        .values()
        .any(|count| *count > MAX_LINKS_PER_BACKING_FILE)
    {
        bail!("performance dataset generation failed: backing link fanout exceeds limit");
    }

    for class in ImageClass::ALL {
        let (width, height) = class.dimensions();
        let entry = dataset
            .manifest
            .entries
            .iter()
            .find(|entry| {
                entry.format == class.format() && entry.width == width && entry.height == height
            })
            .ok_or_else(|| {
                anyhow!("performance dataset generation failed: image class is missing")
            })?;
        let decoded = image::open(dataset.path_for_entry(entry)?).with_context(|| {
            format!(
                "performance dataset generation failed: decode {} sample",
                class.slug()
            )
        })?;
        if decoded.width() != width || decoded.height() != height {
            bail!("performance dataset generation failed: decoded dimensions mismatch");
        }
    }
    Ok(())
}

fn has_path_alias(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        || path
            .to_string_lossy()
            .split(['/', '\\'])
            .any(|component| matches!(component, "." | ".."))
}

fn path_identity(path: &Path) -> String {
    let mut identity = path.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        if identity
            .as_bytes()
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"//?/UNC/"))
        {
            identity = format!("//{}", &identity[8..]);
        } else if identity
            .as_bytes()
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"//?/"))
        {
            identity = identity[4..].to_owned();
        }
        identity.make_ascii_lowercase();
    }
    while identity.len() > 1 && identity.ends_with('/') {
        identity.pop();
    }
    identity
}

fn is_strict_descendant(path: &Path, root: &Path) -> bool {
    let path = path_identity(path);
    let root = path_identity(root);
    path != root && path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance_harness::owned_temp::OwnedRunRoot;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    fn image_count(root: &Path) -> usize {
        crate::scanner::scan_folder(root.to_str().unwrap())
            .unwrap()
            .len()
    }

    fn with_owned_run<T>(run_id: &str, test: impl FnOnce(&OwnedRunRoot) -> T) -> T {
        let run = OwnedRunRoot::create_for_test(run_id).unwrap();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| test(&run)));
        let cleanup = run.cleanup();
        match outcome {
            Ok(value) => {
                cleanup.unwrap();
                value
            }
            Err(panic) => {
                let _ = cleanup;
                std::panic::resume_unwind(panic)
            }
        }
    }

    #[test]
    fn ci_dataset_is_deterministic_valid_and_within_scan_limits() {
        with_owned_run("dataset-a", |first| {
            with_owned_run("dataset-b", |second| {
                let a = generate_dataset(first, DatasetScale::Ci, DATASET_SEED).unwrap();
                let b = generate_dataset(second, DatasetScale::Ci, DATASET_SEED).unwrap();

                assert_eq!(a.manifest.logical_digest, b.manifest.logical_digest);
                assert_eq!(a.manifest, b.manifest);
                assert_eq!(a.absolute_paths.len(), 120);
                assert_eq!(a.source_roots.len(), 2);
                assert!(a.source_roots.iter().all(|root| image_count(root) == 60));
                assert_eq!(a.manifest.entries.len(), 120);
                assert_eq!(a.manifest.logical_digest.len(), 64);
                assert!(a
                    .manifest
                    .logical_digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
                assert_eq!(
                    a.manifest.logical_digest,
                    compute_manifest_digest(&a.manifest).unwrap()
                );
                assert!(a
                    .manifest
                    .entries
                    .windows(2)
                    .all(|pair| pair[0].relative_path <= pair[1].relative_path));
                assert!(a
                    .manifest
                    .entries
                    .iter()
                    .any(|entry| entry.relative_path.contains(' ')));
                assert!(a
                    .manifest
                    .entries
                    .iter()
                    .any(|entry| entry.relative_path.contains("风景")));
                assert!(a
                    .source_roots
                    .iter()
                    .all(|root| root.starts_with(first.sources_dir())));
                assert!(a
                    .source_roots
                    .iter()
                    .all(|root| !a.backing_dir.starts_with(root)));
                assert_eq!(a.backing_dir, first.root().join("backing"));
                assert!(
                    !crate::scanner::scan_folder(a.backing_dir.to_str().unwrap())
                        .unwrap()
                        .is_empty()
                );
                assert_eq!(
                    a.source_roots
                        .iter()
                        .map(|root| image_count(root))
                        .sum::<usize>(),
                    a.absolute_paths.len()
                );
                let manifest_bytes =
                    std::fs::read(first.root().join("dataset-manifest.json")).unwrap();
                let persisted: DatasetManifest = serde_json::from_slice(&manifest_bytes).unwrap();
                assert_eq!(persisted, a.manifest);
            });
        });
    }

    #[test]
    fn scale_contract_never_bypasses_scanner_limits() {
        assert_eq!(DatasetScale::Ci.layout(), (120, 2));
        assert_eq!(DatasetScale::Standard.layout(), (10_000, 1));
        assert_eq!(DatasetScale::Stress.layout(), (100_000, 10));
        for scale in [
            DatasetScale::Ci,
            DatasetScale::Standard,
            DatasetScale::Stress,
        ] {
            let (total, roots) = scale.layout();
            assert_eq!(total % roots, 0);
            assert!(total / roots < crate::scanner::MAX_SCAN_IMAGES);
            assert!(total / roots < crate::scanner::MAX_SCAN_ENTRIES);
        }
        assert!(DatasetScale::Stress.requires_explicit_opt_in());
        assert!(!DatasetScale::Ci.requires_explicit_opt_in());
        assert!(!DatasetScale::Standard.requires_explicit_opt_in());
    }

    #[test]
    fn stress_generation_requires_a_later_explicit_opt_in() {
        with_owned_run("dataset-stress-gate", |run| {
            let error = generate_dataset(run, DatasetScale::Stress, DATASET_SEED)
                .unwrap_err()
                .to_string();
            assert_eq!(
                error,
                "performance dataset preflight failed: stress generation requires explicit opt-in"
            );
            assert!(!run.root().join("backing").exists());
            assert!(std::fs::read_dir(run.sources_dir())
                .unwrap()
                .next()
                .is_none());
        });
    }

    #[test]
    fn preflight_space_failure_is_stable_and_precedes_generation() {
        let estimate = estimate_required_space(DatasetScale::Ci).unwrap();
        assert!(estimate.backing_bytes > 0);
        assert!(estimate.database_bytes > 0);
        assert!(estimate.cache_bytes > 0);
        assert!(estimate.reserve_bytes > 0);
        let error = preflight_space_with_available(DatasetScale::Ci, estimate.total_bytes() - 1)
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("performance dataset preflight failed:"));
        assert!(error.contains("insufficient free space"));
    }

    #[test]
    fn hard_link_failure_has_a_stable_generation_error() {
        let error = hard_link_with(Path::new("backing.jpg"), Path::new("linked.jpg"), |_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "no links",
            ))
        })
        .unwrap_err()
        .to_string();
        assert!(error.starts_with("performance dataset generation failed: hard links unsupported:"));
    }

    #[test]
    fn ci_distribution_and_all_image_classes_are_exact_and_decodable() {
        with_owned_run("dataset-layout", |run| {
            let dataset = generate_dataset(run, DatasetScale::Ci, DATASET_SEED).unwrap();
            let mut formats = BTreeMap::new();
            let mut depths = BTreeMap::new();
            let mut per_source = BTreeMap::new();
            let mut mutation_roles = BTreeMap::new();

            for entry in &dataset.manifest.entries {
                *formats.entry(entry.format).or_insert(0usize) += 1;
                *depths.entry(entry.directory_depth()).or_insert(0usize) += 1;
                *per_source.entry(entry.source_index).or_insert(0usize) += 1;
                *mutation_roles.entry(entry.mutation_role).or_insert(0usize) += 1;
            }

            assert_eq!(formats.get(&DatasetImageFormat::Jpeg), Some(&60));
            assert_eq!(formats.get(&DatasetImageFormat::Png), Some(&30));
            assert_eq!(formats.get(&DatasetImageFormat::Webp), Some(&30));
            assert_eq!(depths, BTreeMap::from([(0, 30), (1, 30), (2, 30), (3, 30)]));
            assert_eq!(per_source, BTreeMap::from([(0, 60), (1, 60)]));
            assert_eq!(
                mutation_roles,
                BTreeMap::from([
                    (MutationRole::Baseline, 112),
                    (MutationRole::AdditionSource, 4),
                    (MutationRole::Removal, 4),
                ])
            );

            let classes = dataset
                .manifest
                .entries
                .iter()
                .map(|entry| (entry.format, entry.width, entry.height))
                .collect::<BTreeSet<_>>();
            assert_eq!(
                classes,
                BTreeSet::from([
                    (DatasetImageFormat::Jpeg, 320, 180),
                    (DatasetImageFormat::Png, 180, 320),
                    (DatasetImageFormat::Webp, 512, 512),
                    (DatasetImageFormat::Jpeg, 3840, 2160),
                ])
            );
            for class in classes {
                let entry = dataset
                    .manifest
                    .entries
                    .iter()
                    .find(|entry| (entry.format, entry.width, entry.height) == class)
                    .unwrap();
                let path = dataset.path_for_entry(entry).unwrap();
                let decoded = image::open(path).unwrap();
                assert_eq!(
                    (decoded.width(), decoded.height()),
                    (entry.width, entry.height)
                );
            }
        });
    }

    #[test]
    fn backing_fanout_is_bounded_and_manifest_contains_no_absolute_paths() {
        with_owned_run("dataset-links", |run| {
            let dataset = generate_dataset(run, DatasetScale::Ci, DATASET_SEED).unwrap();

            let mut link_counts = BTreeMap::new();
            for backing in dataset.backing_lookup.values() {
                *link_counts.entry(backing).or_insert(1usize) += 1;
            }
            assert!(link_counts
                .values()
                .all(|count| *count <= MAX_LINKS_PER_BACKING_FILE));
            assert!(dataset.backing_lookup.keys().all(|path| path.is_absolute()));
            assert!(dataset
                .backing_lookup
                .values()
                .all(|path| path.is_absolute()));

            let manifest_json = serde_json::to_string(&dataset.manifest).unwrap();
            assert!(!manifest_json.contains(&run.root().to_string_lossy().to_string()));
            assert!(!manifest_json.contains("created_at"));
            assert!(!manifest_json.contains("timestamp"));
        });
    }

    #[test]
    fn deterministic_mutations_return_exact_paths_and_recover_removed_links() {
        with_owned_run("dataset-mutations", |run| {
            let dataset = generate_dataset(run, DatasetScale::Ci, DATASET_SEED).unwrap();
            let baseline = dataset.absolute_paths.len();
            let expected = dataset.mutations.clone();

            assert!(!expected.additions.is_empty());
            assert_eq!(expected.additions.len(), expected.removals.len());
            assert!(expected
                .additions
                .iter()
                .all(|path| !path.exists() && dataset.contains_source_path(path)));
            assert!(expected
                .removals
                .iter()
                .all(|path| path.is_file() && dataset.contains_source_path(path)));

            let changed = dataset.apply_incremental_mutations().unwrap();
            assert_eq!(changed, expected);
            assert!(changed.additions.iter().all(|path| path.is_file()));
            assert!(changed.removals.iter().all(|path| !path.exists()));
            assert_eq!(
                dataset
                    .source_roots
                    .iter()
                    .map(|root| image_count(root))
                    .sum::<usize>(),
                baseline
            );

            let recovered = dataset.recover_removed().unwrap();
            assert_eq!(recovered, expected.removals);
            assert!(recovered.iter().all(|path| path.is_file()));
        });
    }

    #[test]
    fn mutation_boundary_refuses_outside_paths() {
        with_owned_run("dataset-boundary", |run| {
            let dataset = generate_dataset(run, DatasetScale::Ci, DATASET_SEED).unwrap();
            let outside = run.root().join("outside.jpg");

            let error = dataset
                .validate_mutation_paths(&[outside])
                .unwrap_err()
                .to_string();
            assert_eq!(
                error,
                "performance dataset mutation refused path outside owned source roots"
            );
        });
    }
}
