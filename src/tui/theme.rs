//! Semantic color scheme with switchable palettes, predefined themes, export/import, creator.

use ratatui::style::{Color, Modifier, Style};
use std::sync::RwLock;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ThemePalette {
    pub name: String,
    pub bg: [u8; 3],
    pub panel_bg: [u8; 3],
    pub border: [u8; 3],
    pub heading: [u8; 3],
    pub body: [u8; 3],
    pub dim: [u8; 3],
    pub accent: [u8; 3],
    pub success: [u8; 3],
    pub key: [u8; 3],
    pub danger: [u8; 3],
    pub neutral_pnl: [u8; 3],
}

impl ThemePalette {
    fn rgb([r, g, b]: [u8; 3]) -> Color {
        Color::Rgb(r, g, b)
    }
    pub fn role_color(&self, role: usize) -> [u8; 3] {
        match role {
            0 => self.bg,
            1 => self.panel_bg,
            2 => self.border,
            3 => self.heading,
            4 => self.body,
            5 => self.dim,
            6 => self.accent,
            7 => self.success,
            8 => self.key,
            9 => self.danger,
            10 => self.neutral_pnl,
            _ => self.body,
        }
    }
    pub fn set_role_color(&mut self, role: usize, c: [u8; 3]) {
        match role {
            0 => self.bg = c,
            1 => self.panel_bg = c,
            2 => self.border = c,
            3 => self.heading = c,
            4 => self.body = c,
            5 => self.dim = c,
            6 => self.accent = c,
            7 => self.success = c,
            8 => self.key = c,
            9 => self.danger = c,
            10 => self.neutral_pnl = c,
            _ => {}
        }
    }
    pub fn bg(&self) -> Color {
        Self::rgb(self.bg)
    }
    pub fn panel_bg(&self) -> Color {
        Self::rgb(self.panel_bg)
    }
    pub fn border(&self) -> Color {
        Self::rgb(self.border)
    }
    pub fn heading(&self) -> Color {
        Self::rgb(self.heading)
    }
    pub fn body(&self) -> Color {
        Self::rgb(self.body)
    }
    pub fn dim(&self) -> Color {
        Self::rgb(self.dim)
    }
    pub fn accent(&self) -> Color {
        Self::rgb(self.accent)
    }
    pub fn success(&self) -> Color {
        Self::rgb(self.success)
    }
    pub fn key(&self) -> Color {
        Self::rgb(self.key)
    }
    pub fn danger(&self) -> Color {
        Self::rgb(self.danger)
    }
    pub fn neutral_pnl(&self) -> Color {
        Self::rgb(self.neutral_pnl)
    }
}

fn default_dark() -> ThemePalette {
    ThemePalette {
        name: "Default Dark".into(),
        bg: [8, 8, 10],
        panel_bg: [16, 16, 18],
        border: [72, 70, 58],
        heading: [180, 172, 130],
        body: [165, 162, 155],
        dim: [100, 98, 92],
        accent: [255, 220, 100],
        success: [140, 180, 120],
        key: [170, 150, 210],
        danger: [220, 120, 100],
        neutral_pnl: [200, 180, 100],
    }
}

fn nord() -> ThemePalette {
    ThemePalette {
        name: "Nord".into(),
        bg: [46, 52, 64],
        panel_bg: [59, 66, 82],
        border: [76, 86, 106],
        heading: [143, 188, 187],
        body: [200, 205, 215],
        dim: [129, 161, 193],
        accent: [136, 192, 208],
        success: [163, 190, 140],
        key: [180, 142, 173],
        danger: [191, 97, 106],
        neutral_pnl: [235, 203, 139],
    }
}

fn dracula() -> ThemePalette {
    ThemePalette {
        name: "Dracula".into(),
        bg: [40, 42, 54],
        panel_bg: [49, 51, 68],
        border: [68, 71, 90],
        heading: [189, 147, 249],
        body: [210, 210, 200],
        dim: [98, 114, 164],
        accent: [255, 121, 198],
        success: [80, 250, 123],
        key: [255, 184, 108],
        danger: [255, 85, 85],
        neutral_pnl: [241, 250, 140],
    }
}

fn green() -> ThemePalette {
    ThemePalette {
        name: "Green Terminal".into(),
        bg: [0, 20, 0],
        panel_bg: [0, 35, 0],
        border: [40, 80, 40],
        heading: [150, 255, 150],
        body: [180, 255, 180],
        dim: [80, 140, 80],
        accent: [200, 255, 200],
        success: [100, 255, 100],
        key: [150, 255, 200],
        danger: [255, 100, 100],
        neutral_pnl: [200, 255, 150],
    }
}

fn monokai() -> ThemePalette {
    ThemePalette {
        name: "Monokai".into(),
        bg: [39, 40, 34],
        panel_bg: [49, 51, 53],
        border: [73, 72, 62],
        heading: [249, 38, 114],
        body: [215, 215, 208],
        dim: [117, 113, 94],
        accent: [102, 217, 239],
        success: [166, 226, 46],
        key: [230, 219, 116],
        danger: [249, 38, 114],
        neutral_pnl: [230, 219, 116],
    }
}

pub fn predefined_themes() -> Vec<ThemePalette> {
    vec![default_dark(), nord(), dracula(), green(), monokai()]
}

static THEME_STATE: std::sync::OnceLock<RwLock<ThemeState>> = std::sync::OnceLock::new();

#[derive(Default)]
pub struct ThemeState {
    pub current: usize,
    pub palettes: Vec<ThemePalette>,
}

fn theme_state() -> &'static RwLock<ThemeState> {
    THEME_STATE.get_or_init(|| {
        let palettes = predefined_themes();
        RwLock::new(ThemeState {
            current: 0,
            palettes,
        })
    })
}

pub fn init_themes() {
    let _ = theme_state();
}

pub fn current_palette() -> ThemePalette {
    // Init runs at startup; lock only poisoned on panic.
    let state = theme_state().read().expect("theme lock");
    let idx = state.current.min(state.palettes.len().saturating_sub(1));
    state
        .palettes
        .get(idx)
        .cloned()
        .unwrap_or_else(default_dark)
}

pub fn theme_count() -> usize {
    theme_state().read().map(|s| s.palettes.len()).unwrap_or(0)
}

pub fn theme_name_at(i: usize) -> String {
    theme_state()
        .read()
        .ok()
        .and_then(|s| s.palettes.get(i).map(|p| p.name.clone()))
        .unwrap_or_else(|| "?".into())
}

pub fn set_theme_index(i: usize) {
    if let Ok(mut state) = theme_state().write() {
        if i < state.palettes.len() {
            state.current = i;
        }
    }
}

pub fn add_custom_theme(p: ThemePalette) -> usize {
    if let Ok(mut state) = theme_state().write() {
        let idx = state.palettes.len();
        state.palettes.push(p);
        return idx;
    }
    0
}

pub fn export_current_theme(path: &std::path::Path) -> std::io::Result<()> {
    let state = theme_state()
        .read()
        .map_err(|_| std::io::ErrorKind::Other)?;
    let idx = state.current.min(state.palettes.len().saturating_sub(1));
    let p = state
        .palettes
        .get(idx)
        .ok_or(std::io::ErrorKind::NotFound)?;
    let s = toml::to_string_pretty(p).map_err(|_| std::io::ErrorKind::InvalidData)?;
    std::fs::write(path, s)
}

pub fn import_theme(path: &std::path::Path) -> anyhow::Result<usize> {
    let s = std::fs::read_to_string(path)?;
    let p: ThemePalette = toml::from_str(&s)?;
    Ok(add_custom_theme(p))
}

pub fn current_theme_index() -> usize {
    theme_state().read().map(|s| s.current).unwrap_or(0)
}

/// Preset colors for theme creator (indexable).
pub const COLOR_PRESETS: [[u8; 3]; 24] = [
    [0, 0, 0],
    [8, 8, 10],
    [46, 52, 64],
    [40, 42, 54],
    [255, 255, 255],
    [200, 198, 190],
    [248, 248, 242],
    [236, 239, 244],
    [255, 220, 100],
    [136, 192, 208],
    [189, 147, 249],
    [102, 217, 239],
    [140, 180, 120],
    [80, 250, 123],
    [166, 226, 46],
    [220, 120, 100],
    [249, 38, 114],
    [191, 97, 106],
    [180, 160, 220],
    [255, 184, 108],
    [120, 115, 100],
    [117, 113, 94],
    [72, 70, 58],
    [76, 86, 106],
];

pub const THEME_CREATOR_ROLES: &[&str] = &[
    "Background",
    "Panel BG",
    "Border",
    "Heading",
    "Body",
    "Dim",
    "Accent",
    "Success",
    "Key",
    "Danger",
    "Neutral P&L",
];

/// Theme facade: all styles read from current palette.
pub struct Theme;

impl Theme {
    pub fn block_border() -> Style {
        Style::default().fg(current_palette().border())
    }
    pub fn block_title() -> Style {
        Style::default()
            .fg(current_palette().heading())
            .add_modifier(Modifier::BOLD)
    }
    /// Metrics section labels (bold, heading color).
    pub fn metrics_label() -> Style {
        Style::default()
            .fg(current_palette().heading())
            .add_modifier(Modifier::BOLD)
    }
    pub fn tab_default() -> Style {
        Style::default().fg(current_palette().dim())
    }
    pub fn tab_selected() -> Style {
        Style::default()
            .fg(current_palette().accent())
            .add_modifier(Modifier::BOLD)
    }
    pub fn body() -> Style {
        Style::default().fg(current_palette().body())
    }
    pub fn key() -> Style {
        Style::default().fg(current_palette().key())
    }
    pub fn dim() -> Style {
        Style::default().fg(current_palette().dim())
    }
    pub fn success() -> Style {
        Style::default().fg(current_palette().success())
    }
    pub fn danger() -> Style {
        Style::default().fg(current_palette().danger())
    }
    /// Warning / yellow for log level.
    pub fn warning() -> Style {
        Style::default().fg(Color::Rgb(200, 180, 80))
    }
    pub fn neutral_pnl() -> Style {
        Style::default().fg(current_palette().neutral_pnl())
    }
    pub fn content() -> Style {
        Self::body()
    }
    /// For layout background/panel.
    #[allow(non_snake_case)]
    pub fn BG() -> Color {
        current_palette().bg()
    }
    #[allow(non_snake_case)]
    pub fn PANEL_BG() -> Color {
        current_palette().panel_bg()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_palette_serialize_roundtrip() {
        let p = predefined_themes().into_iter().next().unwrap();
        let s = toml::to_string_pretty(&p).unwrap();
        let q: ThemePalette = toml::from_str(&s).unwrap();
        assert_eq!(p.name, q.name);
        assert_eq!(p.bg, q.bg);
        assert_eq!(p.body, q.body);
    }
}
