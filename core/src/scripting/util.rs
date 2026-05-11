use chrono::{Datelike, Utc};
use rune::Module;
use url::Url;

/// Create the util module for Rune scripts.
///
/// # Errors
///
/// Returns an error if the module cannot be created.
pub fn module() -> anyhow::Result<Module, rune::ContextError> {
    let mut m = Module::with_item(["util"])?;
    m.function_meta(log)?;
    m.function_meta(clean_url)?;
    m.function_meta(resolve_url)?;
    m.function_meta(norm_url)?;
    m.function_meta(current_year)?;
    m.function_meta(extract_flag_code)?;
    m.function_meta(age_to_birth_year)?;
    m.function_meta(trim)?;
    Ok(m)
}

/// Get the current year (UTC).
#[rune::function]
fn current_year() -> i64 {
    i64::from(Utc::now().year())
}

#[rune::function]
fn log(msg: &str) {
    tracing::info!("[Rune] {msg}");
}

#[rune::function]
fn clean_url(url: &str) -> String {
    Url::parse(url).map_or_else(
        |_| url.to_string(),
        |mut u| {
            u.set_query(None);
            u.to_string()
        },
    )
}

use crate::utils::{norm_url as norm_url_impl, resolve_url as resolve_url_impl};

/// Resolve a potentially relative URL against a base URL.
#[rune::function]
fn resolve_url(base: &str, url: &str) -> String {
    resolve_url_impl(base, url)
}

/// Normalize URL to absolute form (handling // and / prefixes).
#[rune::function]
fn norm_url(base: &str, href: &str) -> String {
    norm_url_impl(base, href)
}

/// Extract flag code from icon path (e.g., "/assets/img/flags/us.png" -> "us").
#[rune::function]
fn extract_flag_code(icon_path: &str) -> Option<String> {
    // Find last slash
    let slash_idx = icon_path.rfind('/')?;
    let filename = &icon_path[slash_idx + 1..];

    // Remove extension
    filename.rfind('.').map_or_else(
        || Some(filename.to_string()),
        |dot_idx| Some(filename[..dot_idx].to_string()),
    )
}

/// Convert age string to birth year.
#[rune::function]
fn age_to_birth_year(age_str: &str) -> Option<i64> {
    let age: i64 = age_str.trim().parse().ok()?;
    if age > 0 && age < 150 {
        Some(i64::from(Utc::now().year()) - age)
    } else {
        None
    }
}

/// Trim whitespace from a string.
#[rune::function]
fn trim(s: &str) -> String {
    s.trim().to_string()
}
