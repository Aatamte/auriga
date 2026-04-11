use auriga_types::{ClassificationResult, ClassifierConfig, ClassifierTrigger, Trace, Turn};

/// Trait that individual classifier implementations must satisfy.
///
/// Each classifier has a unique name, a trigger configuration, and a
/// `classify` method that examines a trace and its turns to produce
/// zero or more detection results.
pub trait Classifier: Send + Sync {
    /// Unique identifier for this classifier (e.g. "token-budget-check").
    fn name(&self) -> &str;

    /// When this classifier should run.
    fn trigger(&self) -> ClassifierTrigger;

    /// The full config, if this classifier was loaded from a config file.
    fn config(&self) -> Option<&ClassifierConfig> {
        None
    }

    /// Analyze a trace and its turns, returning zero or more results.
    ///
    /// For incremental triggers this may be a partial turn list;
    /// for on-complete triggers the turn list is final.
    fn classify(&self, trace: &Trace, turns: &[Turn]) -> Vec<ClassificationResult>;
}
