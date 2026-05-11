use super::MediaProbe;
use anyhow::Result;
use async_trait::async_trait;

pub struct DesktopMediaProbe;

impl DesktopMediaProbe {
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MediaProbe for DesktopMediaProbe {
    async fn get_duration(&self, path: &str) -> Result<f64> {
        // Use ffprobe from PATH
        crate::downloader::ffmpeg::probe_duration(std::path::Path::new("ffprobe"), path).await
    }

    async fn extract_thumbnail(&self, video_path: &str, dest_path: &str) -> Result<()> {
        crate::downloader::ffmpeg::extract_snapshot(
            std::path::Path::new("ffmpeg"),
            std::path::Path::new(video_path),
            std::path::Path::new(dest_path),
        )
        .await
    }
}
