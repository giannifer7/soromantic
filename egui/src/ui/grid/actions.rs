use std::path::PathBuf;

pub enum GridAction {
    ToggleSelection(i64, usize),
    SelectRange(usize, usize),
    Play(i64),
    Focus(usize),
    RequestPreview(i64, String, PathBuf),
    OpenPage(i64),
}
