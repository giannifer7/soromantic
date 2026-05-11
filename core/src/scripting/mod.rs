use anyhow::{Context as _, Result};
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Context, Diagnostics, Source, Sources, Vm};
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub mod glue;
pub mod html;
pub mod http;
pub mod progress;
pub mod util;

// Re-export progress types for convenience
pub use progress::ProgressUpdate;

/// Initialize a Rune context with default modules and our host functions.
///
/// # Errors
///
/// Returns an error if the context cannot be created or modules cannot be installed.
pub fn create_context() -> Result<Context> {
    let mut context = Context::with_default_modules()?;

    context.install(crate::scripting::http::module()?)?;
    context.install(crate::scripting::html::module()?)?;
    context.install(crate::scripting::util::module()?)?;
    context.install(crate::scripting::progress::module()?)?;

    Ok(context)
}

/// Execute a Rune script source.
///
/// # Errors
///
/// Returns an error if the script cannot be parsed, compiled, or executed.
pub fn run_script(source_code: &str) -> Result<rune::runtime::Value> {
    run_script_internal(source_code, None)
}

/// Execute a Rune script with progress reporting.
///
/// Progress updates are sent to the provided channel as the script executes.
///
/// # Errors
///
/// Returns an error if the script cannot be parsed, compiled, or executed.
pub fn run_script_with_progress(
    source_code: &str,
    progress_tx: Sender<ProgressUpdate>,
) -> Result<rune::runtime::Value> {
    run_script_internal(source_code, Some(progress_tx))
}

/// Internal script execution with optional progress reporting.
fn run_script_internal(
    source_code: &str,
    progress_tx: Option<Sender<ProgressUpdate>>,
) -> Result<rune::runtime::Value> {
    // Set up progress reporting if a sender was provided
    if let Some(tx) = progress_tx {
        progress::init(tx);
    }

    // Ensure cleanup happens even on error
    let result = run_script_core(source_code);

    progress::cleanup();

    result
}

/// Core script execution logic.
fn run_script_core(source_code: &str) -> Result<rune::runtime::Value> {
    let context = create_context()?;
    let mut vm = compile_and_init_vm(&context, source_code)?;
    let result = vm
        .call(["main"], ())
        .context("Failed to execute Rune script")?;

    Ok(result)
}

/// Run a scraper function from a script source.
/// # Errors
/// Returns error if script compilation or execution fails.
pub fn run_scraper_fn<T, F>(
    source_code: &str,
    function: &str,
    args: (String,), // Single URL argument for now
    progress_tx: Option<Sender<ProgressUpdate>>,
    f: F,
) -> Result<T>
where
    F: FnOnce(rune::runtime::Value) -> Result<T>,
{
    if let Some(tx) = progress_tx {
        progress::init(tx);
    }

    let result = (|| {
        let context = create_context()?;
        let mut vm = compile_and_init_vm(&context, source_code)?;

        let output = vm
            .call([function], args)
            .map_err(|e| anyhow::anyhow!("Runtime error in {function}: {e}"))?;

        f(output)
    })();

    progress::cleanup();
    result
}

/// Helper to compile user source and initialize VM.
///
/// # Errors
/// Returns error if compilation fails or context creation fails.
pub fn compile_and_init_vm(context: &Context, source_code: &str) -> Result<Vm> {
    let mut sources = Sources::new();
    let _ = sources.insert(Source::new("script", source_code)?);

    let runtime = Arc::new(context.runtime()?);

    let mut diagnostics = Diagnostics::new();

    let result = rune::prepare(&mut sources)
        .with_context(context)
        .with_diagnostics(&mut diagnostics)
        .build();

    if !diagnostics.is_empty() {
        let mut writer = StandardStream::stderr(ColorChoice::Always);
        diagnostics.emit(&mut writer, &sources)?;
    }

    let unit = result.context("Failed to build Rune script")?;
    Ok(Vm::new(runtime, Arc::new(unit)))
}

/// Progress callback for workflow operations.
pub type WorkflowProgressCallback = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// Helper to execute a scraper function in a blocking task and convert the result.
///
/// # Errors
/// Returns error if the blocking task failed or the scraper execution/conversion failed.
pub async fn execute_scraper_blocking<T, F>(
    script_source: String,
    url: String,
    tx: std::sync::mpsc::Sender<ProgressUpdate>,
    converter: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce(rune::runtime::Value) -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        run_scraper_fn(&script_source, "scrape", (url,), Some(tx), converter)
    })
    .await?
}

/// Helper to spawn a background thread that forwards progress updates from a channel to a callback.
///
/// Returns a `JoinHandle` for the thread, if one was spawned.
#[must_use]
#[allow(clippy::single_option_map)]
pub fn spawn_progress_forwarder(
    on_progress: Option<&WorkflowProgressCallback>,
    rx: std::sync::mpsc::Receiver<ProgressUpdate>,
) -> Option<std::thread::JoinHandle<()>> {
    on_progress.map(|cb| {
        let cb = cb.clone();
        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                cb(&msg.step, &msg.message);
            }
        })
    })
}
