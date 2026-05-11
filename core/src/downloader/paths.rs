//! Download path generation utilities.
//!
//! Port of Python `download/paths.py` - provides consistent path generation
//! for all downloaded file types.

use std::path::PathBuf;
use url::Url;

/// Extract file extension from a URL.
///
/// Returns the lowercase extension if found.
#[must_use]
pub fn get_file_extension(url: &str) -> Option<String> {
    Url::parse(url).ok().and_then(|u| {
        u.path()
            .rsplit('.')
            .next()
            .filter(|ext| !ext.is_empty() && ext.len() <= 5)
            .map(str::to_lowercase)
    })
}

/// File type for download path generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    /// Cover image (stored in covers/)
    Cover,
    /// Thumbnail image (stored in thumbs/)
    Thumb,
    /// Preview video (stored in previews/)
    Preview,
    /// Main video (stored in videos/<resolution>/)
    Video,
}

use crate::config::ResolvedConfig;

/// Generate organized download path for a file.
///
/// # Arguments
/// * `config` - Resolved configuration with paths
/// * `page_id` - Page ID for filename
/// * `url` - Source URL (for extension extraction)
/// * `file_type` - Type of file being downloaded
/// * `resolution` - Video resolution (only used for Video type)
pub fn get_download_path(
    config: &ResolvedConfig,
    page_id: i64,
    url: Option<&str>,
    file_type: FileType,
    resolution: Option<i64>,
) -> PathBuf {
    let ext = url
        .and_then(get_file_extension)
        .unwrap_or_else(|| match file_type {
            FileType::Thumb | FileType::Cover => "jpg".to_string(),
            FileType::Video | FileType::Preview => "mp4".to_string(),
        });

    // Force .jpg for images and .mp4 for video types
    let ext = match file_type {
        FileType::Thumb | FileType::Cover => "jpg".to_string(),
        FileType::Video | FileType::Preview => {
            if ext == "m3u8" || ext == "mp4" || ext == "webm" {
                "mp4".to_string()
            } else {
                ext
            }
        }
    };

    let filename = format!(
        "{page_id:0width$}.{ext}",
        width = crate::constants::ui::PAD_WIDTH
    );

    match file_type {
        FileType::Cover => config.covers_dir.join(&filename),
        FileType::Thumb => config.thumbs_dir.join(&filename),
        FileType::Preview => config.previews_dir.join(&filename),
        FileType::Video => {
            let res_str = resolution.map_or_else(|| "unknown".to_string(), |r| r.to_string());
            config.videos_dir.join(res_str).join(&filename)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Helper to create a test config
    fn test_config() -> ResolvedConfig {
        let root = PathBuf::from("/data");
        ResolvedConfig {
            config_file_path: None,
            db_path: root.join("test.db"),
            download_dir: root.clone(),
            cache_dir: root.join("cache"),
            frontend_dir: root.join("frontend"),
            mpv_socket: root.join("socket"),
            batch_list_path: root.join("batch.txt"),
            ffmpeg_path: PathBuf::from("ffmpeg"),
            ffprobe_path: PathBuf::from("ffprobe"),
            timeouts: Default::default(),
            ui: Default::default(),
            playback: Default::default(),
            thumbs_dir: root.join("thumbs"),
            covers_dir: root.join("covers"),
            videos_dir: root.join("videos"),
            previews_dir: root.join("previews"),
            models_dir: root.join("models"),
            flags_dir: root.join("flags"),
            scrapers_dir: root.join("scrapers"),
            download_delay_ms: 0,
        }
    }

    #[test]
    fn test_get_file_extension_mp4() {
        assert_eq!(
            get_file_extension("https://example.com/video.mp4"),
            Some("mp4".to_string())
        );
    }

    #[test]
    fn test_get_file_extension_jpg() {
        assert_eq!(
            get_file_extension("https://example.com/thumb.jpg?size=large"),
            Some("jpg".to_string())
        );
    }

    #[test]
    fn test_get_file_extension_no_ext() {
        assert_eq!(get_file_extension("https://example.com/video"), None);
    }

    #[test]
    fn test_get_file_extension_m3u8() {
        assert_eq!(
            get_file_extension("https://example.com/stream.m3u8"),
            Some("m3u8".to_string())
        );
    }

    #[test]
    fn test_get_download_path_cover() {
        let config = test_config();
        let path = get_download_path(
            &config,
            42,
            Some("https://x.com/img.jpg"),
            FileType::Cover,
            None,
        );
        assert_eq!(path, PathBuf::from("/data/covers/000042.jpg"));
    }

    #[test]
    fn test_get_download_path_thumb() {
        let config = test_config();
        let path = get_download_path(
            &config,
            123,
            Some("https://x.com/t.jpg"),
            FileType::Thumb,
            None,
        );
        assert_eq!(path, PathBuf::from("/data/thumbs/000123.jpg"));
    }

    #[test]
    fn test_get_download_path_preview() {
        let config = test_config();
        let path = get_download_path(
            &config,
            5,
            Some("https://x.com/p.mp4"),
            FileType::Preview,
            None,
        );
        assert_eq!(path, PathBuf::from("/data/previews/000005.mp4"));
    }

    #[test]
    fn test_get_download_path_video_480p() {
        let config = test_config();
        let path = get_download_path(
            &config,
            999,
            Some("https://x.com/v.mp4"),
            FileType::Video,
            Some(480),
        );
        assert_eq!(path, PathBuf::from("/data/videos/480/000999.mp4"));
    }

    #[test]
    fn test_get_download_path_video_no_resolution() {
        let config = test_config();
        let path = get_download_path(
            &config,
            1,
            Some("https://x.com/v.mp4"),
            FileType::Video,
            None,
        );
        assert_eq!(path, PathBuf::from("/data/videos/unknown/000001.mp4"));
    }
}
