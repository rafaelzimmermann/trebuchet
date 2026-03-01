pub struct Config {
    pub columns: usize,
    pub rows: usize,
    pub icon_size: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            columns: 6,
            rows: 4,
            icon_size: 96,
        }
    }
}
