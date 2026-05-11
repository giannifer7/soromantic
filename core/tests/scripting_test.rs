use anyhow::Result;
use rune::termcolor::{ColorChoice, StandardStream};
use rune::{Diagnostics, Source, Sources, Vm};
use soromantic_core::scripting;
use std::sync::Arc;

#[tokio::test]
async fn test_run_pv_scraper() -> Result<()> {
    // 1. Load the script
    let script_path = std::path::Path::new("../assets/scrapers/pv.rn");
    let script_source = std::fs::read_to_string(script_path)?;

    // 2. Prepare Context + VM
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
    let _vm = Vm::new(runtime, Arc::new(unit));

    // Let's just verify the script compiles for now, to check syntax.
    Ok(())
}

#[tokio::test]
async fn test_run_pv_model_scraper() -> Result<()> {
    // 1. Load the script
    let script_path = std::path::Path::new("../assets/scrapers/pv_model.rn");
    let script_source = std::fs::read_to_string(script_path)?;

    // 2. Prepare Context + VM
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

    // 3. Load fixture
    // Note: The path depends on where the test is run from.
    // In soromantic-core, the fixtures are in tests/fixtures
    let html_path = std::path::Path::new("tests/fixtures/model_page.html");
    if html_path.exists() {
        let html_fixture = std::fs::read_to_string(html_path)?;
        let page_url = "https://pissvids.com/model/anna-de-ville";
        let is_first_page = true;

        let output = vm.call(
            ["test_parse"],
            (html_fixture, page_url.to_string(), is_first_page),
        )?;

        let val: rune::Value = output;
        println!("Scrape result: {:?}", val);
    }

    Ok(())
}
