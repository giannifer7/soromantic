use rune::Any;
use rune::Module;
use scraper::{Html, Selector};
use std::collections::HashMap;

#[derive(Any, Debug, Clone)]
struct Document {
    html: String,
}

#[derive(Any, Debug, Clone)]
struct Element {
    #[rune(get)]
    pub tag: String,
    #[rune(get)]
    pub text: String,
    #[rune(get)]
    pub inner: String,
    #[rune(get)]
    pub outer: String,
    pub attrs: HashMap<String, String>,
}

impl Element {
    fn from_ref(el: scraper::ElementRef) -> Self {
        let tag = el.value().name().to_string();
        let text = el.text().collect::<Vec<_>>().join("");
        let inner = el.inner_html();
        let outer = el.html();
        let attrs = el
            .value()
            .attrs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        Self {
            tag,
            text,
            inner,
            outer,
            attrs,
        }
    }

    /// Get an attribute value by name.
    #[rune::function(instance)]
    fn attr(&self, name: &str) -> Option<String> {
        self.attrs.get(name).cloned()
    }

    /// Query for all matching elements within this element.
    #[rune::function(instance)]
    fn query_all(&self, selector: &str) -> anyhow::Result<Vec<Self>> {
        let fragment = Html::parse_fragment(&self.outer);
        let sel =
            Selector::parse(selector).map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
        Ok(fragment.select(&sel).map(Self::from_ref).collect())
    }

    /// Query for first matching element within this element.
    #[rune::function(instance)]
    fn query(&self, selector: &str) -> anyhow::Result<Option<Self>> {
        let fragment = Html::parse_fragment(&self.outer);
        let sel =
            Selector::parse(selector).map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;
        Ok(fragment.select(&sel).next().map(Self::from_ref))
    }
}

/// Create the html module for Rune scripts.
///
/// # Errors
///
/// Returns an error if the module or types cannot be registered.
pub fn module() -> anyhow::Result<Module, rune::ContextError> {
    let mut m = Module::with_item(["html"])?;
    m.ty::<Document>()?;
    m.ty::<Element>()?;
    m.function_meta(Element::attr)?;
    m.function_meta(Element::query)?;
    m.function_meta(Element::query_all)?;
    m.function_meta(parse)?;
    m.function_meta(query_all)?;
    m.function_meta(query)?;
    m.function_meta(debug_element)?;
    Ok(m)
}

#[rune::function]
fn parse(html: &str) -> Document {
    Document {
        html: html.to_string(),
    }
}

#[rune::function]
fn query_all(doc: &Document, selector: &str) -> anyhow::Result<Vec<Element>> {
    let fragment = Html::parse_document(&doc.html);
    let sel = Selector::parse(selector).map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    Ok(fragment.select(&sel).map(Element::from_ref).collect())
}

#[rune::function]
fn query(doc: &Document, selector: &str) -> anyhow::Result<Option<Element>> {
    let fragment = Html::parse_document(&doc.html);
    let sel = Selector::parse(selector).map_err(|e| anyhow::anyhow!("Invalid selector: {e:?}"))?;

    Ok(fragment.select(&sel).next().map(Element::from_ref))
}

#[rune::function]
fn debug_element(el: &Element) {
    println!(
        "Element <{}> textlen={} attrs={:?}",
        el.tag,
        el.text.len(),
        el.attrs
    );
}
