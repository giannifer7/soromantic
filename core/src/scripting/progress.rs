//! Progress reporting module for Rune scripts.
//!
//! Provides a thread-local channel for scripts to report progress back to the UI.

use rune::Module;
use std::cell::RefCell;
use std::sync::mpsc::Sender;

/// Progress update sent from scripts to the UI.
#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    /// Step identifier (e.g., "fetch", "parse", "`extract_title`")
    pub step: String,
    /// Human-readable message
    pub message: String,
}

thread_local! {
    static PROGRESS_SENDER: RefCell<Option<Sender<ProgressUpdate>>> = const { RefCell::new(None) };
}

/// Initialize the progress sender for the current thread.
/// Must be called before running a script that uses progress reporting.
pub fn init(sender: Sender<ProgressUpdate>) {
    PROGRESS_SENDER.with(|s| {
        *s.borrow_mut() = Some(sender);
    });
}

/// Clear the progress sender after script execution.
pub fn cleanup() {
    PROGRESS_SENDER.with(|s| {
        *s.borrow_mut() = None;
    });
}

/// Create the progress module for Rune scripts.
///
/// # Errors
///
/// Returns an error if the module cannot be created.
pub fn module() -> anyhow::Result<Module, rune::ContextError> {
    let mut m = Module::with_item(["progress"])?;
    m.function_meta(report)?;
    Ok(m)
}

/// Report progress from a Rune script.
///
/// Called as `progress::report("step", "message")` from Rune.
#[rune::function]
fn report(step: &str, message: &str) {
    PROGRESS_SENDER.with(|s| {
        if let Some(sender) = s.borrow().as_ref() {
            let _ = sender.send(ProgressUpdate {
                step: step.to_string(),
                message: message.to_string(),
            });
        }
    });
}
