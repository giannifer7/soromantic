use soromantic_core::types::VideoEntry;

#[test]
fn test_video_entry_default() {
    let entry: VideoEntry<i64> = VideoEntry::default();
    assert_eq!(entry.id, 0);
    assert_eq!(entry.title, "");
    assert_eq!(entry.url, "");
    assert_eq!(entry.image, None);
    assert_eq!(entry.local_image, None);
    assert_eq!(entry.preview_url, None);
    assert_eq!(entry.local_preview, None);
    assert_eq!(entry.finished_videos, 0);
    assert_eq!(entry.failed_videos, 0);
}

#[test]
fn test_video_entry_new() {
    let entry = VideoEntry {
        id: 10,
        title: "Test Video".to_string(),
        url: "http://example.com".to_string(),
        image: Some("http://example.com/img.jpg".to_string()),
        ..Default::default()
    };

    assert_eq!(entry.id, 10);
    assert_eq!(entry.title, "Test Video");
    assert_eq!(entry.url, "http://example.com");
    assert_eq!(entry.image, Some("http://example.com/img.jpg".to_string()));
}
