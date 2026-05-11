use url::Url;

/// Resolve a potentially relative URL against a base URL.
#[must_use]
pub fn resolve_url(base: &str, url: &str) -> String {
    Url::parse(base).map_or_else(
        |_| url.to_string(),
        |base_url| {
            base_url
                .join(url)
                .map_or_else(|_| url.to_string(), |u| u.to_string())
        },
    )
}

/// Normalize URL to absolute form.
#[must_use]
pub fn norm_url(base: &str, href: &str) -> String {
    if href.is_empty() {
        return String::new();
    }
    if href.starts_with("//") {
        return format!("http:{href}");
    }
    if href.starts_with("http") {
        return href.to_string();
    }

    resolve_url(base, href)
}
