//! Centralized technical constants for the soromantic project.

/// Asset status constants (matches DB INTEGER values)
pub mod status {
    pub const NONE: i64 = 0;
    pub const PENDING: i64 = 1;
    pub const DOWNLOADING: i64 = 2;
    pub const DONE: i64 = 3;
    pub const ERROR: i64 = 4;
    /// Preview .mp4 downloaded AND JPEG frames already extracted (rompla's statusFramesCached)
    pub const FRAMES_CACHED: i64 = 5;
}

/// Media processing constants
pub mod media {
    /// FFMPEG snapshot offset (e.g., take snapshot at 5 seconds)
    pub const SNAPSHOT_OFFSET_SECS: &str = "00:00:05";

    /// FFMPEG quality setting (lower is better, 2 is high quality)
    pub const SNAPSHOT_QUALITY: &str = "2";

    /// Conversion factor for microseconds to seconds
    pub const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;

    /// Non-standard resolution found on some sites
    pub const RESOLUTION_NON_STD_576: i64 = 576;

    /// Standard resolution to map to
    pub const RESOLUTION_STD_480: i64 = 480;
}

/// Technical conversion factors
pub mod time {
    /// Conversion factor for microseconds to seconds
    pub const MICROSECONDS_PER_SECOND: f64 = 1_000_000.0;

    /// Conversion factor for milliseconds to seconds
    pub const MILLISECONDS_PER_SECOND: f64 = 1000.0;
}

/// UI and formatting constants
pub mod ui {
    /// Padding width for IDs in filenames
    pub const PAD_WIDTH: usize = 6;
}
