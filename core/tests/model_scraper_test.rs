//! Tests for the Rune model page scraper.

use anyhow::Result;
use rune::diagnostics::Diagnostics;
use rune::runtime::Vm;
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Source, Sources};
use soromantic_core::scripting;
use std::sync::Arc;

/// Test that the model.rn script can parse the test fixture.
#[tokio::test]
async fn test_model_scraper_parse() -> Result<()> {
    // Load the script
    let script_path = std::path::Path::new("../assets/scrapers/pv_model.rn");
    let script_source = std::fs::read_to_string(script_path)?;

    // Load the HTML fixture
    let html_fixture = std::fs::read_to_string("tests/fixtures/model_page.html")?;

    // Prepare Context + VM
    let mut sources = Sources::new();
    let _ = sources.insert(Source::new("script", &script_source)?);

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

    // Call scrape_page function with fixture HTML
    let page_url = "https://pissvids.com/model/anna-de-ville";
    let is_first_page = true;

    let output = vm.call(
        ["test_parse"],
        (html_fixture.clone(), page_url.to_string(), is_first_page),
    )?;

    // Get the result - it should be Ok(map)
    let result_value: rune::Value = output;

    // Just print the result - we're mainly testing that the script runs
    println!("Scrape result type: {:?}", result_value.type_info());

    Ok(())
}

/// Test that the model.rn script compiles without errors.
#[test]
fn test_model_scraper_compiles() -> Result<()> {
    // Load the script
    let script_path = std::path::Path::new("../assets/scrapers/pv_model.rn");
    let script_source = std::fs::read_to_string(script_path)?;

    // Prepare Context + VM
    let mut sources = Sources::new();
    let _ = sources.insert(Source::new("script", &script_source)?);

    let context = scripting::create_context()?;
    let mut diagnostics = Diagnostics::new();

    let result = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build();

    // Print any warnings (not errors)
    if !diagnostics.is_empty() {
        let mut writer = StandardStream::stderr(ColorChoice::Always);
        diagnostics.emit(&mut writer, &sources)?;
    }

    // Should compile successfully
    let _unit = result?;

    println!("model.rn compiled successfully");

    Ok(())
}
