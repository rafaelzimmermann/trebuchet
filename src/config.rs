pub struct Config {
    pub columns: usize,
    pub rows: usize,
    pub icon_size: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            columns: 7,
            rows: 5,
            icon_size: 96,
        }
    }
}
