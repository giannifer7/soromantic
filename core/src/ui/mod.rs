use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ViewMode {
    #[default]
    Library,
    Related(i64),
    Models,
    Studios,
    ModelDetails(String),
    StudioDetails(String),
}
