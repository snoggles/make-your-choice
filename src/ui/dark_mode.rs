pub enum Theme {
    Light,
    Dark,
}

pub fn detect_theme() -> Theme {
    match dark_light::detect() {
        dark_light::Mode::Dark => Theme::Dark,
        dark_light::Mode::Light => Theme::Light,
        dark_light::Mode::Default => Theme::Light,
    }
}
