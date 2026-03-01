pub struct Config {
    pub columns: usize,
    pub icon_size: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            columns: 6,
            icon_size: 96,
        }
    }
}
