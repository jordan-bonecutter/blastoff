use serde::Deserialize;
use std::{env, fs, path::PathBuf};

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub width: f32,
    pub max_results: usize,
    pub font_size: f32,
    pub radius: f32,
    pub background: String,
    pub foreground: String,
    pub accent: String,
    pub icon_theme: String,
    pub terminal: Vec<String>,
    pub close_on_unfocus: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            width: 680.0,
            max_results: 7,
            font_size: 17.0,
            radius: 20.0,
            background: "#181b24".into(),
            foreground: "#eef0f7".into(),
            accent: "#a5b4fc".into(),
            icon_theme: "Adwaita".into(),
            terminal: vec!["foot".into(), "-e".into()],
            close_on_unfocus: true,
        }
    }
}
pub fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}
pub fn path() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("blastoff/config.toml")
}
impl Config {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let config: Self = match fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.to_string()),
        };
        if !(400.0..=1400.0).contains(&config.width)
            || !(1..=12).contains(&config.max_results)
            || !(12.0..=28.0).contains(&config.font_size)
            || !(0.0..=40.0).contains(&config.radius)
        {
            return Err(
                "width must be 400–1400, max_results 1–12, font_size 12–28, radius 0–40".into(),
            );
        }
        for value in [&config.background, &config.foreground, &config.accent] {
            if parse_color(value).is_none() {
                return Err(format!("invalid color {value:?}; use #RRGGBB"));
            }
        }
        if config.terminal.first().is_none_or(|s| s.is_empty()) {
            return Err("terminal must contain an executable".into());
        }
        Ok(config)
    }
}
pub fn parse_color(s: &str) -> Option<iced::Color> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(iced::Color::from_rgb8(
        (n >> 16) as u8,
        (n >> 8) as u8,
        n as u8,
    ))
}
