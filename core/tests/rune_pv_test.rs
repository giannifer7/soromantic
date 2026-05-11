use rune::{from_value, runtime::Object};
use soromantic_core::scripting;

#[test]
fn test_pv_rune_scraper() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Test Video Title - Pissvids.com</title>
        </head>
        <body>
            <div class="watch__title">
                <a href="/models/model1">Model One</a>
                <a href="/models/model2">Model Two</a>
            </div>
            <div class="genres-list">
                <a href="/studio/test-studio">Test Studio</a>
            </div>
            <video data-poster="https://example.com/poster.jpg">
                <source src="https://example.com/480p.mp4" size="480">
                <source src="https://example.com/720p.mp4" size="720">
            </video>
            <div class="card-scene">
                <div class="card-scene__view">
                    <a href="/video/related1" data-preview="https://example.com/preview1.mp4">
                        <img data-src="https://example.com/thumb1.jpg">
                    </a>
                </div>
                <div class="card-scene__text">
                    <a href="/video/related1">Related Video 1</a>
                </div>
            </div>
        </body>
        </html>
    "#;

    // Load the script
    let script_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("assets/scrapers/pv.rn");

    let source = std::fs::read_to_string(&script_path).expect("Failed to read pv.rn");

    // Initialize Rune
    let context = scripting::create_context().expect("Failed to create context");
    let mut vm = scripting::compile_and_init_vm(&context, &source).expect("Failed to compile VM");

    // Call scrape_html
    let url = "https://pissvids.com/video/test".to_string();
    let args = (url.clone(), html.to_string());

    let result = vm
        .call(["scrape_html"], args)
        .expect("Failed to call scrape_html");

    // Unwrap the Result from Rune
    let value = match from_value::<std::result::Result<rune::runtime::Value, rune::runtime::Value>>(
        result.clone(),
    ) {
        Ok(res) => res.expect("Script returned Err"),
        Err(_) => result, // Not a Result, maybe direct value (though scrape returning Ok implies Result)
    };

    // Check result
    let obj: Object = from_value(value).expect("Failed to convert result to Object");

    // Check title
    let title: String = from_value(
        obj.get_value::<_, rune::runtime::Value>("title")
            .into_result()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(title, "Test Video Title");

    // Check grid_boxes
    let grid_boxes_val = obj
        .get_value::<_, rune::runtime::Value>("grid_boxes")
        .into_result()
        .unwrap()
        .unwrap();
    let grid_boxes: Vec<rune::runtime::Value> = from_value(grid_boxes_val).unwrap();

    assert_eq!(grid_boxes.len(), 1, "Should assume 1 related video found");

    let box0: Object = from_value(grid_boxes[0].clone()).unwrap();
    let b_title: String = from_value(
        box0.get_value::<_, rune::runtime::Value>("title")
            .into_result()
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(b_title, "Related Video 1");
}
