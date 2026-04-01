mod classifier_trait;
mod registry;
mod schema;
mod types;

pub use classifier_trait::Classifier;
pub use registry::ClassifierRegistry;
pub use schema::CLASSIFICATIONS_TABLE_SQL;
pub use types::{ClassificationId, ClassificationResult, ClassifierStatus, ClassifierTrigger};
