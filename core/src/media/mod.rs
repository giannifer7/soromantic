use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait MediaProbe: Send + Sync {
    async fn get_duration(&self, path: &str) -> Result<f64>;
    async fn extract_thumbnail(&self, video_path: &str, dest_path: &str) -> Result<()>;
}

// Factory or provider
#[must_use]
pub fn get_probe() -> Box<dyn MediaProbe> {
    Box::new(desktop::DesktopMediaProbe::new())
}

mod desktop;
