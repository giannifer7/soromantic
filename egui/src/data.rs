use eframe::egui::{ColorImage, TextureHandle};

pub struct LoadedImage {
    pub texture: TextureHandle,
}

#[derive(Clone)]
pub struct PendingImage {
    pub id: i64,
    pub image: ColorImage,
    pub is_preview: bool, // Allow distinguishing
}

#[derive(Default)]
pub struct PreviewFrames {
    pub frames: Vec<TextureHandle>,
    pub ready: bool,
}
