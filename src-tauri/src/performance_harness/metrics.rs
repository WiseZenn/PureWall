use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::{error::Error as StdError, fmt};

#[cfg(not(windows))]
use anyhow::bail;
use anyhow::{anyhow, ensure, Context, Result};
use sha2::{Digest, Sha256};

use super::protocol::{EnvironmentFingerprint, ScenarioSamples};

const WORKING_SET_SAMPLE_INTERVAL: Duration = Duration::from_millis(10);
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunMetadata {
    pub(crate) git_commit: String,
    pub(crate) generated_at_utc: String,
    pub(crate) environment: EnvironmentFingerprint,
}

#[derive(Debug)]
pub(crate) enum DeadlineError {
    Timeout { timeout: Duration },
    Failed(anyhow::Error),
}

impl fmt::Display for DeadlineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { timeout } => write!(
                formatter,
                "performance scenario timed out after {} ms",
                timeout.as_millis()
            ),
            Self::Failed(error) => error.fmt(formatter),
        }
    }
}

impl StdError for DeadlineError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Timeout { .. } => None,
            Self::Failed(error) => Some(error.as_ref()),
        }
    }
}

pub(crate) fn measure_operation<T>(
    sample_count: usize,
    operation: impl FnMut(usize) -> Result<T>,
) -> Result<(ScenarioSamples, T)> {
    measure_operation_validated(sample_count, operation, |_, _| Ok(()))
}

pub(crate) fn measure_operation_validated<T>(
    sample_count: usize,
    operation: impl FnMut(usize) -> Result<T>,
    validate: impl FnMut(usize, &T) -> Result<()>,
) -> Result<(ScenarioSamples, T)> {
    ensure!(sample_count > 0, "sample count must be positive");
    let sampler = WorkingSetSampler::start()?;
    measure_operation_validated_with_sampler(sample_count, sampler, operation, validate)
}

pub(crate) fn measure_operation_with_setup_validated<Setup, T>(
    sample_count: usize,
    prepare: impl FnMut(usize) -> Result<Setup>,
    operation: impl FnMut(usize, Setup) -> Result<T>,
    validate: impl FnMut(usize, &T) -> Result<()>,
) -> Result<(ScenarioSamples, T)> {
    ensure!(sample_count > 0, "sample count must be positive");
    let sampler = WorkingSetSampler::start()?;
    measure_operation_with_setup_validated_with_sampler(
        sample_count,
        sampler,
        prepare,
        operation,
        validate,
    )
}

trait OperationWorkingSetSampler: Sized {
    fn begin_operation(&mut self) -> Result<()>;
    fn end_operation(&mut self) -> Result<()>;
    fn finish(self) -> Result<u64>;
}

fn measure_operation_validated_with_sampler<T, S>(
    sample_count: usize,
    sampler: S,
    mut operation: impl FnMut(usize) -> Result<T>,
    validate: impl FnMut(usize, &T) -> Result<()>,
) -> Result<(ScenarioSamples, T)>
where
    S: OperationWorkingSetSampler,
{
    measure_operation_with_setup_validated_with_sampler(
        sample_count,
        sampler,
        |_| Ok(()),
        |sample_index, ()| operation(sample_index),
        validate,
    )
}

fn measure_operation_with_setup_validated_with_sampler<Setup, T, S>(
    sample_count: usize,
    mut sampler: S,
    mut prepare: impl FnMut(usize) -> Result<Setup>,
    mut operation: impl FnMut(usize, Setup) -> Result<T>,
    mut validate: impl FnMut(usize, &T) -> Result<()>,
) -> Result<(ScenarioSamples, T)>
where
    S: OperationWorkingSetSampler,
{
    ensure!(sample_count > 0, "sample count must be positive");
    let measured: Result<(Vec<u64>, T)> = (|| {
        let mut elapsed_micros = Vec::with_capacity(sample_count);
        let mut final_value = None;
        for sample_index in 0..sample_count {
            drop(final_value.take());
            let setup = prepare(sample_index)?;
            sampler.begin_operation()?;
            let started = Instant::now();
            let value = operation(sample_index, setup);
            elapsed_micros.push(micros_u64(started.elapsed()));
            sampler.end_operation()?;
            let value = value?;
            validate(sample_index, &value)?;
            final_value = Some(value);
        }
        Ok((
            elapsed_micros,
            final_value.context("measurement produced no value")?,
        ))
    })();
    let peak_working_set_bytes = sampler.finish()?;
    let (elapsed_micros, final_value) = measured?;
    Ok((
        ScenarioSamples::from_elapsed_micros(elapsed_micros, peak_working_set_bytes)?,
        final_value,
    ))
}

pub(crate) fn run_with_deadline<T, F>(
    timeout: Duration,
    operation: F,
) -> std::result::Result<T, DeadlineError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    if timeout == Duration::ZERO {
        return Err(DeadlineError::Failed(anyhow!(
            "performance scenario timeout must be positive"
        )));
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = thread::Builder::new()
        .name("purewall-performance-scenario".into())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(operation));
            let _ = sender.send(outcome);
        })
        .context("spawn performance scenario worker")
        .map_err(DeadlineError::Failed)?;

    match receiver.recv_timeout(timeout) {
        Ok(Ok(result)) => {
            worker.join().map_err(|_| {
                DeadlineError::Failed(anyhow!("performance scenario worker panicked"))
            })?;
            result.map_err(DeadlineError::Failed)
        }
        Ok(Err(_)) => {
            let _ = worker.join();
            Err(DeadlineError::Failed(anyhow!(
                "performance scenario worker panicked"
            )))
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            drop(worker);
            Err(DeadlineError::Timeout { timeout })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = worker.join();
            Err(DeadlineError::Failed(anyhow!(
                "performance scenario worker disconnected before sending a result"
            )))
        }
    }
}

pub(crate) fn collect_run_metadata(policy: super::scenarios::SamplePolicy) -> Result<RunMetadata> {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("Cargo manifest directory has no repository parent")?;
    let output = Command::new("git")
        .arg("-C")
        .arg(repository_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("read performance harness Git commit")?;
    ensure!(
        output.status.success(),
        "git rev-parse HEAD failed with status {}",
        output.status
    );
    let git_commit = String::from_utf8(output.stdout)
        .context("git rev-parse HEAD was not UTF-8")?
        .trim()
        .to_string();
    ensure!(
        git_commit.len() == 40 && git_commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "performance Git commit must be exactly 40 ASCII hexadecimal characters"
    );

    Ok(RunMetadata {
        git_commit,
        generated_at_utc: chrono::Utc::now().to_rfc3339(),
        environment: EnvironmentFingerprint {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu: std::env::var("PROCESSOR_IDENTIFIER")
                .unwrap_or_else(|_| "unknown-processor".to_string()),
            logical_cores: thread::available_parallelism()
                .context("read logical processor count")?
                .get(),
            installed_memory_bytes: installed_memory_bytes()?,
            rust_profile: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "release".to_string()
            },
            config_digest: configuration_digest(policy),
        },
    })
}

fn configuration_digest(policy: super::scenarios::SamplePolicy) -> String {
    let input = format!(
        "scenario-contract={};workingset-ms={};{};scan-depth={};scan-images={};scan-entries={};{};{}",
        super::protocol::SCENARIO_CONTRACT_VERSION,
        WORKING_SET_SAMPLE_INTERVAL.as_millis(),
        super::scenarios::performance_config_descriptor(policy),
        crate::scanner::MAX_SCAN_DEPTH,
        crate::scanner::MAX_SCAN_IMAGES,
        crate::scanner::MAX_SCAN_ENTRIES,
        crate::media_queue::MediaJobQueue::performance_config_descriptor(),
        crate::thumbnails::ThumbnailCache::performance_config_descriptor(),
    );
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

struct WorkingSetSampler {
    stop: Arc<AtomicBool>,
    active: Arc<AtomicBool>,
    peak: Arc<AtomicU64>,
    worker: Option<JoinHandle<Result<()>>>,
}

impl WorkingSetSampler {
    fn start() -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let active = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU64::new(0));
        let worker_stop = Arc::clone(&stop);
        let worker_active = Arc::clone(&active);
        let worker_peak = Arc::clone(&peak);
        let worker = thread::Builder::new()
            .name("purewall-working-set-sampler".into())
            .spawn(move || {
                while !worker_stop.load(Ordering::Acquire) {
                    thread::sleep(WORKING_SET_SAMPLE_INTERVAL);
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    if worker_active.load(Ordering::Acquire) {
                        let sample = current_working_set_bytes()?;
                        if worker_active.load(Ordering::Acquire) {
                            worker_peak.fetch_max(sample, Ordering::AcqRel);
                        }
                    }
                }
                Ok(())
            })
            .context("spawn working-set sampler")?;
        Ok(Self {
            stop,
            active,
            peak,
            worker: Some(worker),
        })
    }
}

impl OperationWorkingSetSampler for WorkingSetSampler {
    fn begin_operation(&mut self) -> Result<()> {
        self.active.store(true, Ordering::Release);
        let sample = current_working_set_bytes().context("sample current working set")?;
        self.peak.fetch_max(sample, Ordering::AcqRel);
        Ok(())
    }

    fn end_operation(&mut self) -> Result<()> {
        self.active.store(false, Ordering::Release);
        Ok(())
    }

    fn finish(mut self) -> Result<u64> {
        self.active.store(false, Ordering::Release);
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .context("working-set sampler worker is missing")?
            .join()
            .map_err(|_| anyhow!("working-set sampler panicked"))??;
        Ok(self.peak.load(Ordering::Acquire))
    }
}

impl Drop for WorkingSetSampler {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn micros_u64(duration: Duration) -> u64 {
    duration.as_micros().try_into().unwrap_or(u64::MAX)
}

#[cfg(windows)]
fn current_working_set_bytes() -> Result<u64> {
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let mut counters = PROCESS_MEMORY_COUNTERS {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>()
            .try_into()
            .context("process memory counter size does not fit in u32")?,
        ..Default::default()
    };
    let success =
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
    ensure!(success.as_bool(), "K32GetProcessMemoryInfo failed");
    counters
        .WorkingSetSize
        .try_into()
        .context("current working set does not fit in u64")
}

#[cfg(not(windows))]
fn current_working_set_bytes() -> Result<u64> {
    bail!("current working-set sampling is supported only on Windows")
}

#[cfg(windows)]
fn installed_memory_bytes() -> Result<u64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>()
            .try_into()
            .context("memory status size does not fit in u32")?,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut status) }.context("GlobalMemoryStatusEx failed")?;
    Ok(status.ullTotalPhys)
}

#[cfg(not(windows))]
fn installed_memory_bytes() -> Result<u64> {
    bail!("installed-memory sampling is supported only on Windows")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::HashSet;
    use std::rc::Rc;
    use std::time::Duration;

    use super::*;

    struct ControllableSampler {
        current: Rc<Cell<u64>>,
        active: Rc<Cell<bool>>,
        peak: u64,
    }

    impl OperationWorkingSetSampler for ControllableSampler {
        fn begin_operation(&mut self) -> Result<()> {
            self.active.set(true);
            Ok(())
        }

        fn end_operation(&mut self) -> Result<()> {
            self.peak = self.peak.max(self.current.get());
            self.active.set(false);
            Ok(())
        }

        fn finish(self) -> Result<u64> {
            Ok(self.peak)
        }
    }

    #[test]
    fn scan_validator_hashset_allocation_is_outside_working_set_sampling() {
        let current = Rc::new(Cell::new(10));
        let active = Rc::new(Cell::new(false));
        let operation_current = Rc::clone(&current);
        let validator_current = Rc::clone(&current);
        let validator_active = Rc::clone(&active);

        let (samples, value) = measure_operation_validated_with_sampler(
            1,
            ControllableSampler {
                current,
                active,
                peak: 0,
            },
            move |_| {
                operation_current.set(100);
                Ok(vec!["a", "b", "c"])
            },
            move |_, paths| {
                assert!(
                    !validator_active.get(),
                    "sampler must pause before validation"
                );
                validator_current.set(1_000);
                let identities = paths.iter().copied().collect::<HashSet<_>>();
                assert_eq!(identities.len(), 3);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(value.len(), 3);
        assert_eq!(samples.peak_working_set_bytes, 100);
    }

    #[test]
    fn per_sample_setup_runs_with_sampling_paused_before_the_timed_operation() {
        let current = Rc::new(Cell::new(10));
        let active = Rc::new(Cell::new(false));
        let setup_active = Rc::clone(&active);
        let operation_active = Rc::clone(&active);
        let validator_active = Rc::clone(&active);
        let setup_count = Rc::new(Cell::new(0));
        let observed_setup_count = Rc::clone(&setup_count);

        let (samples, value) = measure_operation_with_setup_validated_with_sampler(
            3,
            ControllableSampler {
                current,
                active,
                peak: 0,
            },
            move |_| {
                assert!(!setup_active.get());
                setup_count.set(setup_count.get() + 1);
                Ok(vec![0_u8; 256])
            },
            move |_, mut backlog| {
                assert!(operation_active.get());
                backlog.push(1);
                Ok(backlog.len())
            },
            move |_, length| {
                assert!(!validator_active.get());
                assert_eq!(*length, 257);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(samples.elapsed_micros.len(), 3);
        assert_eq!(value, 257);
        assert_eq!(observed_setup_count.get(), 3);
    }

    #[test]
    fn measurement_has_samples_percentiles_and_working_set() {
        let (samples, value) = measure_operation(3, |_| Ok(vec![7_u8; 64 * 1024].len())).unwrap();
        assert_eq!(value, 64 * 1024);
        assert_eq!(samples.elapsed_micros.len(), 3);
        assert!(samples.peak_working_set_bytes > 0);
        assert!(samples.p95_micros >= samples.median_micros);
    }

    #[test]
    fn deadline_returns_a_stable_timeout_error() {
        let error = run_with_deadline(Duration::from_millis(10), || {
            std::thread::sleep(Duration::from_millis(100));
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(
            &error,
            DeadlineError::Timeout { timeout } if *timeout == Duration::from_millis(10)
        ));
        assert_eq!(
            error.to_string(),
            "performance scenario timed out after 10 ms"
        );
    }

    #[test]
    fn deadline_maps_success_and_worker_panic_explicitly() {
        assert_eq!(
            run_with_deadline(Duration::from_secs(1), || Ok::<_, anyhow::Error>(7)).unwrap(),
            7
        );

        let error = run_with_deadline(Duration::from_secs(1), || -> anyhow::Result<()> {
            panic!("sentinel deadline panic")
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "performance scenario worker panicked");
    }

    #[test]
    fn config_digest_tracks_the_effective_sample_policy() {
        let ci = super::super::scenarios::SamplePolicy::ci();
        let custom = super::super::scenarios::SamplePolicy {
            heavy: ci.heavy + 1,
            short: ci.short + 2,
            scenario_timeout: ci.scenario_timeout + Duration::from_secs(1),
        };

        assert_ne!(configuration_digest(ci), configuration_digest(custom));
    }

    #[test]
    fn run_metadata_has_strict_comparable_fingerprints() {
        let metadata = collect_run_metadata(super::super::scenarios::SamplePolicy::ci()).unwrap();
        assert_eq!(metadata.git_commit.len(), 40);
        assert!(metadata
            .git_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));
        assert!(chrono::DateTime::parse_from_rfc3339(&metadata.generated_at_utc).is_ok());
        assert!(!metadata.environment.os.is_empty());
        assert!(!metadata.environment.arch.is_empty());
        assert!(!metadata.environment.cpu.is_empty());
        assert!(metadata.environment.logical_cores > 0);
        assert!(metadata.environment.installed_memory_bytes > 0);
        assert_eq!(metadata.environment.config_digest.len(), 64);
        assert!(metadata
            .environment
            .config_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn dropping_a_sampler_signals_its_worker_to_stop() {
        let stop = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&stop);
        let worker = std::thread::spawn({
            let worker_stop = Arc::clone(&stop);
            move || {
                while !worker_stop.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                Ok(())
            }
        });
        let sampler = WorkingSetSampler {
            stop: Arc::clone(&stop),
            active: Arc::new(AtomicBool::new(false)),
            peak: Arc::new(AtomicU64::new(1)),
            worker: Some(worker),
        };

        drop(sampler);
        let stopped = observed.load(Ordering::Acquire);
        if !stopped {
            observed.store(true, Ordering::Release);
        }
        assert!(stopped, "dropping a sampler must stop its worker");
    }
}
