//! Tests for XV scraper.

use soromantic_core::scraper::xv;

#[test]
fn test_parse_duration() {
    assert_eq!(xv::parse_duration("PT00H07M24S"), Some(444));
    assert_eq!(xv::parse_duration("PT1H30M00S"), Some(5400));
    assert_eq!(xv::parse_duration("PT5M"), Some(300));
    assert_eq!(xv::parse_duration("PT30S"), Some(30));
    assert_eq!(xv::parse_duration("invalid"), None);
}

#[test]
fn test_extract_html5player_urls() {
    let html = r#"
        <script>
            html5player.setVideoUrlLow('https://example.com/low.mp4');
            html5player.setVideoUrlHigh('https://example.com/high.mp4');
            html5player.setVideoHLS('https://example.com/hls.m3u8');
        </script>
    "#;

    let urls = xv::extract_html5player_urls(html);
    assert_eq!(
        urls.url_low,
        Some("https://example.com/low.mp4".to_string())
    );
    assert_eq!(
        urls.url_high,
        Some("https://example.com/high.mp4".to_string())
    );
    assert_eq!(
        urls.url_hls,
        Some("https://example.com/hls.m3u8".to_string())
    );
}

#[test]
fn test_extract_html5player_urls_partial() {
    let html = r#"
        <script>
            html5player.setVideoUrlHigh('https://example.com/high.mp4');
        </script>
    "#;

    let urls = xv::extract_html5player_urls(html);
    assert_eq!(urls.url_low, None);
    assert_eq!(
        urls.url_high,
        Some("https://example.com/high.mp4".to_string())
    );
    assert_eq!(urls.url_hls, None);
}

#[test]
fn test_parse_hls_master() {
    let m3u8 = r#"#EXTM3U
#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=854x480,NAME="480p"
hls-480p.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000000,RESOLUTION=1280x720,NAME="720p"
hls-720p.m3u8
"#;

    let streams = xv::parse_hls_master(m3u8, "https://example.com/video/hls.m3u8");

    assert_eq!(streams.len(), 2);
    // Sorted by resolution descending
    assert_eq!(streams[0].resolution, 720);
    assert_eq!(streams[0].url, "https://example.com/video/hls-720p.m3u8");
    assert_eq!(streams[1].resolution, 480);
    assert_eq!(streams[1].url, "https://example.com/video/hls-480p.m3u8");
}

#[test]
fn test_build_grid_boxes() {
    let related = serde_json::json!([
        {
            "u": "/video123/test-title",
            "tf": "Test &amp; Title",
            "il": "https://example.com/thumb.jpg",
            "ipu": "https://example.com/preview.mp4"
        },
        {
            "u": "/video456/another",
            "t": "Short Title",
            "i": "https://example.com/small.jpg"
        }
    ]);

    let arr = related.as_array().unwrap();
    let boxes = xv::build_grid_boxes(arr);

    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[0].title, "Test & Title".to_string());
    assert_eq!(
        boxes[0].url,
        "https://www.xvideos.com/video123/test-title".to_string()
    );
    assert_eq!(
        boxes[0].image,
        Some("https://example.com/thumb.jpg".to_string())
    );
    assert_eq!(
        boxes[0].preview_url,
        Some("https://example.com/preview.mp4".to_string())
    );

    assert_eq!(boxes[1].title, "Short Title".to_string());
    assert_eq!(
        boxes[1].image,
        Some("https://example.com/small.jpg".to_string())
    );
    assert_eq!(boxes[1].preview_url, None);
}

#[test]
fn test_build_video_sources() {
    let urls = xv::Html5PlayerUrls {
        url_low: Some("https://example.com/low.mp4".to_string()),
        url_high: Some("https://example.com/high.mp4".to_string()),
        url_hls: Some("https://example.com/hls.m3u8".to_string()),
    };

    let sources = xv::build_video_sources(&urls);

    assert_eq!(sources.len(), 3);
    assert_eq!(sources[0].resolution, 720);
    assert_eq!(sources[1].resolution, 480);
    assert_eq!(sources[2].resolution, 360);
}
