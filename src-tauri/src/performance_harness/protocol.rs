use serde::{Deserialize, Serialize};

pub(crate) const REPORT_SCHEMA_VERSION: u32 = 1;
pub(crate) const SCENARIO_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CacheState {
    NotApplicable,
    ColdDerivative,
    WarmDerivative,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RunStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EnvironmentFingerprint {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub logical_cores: usize,
    pub installed_memory_bytes: u64,
    pub rust_profile: String,
    pub config_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct DatasetFingerprint {
    pub schema_version: u32,
    pub seed: u64,
    pub item_count: usize,
    pub source_count: usize,
    pub manifest_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScenarioSamples {
    pub elapsed_micros: Vec<u64>,
    pub median_micros: u64,
    pub p95_micros: u64,
    pub peak_working_set_bytes: u64,
}

impl ScenarioSamples {
    pub(crate) fn from_elapsed_micros(
        elapsed_micros: Vec<u64>,
        peak_working_set_bytes: u64,
    ) -> anyhow::Result<Self> {
        if elapsed_micros.is_empty() {
            return Err(anyhow::anyhow!("elapsed_micros must not be empty"));
        }

        let mut sorted = elapsed_micros.clone();
        sorted.sort_unstable();
        Ok(Self {
            median_micros: nearest_rank(&sorted, 50),
            p95_micros: nearest_rank(&sorted, 95),
            elapsed_micros,
            peak_working_set_bytes,
        })
    }
}

fn nearest_rank(sorted: &[u64], percentile: usize) -> u64 {
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1)]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScenarioRecord {
    pub id: String,
    pub critical: bool,
    pub selected_hotspot: bool,
    pub cache_state: CacheState,
    pub status: RunStatus,
    pub samples: ScenarioSamples,
    pub expected_count: Option<u64>,
    pub actual_count: Option<u64>,
    pub database_bytes: Option<u64>,
    pub wal_bytes: Option<u64>,
    pub cache_hits: Option<u64>,
    pub cache_misses: Option<u64>,
    pub generated_derivatives: Option<u64>,
    pub queue_peak_pending: Option<u64>,
    pub timeout_count: u64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BenchmarkReport {
    pub schema_version: u32,
    pub scenario_contract_version: u32,
    pub git_commit: String,
    pub generated_at_utc: String,
    pub environment: EnvironmentFingerprint,
    pub dataset: DatasetFingerprint,
    pub scenarios: Vec<ScenarioRecord>,
    pub run_status: RunStatus,
    pub errors: Vec<String>,
}

#[cfg(test)]
pub(crate) fn protocol_test_report(
    digest: &str,
    median: u64,
    p95: u64,
    memory: u64,
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        scenario_contract_version: SCENARIO_CONTRACT_VERSION,
        git_commit: "0000000000000000000000000000000000000000".into(),
        generated_at_utc: "2026-08-13T00:00:00Z".into(),
        environment: EnvironmentFingerprint {
            os: "windows".into(),
            arch: "x86_64".into(),
            cpu: "test-cpu".into(),
            logical_cores: 8,
            installed_memory_bytes: 16 * 1024 * 1024 * 1024,
            rust_profile: "release".into(),
            config_digest: "config-v1".into(),
        },
        dataset: DatasetFingerprint {
            schema_version: 1,
            seed: 1,
            item_count: 120,
            source_count: 2,
            manifest_digest: Some(digest.into()),
        },
        scenarios: vec![ScenarioRecord {
            id: "query.page.first".into(),
            critical: true,
            selected_hotspot: false,
            cache_state: CacheState::NotApplicable,
            status: RunStatus::Passed,
            samples: ScenarioSamples {
                elapsed_micros: vec![median, p95],
                median_micros: median,
                p95_micros: p95,
                peak_working_set_bytes: memory,
            },
            expected_count: Some(120),
            actual_count: Some(120),
            database_bytes: None,
            wal_bytes: None,
            cache_hits: None,
            cache_misses: None,
            generated_derivatives: None,
            queue_peak_pending: None,
            timeout_count: 0,
            errors: Vec::new(),
        }],
        run_status: RunStatus::Passed,
        errors: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_round_trips_with_stable_serde_spellings() {
        let report = protocol_test_report("same", 100, 120, 1_000);
        let json = serde_json::to_value(&report).expect("serialize report");

        assert_eq!(json["scenarios"][0]["cache_state"], "not-applicable");
        assert_eq!(json["scenarios"][0]["status"], "passed");
        assert_eq!(json["dataset"]["manifest_digest"], "same");

        let decoded: BenchmarkReport = serde_json::from_value(json).expect("deserialize report");
        assert_eq!(decoded, report);
    }

    #[test]
    fn sample_summary_uses_nearest_rank_on_a_sorted_copy() {
        let samples = ScenarioSamples::from_elapsed_micros(vec![50, 10, 40, 20, 30], 1_024)
            .expect("nonempty samples");

        assert_eq!(samples.elapsed_micros, vec![50, 10, 40, 20, 30]);
        assert_eq!(samples.median_micros, 30);
        assert_eq!(samples.p95_micros, 50);
        assert_eq!(samples.peak_working_set_bytes, 1_024);
    }

    #[test]
    fn empty_sample_summary_is_rejected() {
        let error = ScenarioSamples::from_elapsed_micros(Vec::new(), 0)
            .expect_err("empty samples must fail");
        assert_eq!(error.to_string(), "elapsed_micros must not be empty");
    }
}
