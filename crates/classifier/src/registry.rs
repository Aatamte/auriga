use auriga_types::{
    ClassificationResult, ClassifierConfig, ClassifierStatus, ClassifierTrigger, Trace, Turn,
};

use crate::Classifier;

struct ClassifierEntry {
    classifier: Box<dyn Classifier>,
    enabled: bool,
}

/// Holds registered classifiers and dispatches trace/turn data to them
/// based on trigger configuration and enabled state.
pub struct ClassifierRegistry {
    entries: Vec<ClassifierEntry>,
}

impl ClassifierRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a classifier (enabled by default). Panics on duplicate name.
    pub fn register(&mut self, classifier: Box<dyn Classifier>) {
        let name = classifier.name().to_string();
        assert!(
            !self.entries.iter().any(|e| e.classifier.name() == name),
            "duplicate classifier name: '{name}' is already registered",
        );
        self.entries.push(ClassifierEntry {
            classifier,
            enabled: true,
        });
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.classifier.name()).collect()
    }

    /// Enable or disable a classifier by name. Returns false if not found.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.classifier.name() == name)
        {
            entry.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|e| e.classifier.name() == name)
            .is_some_and(|e| e.enabled)
    }

    /// Summary info for all registered classifiers.
    pub fn classifiers_info(&self) -> Vec<ClassifierStatus> {
        self.entries
            .iter()
            .map(|e| ClassifierStatus {
                name: e.classifier.name().to_string(),
                trigger: e.classifier.trigger(),
                enabled: e.enabled,
            })
            .collect()
    }

    /// Full configs for all registered classifiers that have one.
    /// Returns (enabled, config) pairs.
    pub fn classifiers_with_configs(&self) -> Vec<(bool, &ClassifierConfig)> {
        self.entries
            .iter()
            .filter_map(|e| e.classifier.config().map(|c| (e.enabled, c)))
            .collect()
    }

    /// Run enabled classifiers whose trigger matches incremental events.
    /// Applies turn filtering before dispatching.
    pub fn run_incremental(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult> {
        self.run_filtered(turns, trace, |t: &ClassifierTrigger| t.runs_incremental())
    }

    /// Run enabled classifiers whose trigger matches on-complete events.
    /// Applies turn filtering before dispatching.
    pub fn run_on_complete(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult> {
        self.run_filtered(turns, trace, |t: &ClassifierTrigger| t.runs_on_complete())
    }

    fn run_filtered(
        &self,
        turns: &[Turn],
        trace: &Trace,
        phase_check: impl Fn(&ClassifierTrigger) -> bool,
    ) -> Vec<ClassificationResult> {
        let mut results = Vec::new();
        for entry in &self.entries {
            let trigger = entry.classifier.trigger();
            if !entry.enabled || !phase_check(&trigger) {
                continue;
            }
            if trigger.has_filter() {
                let filtered: Vec<Turn> =
                    trigger.filter_turns(turns).into_iter().cloned().collect();
                if filtered.is_empty() {
                    continue;
                }
                results.extend(entry.classifier.classify(trace, &filtered));
            } else {
                results.extend(entry.classifier.classify(trace, turns));
            }
        }
        results
    }
}

impl Default for ClassifierRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auriga_types::{
        AgentId, ClassificationId, ClassifierTrigger, TokenUsage, Trace, TraceId, TraceStatus,
        TriggerPhase, TurnFilter,
    };

    struct MockClassifier {
        name: &'static str,
        trigger: ClassifierTrigger,
        result_count: usize,
    }

    impl Classifier for MockClassifier {
        fn name(&self) -> &str {
            self.name
        }

        fn trigger(&self) -> ClassifierTrigger {
            self.trigger.clone()
        }

        fn classify(&self, trace: &Trace, _turns: &[Turn]) -> Vec<ClassificationResult> {
            (0..self.result_count)
                .map(|_| ClassificationResult {
                    id: ClassificationId::new(),
                    trace_id: trace.id,
                    classifier_name: self.name.into(),
                    timestamp: "2026-01-01T00:00:00Z".into(),
                    payload: serde_json::json!({"detected": true}),
                    notification: None,
                })
                .collect()
        }
    }

    fn mock(name: &'static str, trigger: ClassifierTrigger, count: usize) -> Box<dyn Classifier> {
        Box::new(MockClassifier {
            name,
            trigger,
            result_count: count,
        })
    }

    fn test_trace() -> Trace {
        Trace {
            id: TraceId::from_u128(1),
            agent_id: AgentId::from_u128(1),
            session_id: "s1".into(),
            status: TraceStatus::Active,
            started_at: "2026-01-01T00:00:00Z".into(),
            completed_at: None,
            turn_count: 0,
            token_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
            provider: "claude".into(),
            model: None,
        }
    }

    #[test]
    fn register_and_count() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        reg.register(mock("b", ClassifierTrigger::on_complete(), 1));
        assert_eq!(reg.count(), 2);
        assert_eq!(reg.names(), vec!["a", "b"]);
    }

    #[test]
    #[should_panic(expected = "duplicate classifier name")]
    fn duplicate_name_panics() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("dup", ClassifierTrigger::incremental(), 1));
        reg.register(mock("dup", ClassifierTrigger::on_complete(), 1));
    }

    #[test]
    fn run_incremental_only_fires_matching() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("inc", ClassifierTrigger::incremental(), 1));
        reg.register(mock("comp", ClassifierTrigger::on_complete(), 1));
        reg.register(mock(
            "both",
            ClassifierTrigger::new(TriggerPhase::Both, TurnFilter::default()),
            1,
        ));

        let trace = test_trace();
        let results = reg.run_incremental(&trace, &[]);
        let names: Vec<_> = results.iter().map(|r| r.classifier_name.as_str()).collect();
        assert!(names.contains(&"inc"));
        assert!(!names.contains(&"comp"));
        assert!(names.contains(&"both"));
    }

    #[test]
    fn run_on_complete_only_fires_matching() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("inc", ClassifierTrigger::incremental(), 1));
        reg.register(mock("comp", ClassifierTrigger::on_complete(), 1));
        reg.register(mock(
            "both",
            ClassifierTrigger::new(TriggerPhase::Both, TurnFilter::default()),
            1,
        ));

        let trace = test_trace();
        let results = reg.run_on_complete(&trace, &[]);
        let names: Vec<_> = results.iter().map(|r| r.classifier_name.as_str()).collect();
        assert!(!names.contains(&"inc"));
        assert!(names.contains(&"comp"));
        assert!(names.contains(&"both"));
    }

    #[test]
    fn empty_registry_returns_empty() {
        let reg = ClassifierRegistry::new();
        let trace = test_trace();
        assert!(reg.run_incremental(&trace, &[]).is_empty());
        assert!(reg.run_on_complete(&trace, &[]).is_empty());
    }

    #[test]
    fn multiple_results_collected() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("multi", ClassifierTrigger::incremental(), 3));

        let trace = test_trace();
        let results = reg.run_incremental(&trace, &[]);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn zero_result_classifier_produces_nothing() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("empty", ClassifierTrigger::incremental(), 0));

        let trace = test_trace();
        assert!(reg.run_incremental(&trace, &[]).is_empty());
    }

    #[test]
    fn disabled_classifier_skipped() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        reg.set_enabled("a", false);

        let trace = test_trace();
        assert!(reg.run_incremental(&trace, &[]).is_empty());
    }

    #[test]
    fn re_enabled_classifier_runs() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        reg.set_enabled("a", false);
        reg.set_enabled("a", true);

        let trace = test_trace();
        assert_eq!(reg.run_incremental(&trace, &[]).len(), 1);
    }

    #[test]
    fn set_enabled_unknown_returns_false() {
        let mut reg = ClassifierRegistry::new();
        assert!(!reg.set_enabled("nope", false));
    }

    #[test]
    fn is_enabled_default_true() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        assert!(reg.is_enabled("a"));
    }

    #[test]
    fn is_enabled_after_disable() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        reg.set_enabled("a", false);
        assert!(!reg.is_enabled("a"));
    }

    #[test]
    fn classifiers_info_returns_all() {
        let mut reg = ClassifierRegistry::new();
        reg.register(mock("a", ClassifierTrigger::incremental(), 1));
        reg.register(mock("b", ClassifierTrigger::on_complete(), 1));
        reg.set_enabled("b", false);

        let info = reg.classifiers_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].name, "a");
        assert!(info[0].enabled);
        assert_eq!(info[1].name, "b");
        assert!(!info[1].enabled);
    }
}
