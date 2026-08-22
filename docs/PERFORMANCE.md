# PureWall performance harness

PureWall includes a feature-gated Windows performance harness for repeatable local measurements. It exercises production scanner, SQLite, playback-selection, thumbnail, preview, and media-queue code without starting Tauri.

The results describe one recorded environment. They are not a universal Windows performance claim.

## Safety boundary

The harness is compiled only with the Cargo `performance-harness` feature and runs only after the explicit `--performance-harness` CLI marker. It exits before Tauri setup, so a harness run does not start the GUI, native watcher, tray, widget, wallpaper application, or normal application services.

Synthetic data, SQLite files, and derivative caches live under a marker-owned temporary run root. The harness never falls back to PureWall AppData, a real wallpaper library, or user-selected source folders. It does not access the registry, Recycle Bin, Windows wallpaper settings, or unrelated system settings.

Successful runs clean their owned temporary root unless `-KeepData` is supplied. Failed runs retain the root and print its path for diagnosis. Cleanup validates the exact marker, path identity, containment, and Windows reparse boundary before removal; do not substitute a broader path for the printed root.

Reports are local evidence. The repository ignores `/output/`, including performance JSON and Markdown reports. Do not commit benchmark output as a product claim.

## Small CI contract

```powershell
npm run test:performance-harness
```

This is the CI command. It compiles the feature-gated harness and runs its contract tests with only the deterministic 120-item, two-source synthetic fixture. It verifies dataset generation, safety checks, scenario contracts, report I/O, comparison rules, and isolated CI-scale orchestration. CI does not generate the 10,000-item Standard dataset or the 100,000-item Stress dataset.

## Run commands

Run commands use the PowerShell 5.1-compatible wrapper. `-Report` may name a new file, but it must be an absolute or resolvable `.json` path whose parent can be created. Every JSON report has an adjacent Markdown summary.

### CI fixture: 120 items, two roots

```powershell
$report = Join-Path (Resolve-Path .).Path 'output/performance/ci.json'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-performance-harness.ps1 `
  -Mode Run -Scale Ci -Report $report
```

### Standard baseline: 10,000 items, one root

```powershell
$report = Join-Path (Resolve-Path .).Path 'output/performance/standard-baseline.json'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-performance-harness.ps1 `
  -Mode Run -Scale Standard -Report $report
```

Standard is the normal local baseline scale. Run baseline and candidate measurements on the same machine, with the same build profile and effective harness configuration.

### Stress: 100,000 items across ten roots

```powershell
$report = Join-Path (Resolve-Path .).Path 'output/performance/stress.json'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-performance-harness.ps1 `
  -Mode Run -Scale Stress -AllowStress -Report $report
```

Stress requires the additional `-AllowStress` opt-in. It is not a CI gate and should be run only when its disk, time, and memory cost is intentional. The presence of this command does not mean a Stress run has been completed.

Add `-KeepData` to a Run command only when the synthetic root is needed for investigation. Compare mode does not accept `-KeepData` or `-AllowStress`.

## Compare commands

```powershell
$baseline = Join-Path (Resolve-Path .).Path 'output/performance/standard-baseline.json'
$candidate = Join-Path (Resolve-Path .).Path 'output/performance/standard-candidate.json'
$comparison = Join-Path (Resolve-Path .).Path 'output/performance/standard-comparison.json'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run-performance-harness.ps1 `
  -Mode Compare -Baseline $baseline -Candidate $candidate -Report $comparison
```

In Compare mode, `-Report` is the comparison output. Baseline, candidate, and output must be three distinct absolute `.json` paths. The command writes comparison JSON and Markdown, then exits non-zero for `regressed`, `failed`, or `not-comparable`.

A comparison is accepted only when both reports are valid passed runs and have the same report/scenario contract, environment fingerprint, dataset fingerprint, scenario IDs, cache states, and critical markers. Git commits may differ. An incomplete report publication marker causes the reader and comparator to fail closed.

## Measurement definitions

- `cold-derivative` means the scenario uses a fresh empty derivative cache and measures the production generation path.
- `warm-derivative` means the required derivative is prepared before timing and the measured operation must use the production cache-hit path.
- `not-applicable` is used for scanner, database, query, playback-selection, and queue scenarios without a derivative-cache state.
- Timing records every raw elapsed-microsecond sample and derives nearest-rank median and P95 from a sorted copy.
- Peak working set is sampled for the measured production operation. Test preparation, result validation, and release of the previous sample stay outside the measurement boundary.

The environment fingerprint contains OS, architecture, CPU, logical-core count, installed memory, Rust profile, and an effective-configuration digest. The dataset fingerprint contains dataset schema, deterministic seed, item count, source count, and a logical manifest digest. Reports also carry the report schema, scenario contract, Git commit, and generation time.

## Comparison gates

Phase 7A uses an untouched Standard run to identify a hotspot before any production optimization is selected.

- A candidate-selected hotspot must improve median time by at least 20%; otherwise it is `regressed`.
- Every other critical scenario must keep median, P95, and peak working set within a 5% regression guardrail.
- Missing evidence, failed scenarios, timeouts, report errors, duplicate or missing scenarios, incompatible fingerprints, and zero baselines fail closed as `failed` or `not-comparable`.
- Verdicts are never averaged across scenarios; the worst applicable verdict determines the overall result.

Do not edit a baseline report to select a hotspot. Hotspot selection belongs to the candidate only after the untouched baseline has been inspected and documented.

## Failure handling

A generation failure writes a failed report with the requested dataset counts and no fabricated samples. A later scenario or cleanup failure preserves completed records, records the original error, exits non-zero, and retains the owned data path when possible. If cleanup had already started, the error explicitly warns that remaining data may be partial.

JSON and Markdown publication is guarded by an adjacent `.json.publishing` marker. Readers reject the report while that marker exists. PureWall resumes or removes only a marker with its exact owned identity; an unrelated same-name file is preserved and causes the run to stop before creating a data root.
