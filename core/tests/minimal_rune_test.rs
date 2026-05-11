//! Minimal test to isolate Rune String method issues.

use anyhow::Result;
use rune::diagnostics::Diagnostics;
use rune::runtime::Vm;
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Source, Sources};
use soromantic_core::scripting;
use std::sync::Arc;

/// Test minimal model.rn scrape_page function.
#[test]
fn test_minimal_scrape() -> Result<()> {
    // Minimal script that just parses HTML
    let script = r#"
pub fn scrape_page(html) {
    let doc = html::parse(html);
    let title = html::query(doc, "h1")?;
    if title.is_some() {
        Ok(#{ found: true })
    } else {
        Ok(#{ found: false })
    }
}
"#;

    let html = "<html><body><h1>Test</h1></body></html>";

    let mut sources = Sources::new();
    let _ = sources.insert(Source::new("script", script)?);

    let context = scripting::create_context()?;
    let runtime = Arc::new(context.runtime()?);
    let mut diagnostics = Diagnostics::new();

    let result = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();

    if !diagnostics.is_empty() {
        let mut writer = StandardStream::stderr(ColorChoice::Always);
        diagnostics.emit(&mut writer, &sources)?;
    }

    let unit = result?;
    let mut vm = Vm::new(runtime, Arc::new(unit));

    let output = vm.call(["scrape_page"], (html.to_string(),))?;
    println!("Output: {:?}", output);

    Ok(())
}

/// Test util::trim function.
#[test]
fn test_util_trim() -> Result<()> {
    let script = r#"
pub fn test_trim() {
    let result = util::trim("  hello  ");
    result
}
"#;

    let mut sources = Sources::new();
    let _ = sources.insert(Source::new("script", script)?);

    let context = scripting::create_context()?;
    let runtime = Arc::new(context.runtime()?);
    let mut diagnostics = Diagnostics::new();

    let result = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();

    if !diagnostics.is_empty() {
        let mut writer = StandardStream::stderr(ColorChoice::Always);
        diagnostics.emit(&mut writer, &sources)?;
    }

    let unit = result?;
    let mut vm = Vm::new(runtime, Arc::new(unit));

    let output = vm.call(["test_trim"], ())?;
    println!("Trim output: {:?}", output);

    Ok(())
}
