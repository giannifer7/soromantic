use rune::Module;

/// Create the http module for Rune scripts.
///
/// # Errors
///
/// Returns an error if the module cannot be created.
pub fn module() -> anyhow::Result<Module, rune::ContextError> {
    let mut m = Module::with_item(["http"])?;
    m.function_meta(fetch)?;
    Ok(m)
}

/// Fetch a URL and return the text content.
#[rune::function]
fn fetch(url: &str) -> anyhow::Result<String> {
    // Use a blocking call since rune 0.14.1 doesn't directly support async in #[rune::function]
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(crate::scraper::fetch_page_text(url))
    })
}
