//! Tests for PV scraper.

use soromantic_core::scraper::pv;

#[test]
fn test_parse_page_pv_basic() {
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

    let result = pv::parse_page_pv(html, "https://pissvids.com/video/test");
    assert!(result.is_ok());

    let page = result.unwrap();

    // Title should have trailer stripped
    assert_eq!(page.title, Some("Test Video Title".to_string()));

    // Models extracted
    assert_eq!(page.models.len(), 2);
    assert_eq!(page.models[0].0, "Model One");
    assert_eq!(page.models[0].1, "/models/model1");

    // Studio extracted (name, url)
    assert!(page.studio.is_some());
    let (studio_name, studio_url) = page.studio.unwrap();
    assert_eq!(studio_name, "Test Studio");
    assert_eq!(studio_url, "/studio/test-studio");

    // Video sources extracted and sorted by resolution
    assert_eq!(page.video_sources.len(), 2);
    assert_eq!(page.video_sources[0].resolution, 480);
    assert_eq!(page.video_sources[1].resolution, 720);

    // Image poster
    assert_eq!(
        page.image,
        Some("https://example.com/poster.jpg".to_string())
    );

    // Grid boxes extracted
    assert_eq!(page.grid_boxes.len(), 1);
    assert_eq!(page.grid_boxes[0].title, "Related Video 1".to_string());
}

#[test]
fn test_parse_page_pv_custom_trailer() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Another Video - Analvids.com</title>
        </head>
        <body></body>
        </html>
    "#;

    let result =
        pv::parse_page_pv_with_trailer(html, "https://analvids.com/video/test", " - Analvids.com");
    assert!(result.is_ok());

    let page = result.unwrap();
    assert_eq!(page.title, Some("Another Video".to_string()));
}

#[test]
fn test_parse_page_pv_576p_mapped_to_480p() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test - Pissvids.com</title></head>
        <body>
            <video>
                <source src="https://example.com/576p.mp4" size="576">
            </video>
        </body>
        </html>
    "#;

    let result = pv::parse_page_pv(html, "https://pissvids.com/video/test");
    assert!(result.is_ok());

    let page = result.unwrap();
    // 576p should be mapped to 480p
    assert_eq!(page.video_sources.len(), 1);
    assert_eq!(page.video_sources[0].resolution, 480);
}

#[test]
fn test_parse_page_pv_html_entities() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test &amp; Title - Pissvids.com</title></head>
        <body>
            <div class="card-scene">
                <div class="card-scene__text">
                    <a href="/video/test">Video &amp; More</a>
                </div>
            </div>
        </body>
        </html>
    "#;

    let result = pv::parse_page_pv(html, "https://pissvids.com/video/test");
    assert!(result.is_ok());

    let page = result.unwrap();
    assert_eq!(page.title, Some("Test & Title".to_string()));
    assert_eq!(page.grid_boxes[0].title, "Video & More".to_string());
}

#[test]
fn test_parse_page_pv_empty() {
    let html = "<!DOCTYPE html><html><head></head><body></body></html>";

    let result = pv::parse_page_pv(html, "https://pissvids.com/video/test");
    assert!(result.is_ok());

    let page = result.unwrap();
    assert_eq!(page.title, None);
    assert_eq!(page.models.len(), 0);
    assert_eq!(page.studio, None);
    assert_eq!(page.video_sources.len(), 0);
    assert_eq!(page.grid_boxes.len(), 0);
}
