pub struct Config {
    pub columns: usize,
    pub icon_size: u32,
    pub background_opacity: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            columns: 6,
            icon_size: 96,
            background_opacity: 0.85,
        }
    }
}
