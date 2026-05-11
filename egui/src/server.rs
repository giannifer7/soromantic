//! Internal event types for communication between `BatchManager` and UI

use serde_json::Value;

#[derive(Debug)]
pub enum InternalEvent {
    /// Batch processing events from `BatchManager`
    BatchEvent(Value),
}
