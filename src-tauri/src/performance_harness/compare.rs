use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::protocol::{
    BenchmarkReport, CacheState, RunStatus, ScenarioRecord, ScenarioSamples, REPORT_SCHEMA_VERSION,
    SCENARIO_CONTRACT_VERSION,
};

pub(crate) const HOTSPOT_IMPROVEMENT_MIN: f64 = 0.20;
pub(crate) const CRITICAL_REGRESSION_MAX: f64 = 0.05;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Verdict {
    Improved,
    Stable,
    Regressed,
    Failed,
    NotComparable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ScenarioComparison {
    pub id: String,
    pub cache_state: CacheState,
    pub median_change: Option<f64>,
    pub p95_change: Option<f64>,
    pub peak_working_set_change: Option<f64>,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ComparisonReport {
    pub schema_version: u32,
    pub baseline_commit: String,
    pub candidate_commit: String,
    pub overall: Verdict,
    pub scenarios: Vec<ScenarioComparison>,
    pub errors: Vec<String>,
}

pub(crate) fn relative_change(baseline: u64, candidate: u64) -> Option<f64> {
    (baseline > 0).then(|| (candidate as f64 - baseline as f64) / baseline as f64)
}

pub(crate) fn compare_reports(
    baseline: &BenchmarkReport,
    candidate: &BenchmarkReport,
) -> ComparisonReport {
    let failed_reasons = run_failure_reasons("baseline", baseline)
        .into_iter()
        .chain(run_failure_reasons("candidate", candidate))
        .collect::<Vec<_>>();
    if !failed_reasons.is_empty() {
        return comparison_error(baseline, candidate, Verdict::Failed, failed_reasons);
    }

    let invalid_reasons = report_validation_errors("baseline", baseline)
        .into_iter()
        .chain(report_validation_errors("candidate", candidate))
        .collect::<Vec<_>>();
    if !invalid_reasons.is_empty() {
        return comparison_error(baseline, candidate, Verdict::Failed, invalid_reasons);
    }

    let mut incompatible = Vec::new();
    if baseline.schema_version != candidate.schema_version {
        incompatible.push("report schema versions differ".into());
    } else if baseline.schema_version != REPORT_SCHEMA_VERSION {
        incompatible.push(format!(
            "unsupported report schema version {}",
            baseline.schema_version
        ));
    }
    if baseline.scenario_contract_version != candidate.scenario_contract_version {
        incompatible.push("scenario contract versions differ".into());
    } else if baseline.scenario_contract_version != SCENARIO_CONTRACT_VERSION {
        incompatible.push(format!(
            "unsupported scenario contract version {}",
            baseline.scenario_contract_version
        ));
    }
    if baseline.environment != candidate.environment {
        incompatible.push("environment fingerprints differ".into());
    }
    if baseline.dataset != candidate.dataset {
        incompatible.push("dataset fingerprints differ".into());
    }
    if !incompatible.is_empty() {
        return comparison_error(baseline, candidate, Verdict::NotComparable, incompatible);
    }

    let baseline_index = match index_scenarios("baseline", &baseline.scenarios) {
        Ok(index) => index,
        Err(errors) => {
            return comparison_error(baseline, candidate, Verdict::NotComparable, errors);
        }
    };
    let candidate_index = match index_scenarios("candidate", &candidate.scenarios) {
        Ok(index) => index,
        Err(errors) => {
            return comparison_error(baseline, candidate, Verdict::NotComparable, errors);
        }
    };

    let mut hotspot_ids = HashSet::new();
    let mut hotspot_errors = Vec::new();
    for scenario in candidate
        .scenarios
        .iter()
        .filter(|scenario| scenario.selected_hotspot)
    {
        if !hotspot_ids.insert(scenario.id.as_str()) {
            hotspot_errors.push(format!(
                "candidate has duplicate selected hotspot id {}",
                scenario.id
            ));
        }
        if !baseline
            .scenarios
            .iter()
            .any(|baseline_scenario| baseline_scenario.id == scenario.id)
        {
            hotspot_errors.push(format!(
                "candidate selected hotspot id {} has no matching baseline scenario",
                scenario.id
            ));
        }
    }
    if !hotspot_errors.is_empty() {
        return comparison_error(baseline, candidate, Verdict::Failed, hotspot_errors);
    }

    let baseline_keys = baseline_index.keys().cloned().collect::<HashSet<_>>();
    let candidate_keys = candidate_index.keys().cloned().collect::<HashSet<_>>();
    if baseline_keys != candidate_keys {
        let mut errors = Vec::new();
        for key in baseline_keys.difference(&candidate_keys) {
            errors.push(format!(
                "candidate is missing scenario ({}, {})",
                key.0, key.1
            ));
        }
        for key in candidate_keys.difference(&baseline_keys) {
            errors.push(format!(
                "candidate has unknown scenario ({}, {})",
                key.0, key.1
            ));
        }
        errors.sort();
        return comparison_error(baseline, candidate, Verdict::NotComparable, errors);
    }

    let mut comparisons = Vec::with_capacity(baseline.scenarios.len());
    for baseline_scenario in &baseline.scenarios {
        let key = scenario_key(baseline_scenario);
        let candidate_scenario = candidate_index[&key];

        if baseline_scenario.critical != candidate_scenario.critical {
            return comparison_error(
                baseline,
                candidate,
                Verdict::NotComparable,
                vec![format!(
                    "scenario ({}, {}) changed its critical marker",
                    key.0, key.1
                )],
            );
        }
        if baseline_scenario.selected_hotspot {
            return comparison_error(
                baseline,
                candidate,
                Verdict::Failed,
                vec![format!(
                    "baseline scenario ({}, {}) must not select a hotspot",
                    key.0, key.1
                )],
            );
        }

        let disappeared = missing_evidence_fields(baseline_scenario, candidate_scenario);
        if !disappeared.is_empty() {
            return comparison_error(
                baseline,
                candidate,
                Verdict::Failed,
                vec![format!(
                    "candidate scenario ({}, {}) dropped evidence fields: {}",
                    key.0,
                    key.1,
                    disappeared.join(", ")
                )],
            );
        }

        comparisons.push(compare_scenario(baseline_scenario, candidate_scenario));
    }

    let overall = if comparisons
        .iter()
        .any(|comparison| comparison.verdict == Verdict::NotComparable)
    {
        Verdict::NotComparable
    } else if comparisons
        .iter()
        .any(|comparison| comparison.verdict == Verdict::Regressed)
    {
        Verdict::Regressed
    } else if comparisons
        .iter()
        .any(|comparison| comparison.verdict == Verdict::Improved)
    {
        Verdict::Improved
    } else {
        Verdict::Stable
    };

    ComparisonReport {
        schema_version: REPORT_SCHEMA_VERSION,
        baseline_commit: baseline.git_commit.clone(),
        candidate_commit: candidate.git_commit.clone(),
        overall,
        scenarios: comparisons,
        errors: Vec::new(),
    }
}

fn compare_scenario(baseline: &ScenarioRecord, candidate: &ScenarioRecord) -> ScenarioComparison {
    let median_change = relative_change(
        baseline.samples.median_micros,
        candidate.samples.median_micros,
    );
    let p95_change = relative_change(baseline.samples.p95_micros, candidate.samples.p95_micros);
    let peak_working_set_change = relative_change(
        baseline.samples.peak_working_set_bytes,
        candidate.samples.peak_working_set_bytes,
    );

    let (verdict, reasons) = if candidate.selected_hotspot {
        match median_change {
            Some(change) if change <= -HOTSPOT_IMPROVEMENT_MIN => (
                Verdict::Improved,
                vec!["selected hotspot median improved by at least 20%".into()],
            ),
            Some(_) => (
                Verdict::Regressed,
                vec!["selected hotspot median did not improve by at least 20%".into()],
            ),
            None => (
                Verdict::NotComparable,
                vec!["selected hotspot has a zero baseline median".into()],
            ),
        }
    } else if baseline.critical {
        let changes = [
            ("median", median_change),
            ("p95", p95_change),
            ("peak working set", peak_working_set_change),
        ];
        let missing = changes
            .iter()
            .filter_map(|(name, change)| change.is_none().then_some(*name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            (
                Verdict::NotComparable,
                vec![format!(
                    "critical scenario has zero baseline metrics: {}",
                    missing.join(", ")
                )],
            )
        } else {
            let regressions = changes
                .iter()
                .filter_map(|(name, change)| {
                    change
                        .filter(|value| *value > CRITICAL_REGRESSION_MAX)
                        .map(|_| *name)
                })
                .collect::<Vec<_>>();
            if regressions.is_empty() {
                (
                    Verdict::Stable,
                    vec!["critical median, p95, and peak working set stayed within 5%".into()],
                )
            } else {
                (
                    Verdict::Regressed,
                    vec![format!(
                        "critical metrics regressed by more than 5%: {}",
                        regressions.join(", ")
                    )],
                )
            }
        }
    } else {
        (
            Verdict::Stable,
            vec!["non-critical scenario has no automatic threshold".into()],
        )
    };

    ScenarioComparison {
        id: candidate.id.clone(),
        cache_state: candidate.cache_state,
        median_change,
        p95_change,
        peak_working_set_change,
        verdict,
        reasons,
    }
}

fn run_failure_reasons(label: &str, report: &BenchmarkReport) -> Vec<String> {
    let mut reasons = Vec::new();
    if report.run_status == RunStatus::Failed {
        reasons.push(format!("{label} run failed"));
    }
    reasons.extend(
        report
            .errors
            .iter()
            .map(|error| format!("{label} report error: {error}")),
    );
    for scenario in &report.scenarios {
        if scenario.status == RunStatus::Failed {
            reasons.push(format!("{label} scenario {} failed", scenario.id));
        }
        reasons.extend(
            scenario
                .errors
                .iter()
                .map(|error| format!("{label} scenario {} error: {error}", scenario.id)),
        );
        if scenario.timeout_count > 0 {
            reasons.push(format!(
                "{label} scenario {} recorded {} timeouts",
                scenario.id, scenario.timeout_count
            ));
        }
    }
    reasons
}

fn report_validation_errors(label: &str, report: &BenchmarkReport) -> Vec<String> {
    let mut errors = Vec::new();
    if report.git_commit.len() != 40
        || !report
            .git_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        errors.push(format!(
            "{label} git_commit must be 40 hexadecimal characters"
        ));
    }
    if chrono::DateTime::parse_from_rfc3339(&report.generated_at_utc).is_err() {
        errors.push(format!("{label} generated_at_utc must be RFC 3339"));
    }
    if report.environment.os.is_empty()
        || report.environment.arch.is_empty()
        || report.environment.cpu.is_empty()
        || report.environment.rust_profile.is_empty()
        || report.environment.config_digest.is_empty()
        || report.environment.logical_cores == 0
        || report.environment.installed_memory_bytes == 0
    {
        errors.push(format!("{label} environment fingerprint is incomplete"));
    }
    if report.dataset.schema_version == 0
        || report.dataset.item_count == 0
        || report.dataset.source_count == 0
        || report
            .dataset
            .manifest_digest
            .as_deref()
            .is_none_or(str::is_empty)
    {
        errors.push(format!("{label} dataset fingerprint is incomplete"));
    }
    if report.scenarios.is_empty() {
        errors.push(format!("{label} report has no scenarios"));
    }
    for scenario in &report.scenarios {
        if scenario.id.is_empty() {
            errors.push(format!("{label} scenario id must not be empty"));
        }
        match ScenarioSamples::from_elapsed_micros(
            scenario.samples.elapsed_micros.clone(),
            scenario.samples.peak_working_set_bytes,
        ) {
            Ok(summary)
                if summary.median_micros == scenario.samples.median_micros
                    && summary.p95_micros == scenario.samples.p95_micros => {}
            Ok(_) => errors.push(format!(
                "{label} scenario {} has inconsistent sample summaries",
                scenario.id
            )),
            Err(_) => errors.push(format!(
                "{label} scenario {} has no elapsed samples",
                scenario.id
            )),
        }
        if let (Some(expected), Some(actual)) = (scenario.expected_count, scenario.actual_count) {
            if expected != actual {
                errors.push(format!(
                    "{label} scenario {} count mismatch: expected {expected}, got {actual}",
                    scenario.id
                ));
            }
        }
    }
    errors
}

type ScenarioKey = (String, &'static str);

fn index_scenarios<'a>(
    label: &str,
    scenarios: &'a [ScenarioRecord],
) -> Result<HashMap<ScenarioKey, &'a ScenarioRecord>, Vec<String>> {
    let mut index = HashMap::with_capacity(scenarios.len());
    let mut errors = Vec::new();
    for scenario in scenarios {
        let key = scenario_key(scenario);
        if index.insert(key.clone(), scenario).is_some() {
            errors.push(format!(
                "{label} has duplicate scenario ({}, {})",
                key.0, key.1
            ));
        }
    }
    if errors.is_empty() {
        Ok(index)
    } else {
        Err(errors)
    }
}

fn scenario_key(scenario: &ScenarioRecord) -> ScenarioKey {
    (
        scenario.id.clone(),
        match scenario.cache_state {
            CacheState::NotApplicable => "not-applicable",
            CacheState::ColdDerivative => "cold-derivative",
            CacheState::WarmDerivative => "warm-derivative",
        },
    )
}

fn missing_evidence_fields(
    baseline: &ScenarioRecord,
    candidate: &ScenarioRecord,
) -> Vec<&'static str> {
    let mut missing = Vec::new();
    macro_rules! require_if_present {
        ($field:ident) => {
            if baseline.$field.is_some() && candidate.$field.is_none() {
                missing.push(stringify!($field));
            }
        };
    }

    require_if_present!(expected_count);
    require_if_present!(actual_count);
    require_if_present!(database_bytes);
    require_if_present!(wal_bytes);
    require_if_present!(cache_hits);
    require_if_present!(cache_misses);
    require_if_present!(generated_derivatives);
    require_if_present!(queue_peak_pending);
    missing
}

fn comparison_error(
    baseline: &BenchmarkReport,
    candidate: &BenchmarkReport,
    overall: Verdict,
    errors: Vec<String>,
) -> ComparisonReport {
    ComparisonReport {
        schema_version: REPORT_SCHEMA_VERSION,
        baseline_commit: baseline.git_commit.clone(),
        candidate_commit: candidate.git_commit.clone(),
        overall,
        scenarios: Vec::new(),
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::performance_harness::protocol::{
        protocol_test_report, CacheState, RunStatus, SCENARIO_CONTRACT_VERSION,
    };

    #[test]
    fn thresholds_and_fingerprints_are_enforced() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut improved = protocol_test_report("same", 80, 80, 1_000);
        improved.scenarios[0].selected_hotspot = true;
        assert_eq!(
            compare_reports(&baseline, &improved).overall,
            Verdict::Improved
        );
        assert_eq!(
            compare_reports(&baseline, &protocol_test_report("same", 106, 106, 1_060)).overall,
            Verdict::Regressed
        );
        assert_eq!(
            compare_reports(&baseline, &protocol_test_report("different", 80, 80, 1_000)).overall,
            Verdict::NotComparable
        );
    }

    #[test]
    fn failed_scenario_cannot_be_hidden_by_speed() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut candidate = protocol_test_report("same", 50, 50, 900);
        candidate.scenarios[0].status = RunStatus::Failed;
        candidate.scenarios[0]
            .errors
            .push("record count mismatch".into());
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Failed
        );
    }

    #[test]
    fn commit_may_differ_but_contract_environment_and_dataset_must_match() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut candidate = protocol_test_report("same", 100, 100, 1_000);
        candidate.git_commit = "1111111111111111111111111111111111111111".into();
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Stable
        );

        candidate.scenario_contract_version = SCENARIO_CONTRACT_VERSION + 1;
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::NotComparable
        );
        candidate.scenario_contract_version = SCENARIO_CONTRACT_VERSION;
        candidate.environment.config_digest = "config-v2".into();
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::NotComparable
        );
    }

    #[test]
    fn scenarios_match_by_unique_id_and_cache_state_without_averaging() {
        let mut baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut second = baseline.scenarios[0].clone();
        second.cache_state = CacheState::WarmDerivative;
        baseline.scenarios.push(second);

        let mut candidate = baseline.clone();
        candidate.scenarios.reverse();
        candidate.scenarios[0].samples.median_micros = 106;
        candidate.scenarios[0].samples.p95_micros = 106;
        candidate.scenarios[0].samples.elapsed_micros = vec![106, 106];

        let comparison = compare_reports(&baseline, &candidate);
        assert_eq!(comparison.overall, Verdict::Regressed);
        assert_eq!(comparison.scenarios.len(), 2);

        candidate.scenarios.push(candidate.scenarios[0].clone());
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::NotComparable
        );
    }

    #[test]
    fn missing_scenarios_are_not_comparable_and_unknown_hotspots_fail() {
        let mut baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut second = baseline.scenarios[0].clone();
        second.id = "query.page.last".into();
        baseline.scenarios.push(second);

        let mut missing = baseline.clone();
        missing.scenarios.pop();
        assert_eq!(
            compare_reports(&baseline, &missing).overall,
            Verdict::NotComparable
        );

        let mut unknown_hotspot = baseline.clone();
        unknown_hotspot.scenarios[1].id = "unknown.hotspot".into();
        unknown_hotspot.scenarios[1].selected_hotspot = true;
        assert_eq!(
            compare_reports(&baseline, &unknown_hotspot).overall,
            Verdict::Failed
        );
    }

    #[test]
    fn duplicate_hotspot_ids_across_cache_states_fail() {
        let mut baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut warm = baseline.scenarios[0].clone();
        warm.cache_state = CacheState::WarmDerivative;
        baseline.scenarios.push(warm);

        let mut candidate = baseline.clone();
        for scenario in &mut candidate.scenarios {
            scenario.selected_hotspot = true;
            scenario.samples.median_micros = 80;
            scenario.samples.p95_micros = 80;
            scenario.samples.elapsed_micros = vec![80, 80];
        }

        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Failed
        );
    }

    #[test]
    fn report_errors_timeouts_and_failed_runs_propagate() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);

        let mut report_error = protocol_test_report("same", 50, 50, 500);
        report_error.errors.push("dataset setup failed".into());
        assert_eq!(
            compare_reports(&baseline, &report_error).overall,
            Verdict::Failed
        );

        let mut failed_run = protocol_test_report("same", 50, 50, 500);
        failed_run.run_status = RunStatus::Failed;
        assert_eq!(
            compare_reports(&baseline, &failed_run).overall,
            Verdict::Failed
        );

        let mut timeout = protocol_test_report("same", 50, 50, 500);
        timeout.scenarios[0].timeout_count = 1;
        assert_eq!(
            compare_reports(&baseline, &timeout).overall,
            Verdict::Failed
        );
    }

    #[test]
    fn hotspot_requires_twenty_percent_median_improvement() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut candidate = protocol_test_report("same", 81, 100, 1_000);
        candidate.scenarios[0].selected_hotspot = true;
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Regressed
        );

        candidate.scenarios[0].samples.median_micros = 80;
        candidate.scenarios[0].samples.elapsed_micros = vec![80, 100];
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Improved
        );
    }

    #[test]
    fn non_hotspot_critical_metrics_each_have_a_five_percent_guardrail() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);

        for candidate in [
            protocol_test_report("same", 106, 106, 1_000),
            protocol_test_report("same", 100, 106, 1_000),
            protocol_test_report("same", 100, 100, 1_051),
        ] {
            assert_eq!(
                compare_reports(&baseline, &candidate).overall,
                Verdict::Regressed
            );
        }

        assert_eq!(
            compare_reports(&baseline, &protocol_test_report("same", 105, 105, 1_050)).overall,
            Verdict::Stable
        );
    }

    #[test]
    fn evidence_fields_cannot_disappear_but_are_not_thresholded() {
        let mut baseline = protocol_test_report("same", 100, 100, 1_000);
        baseline.scenarios[0].database_bytes = Some(100);
        baseline.scenarios[0].wal_bytes = Some(100);
        baseline.scenarios[0].cache_hits = Some(100);
        baseline.scenarios[0].cache_misses = Some(100);
        baseline.scenarios[0].generated_derivatives = Some(100);
        baseline.scenarios[0].queue_peak_pending = Some(100);

        let mut candidate = baseline.clone();
        candidate.scenarios[0].database_bytes = Some(1_000);
        candidate.scenarios[0].wal_bytes = Some(1_000);
        candidate.scenarios[0].cache_hits = Some(1);
        candidate.scenarios[0].cache_misses = Some(1_000);
        candidate.scenarios[0].generated_derivatives = Some(1_000);
        candidate.scenarios[0].queue_peak_pending = Some(1_000);
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Stable
        );

        candidate.scenarios[0].database_bytes = None;
        assert_eq!(
            compare_reports(&baseline, &candidate).overall,
            Verdict::Failed
        );
    }

    #[test]
    fn invalid_reports_and_zero_threshold_baselines_fail_closed() {
        let baseline = protocol_test_report("same", 100, 100, 1_000);
        let mut invalid = protocol_test_report("same", 100, 100, 1_000);
        invalid.git_commit = "not-a-commit".into();
        assert_eq!(
            compare_reports(&baseline, &invalid).overall,
            Verdict::Failed
        );

        let zero = protocol_test_report("same", 0, 0, 0);
        assert_eq!(
            compare_reports(&zero, &zero).overall,
            Verdict::NotComparable
        );
    }
}
