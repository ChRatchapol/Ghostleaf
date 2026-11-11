use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Language {
    #[default]
    Auto,
    English,
    Thai,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum Theme {
    #[default]
    Light,
    /// Warm off-white.
    Paper,
    Sepia,
    /// Deeper sepia, like old paperback stock.
    Tan,
    /// Dark sepia for night reading.
    Dusk,
    Dark,
    Black,
    /// Any colour at any strength (see [`CustomTheme`]).
    Custom,
}

impl Theme {
    pub const ALL: [Theme; 8] = [
        Theme::Light,
        Theme::Paper,
        Theme::Sepia,
        Theme::Tan,
        Theme::Dusk,
        Theme::Dark,
        Theme::Black,
        Theme::Custom,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

/// User-defined tint: the paper colour is white mixed with `tint` by `strength` (0..1).
/// At 100 % the page is the tint itself; ink and UI colours are derived for contrast.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct CustomTheme {
    pub tint: [u8; 3],
    pub strength: f32,
}

impl Default for CustomTheme {
    fn default() -> Self {
        Self {
            tint: [0xB0, 0x8A, 0x5A],
            strength: 0.35,
        }
    }
}

/// How the operating-system title bar looks.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TitleBar {
    /// Leave it as the OS draws it.
    System,
    /// Colour it to match the reading theme (Windows 11; elsewhere same as System).
    #[default]
    Theme,
    /// No title bar: Ghostleaf draws its own minimal window controls.
    Hidden,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TurnStyle {
    #[default]
    Curl,
    Slide,
    None,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayMode {
    Single,
    Spread,
    #[default]
    Auto,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StealthStyle {
    /// Cover the window with a plain surface in the theme colour.
    #[default]
    Veil,
    /// Show a neutral, spreadsheet-like decoy.
    Decoy,
    /// Minimize the window.
    Minimize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Stealth {
    pub enabled: bool,
    /// Also hide when the window loses keyboard focus.
    pub on_focus_loss: bool,
    pub style: StealthStyle,
    /// Delay before hiding, in milliseconds (0 = instant).
    pub hide_delay_ms: u32,
    /// Require a click to reveal instead of simply moving the mouse back in.
    pub reveal_on_click: bool,
    /// Global boss key (Ctrl+Alt+H) toggles hiding.
    pub boss_key: bool,
    /// Use a neutral window title instead of the book title.
    pub neutral_title: bool,
}

impl Default for Stealth {
    fn default() -> Self {
        Self {
            enabled: true,
            on_focus_loss: false,
            style: StealthStyle::Veil,
            hide_delay_ms: 0,
            reveal_on_click: false,
            boss_key: true,
            neutral_title: true,
        }
    }
}

/// Typography for reflowable formats (TXT, CSV).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TextStyle {
    /// Font family name. Bundled: "Literata", "Inter", "Noto Serif Thai", "Noto Sans Thai".
    pub font: String,
    /// Body size in points.
    pub size: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
    /// Extra space between paragraphs, in multiples of the font size.
    pub paragraph_spacing: f32,
    /// Page margin in points.
    pub margin: f32,
    pub justify: bool,
    pub indent: bool,
    /// Join hard-wrapped lines (common in plain-text e-books) into paragraphs.
    pub join_wrapped_lines: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: "Literata".into(),
            size: 17.0,
            line_height: 1.55,
            paragraph_spacing: 0.6,
            margin: 36.0,
            justify: true,
            indent: false,
            join_wrapped_lines: true,
        }
    }
}

impl TextStyle {
    /// Stable fingerprint used to invalidate layout caches.
    pub fn fingerprint(&self) -> u64 {
        let s = serde_json::to_string(self).unwrap_or_default();
        let h = blake3::hash(s.as_bytes());
        u64::from_le_bytes(h.as_bytes()[..8].try_into().unwrap())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub language: Language,
    pub theme: Theme,
    pub custom_theme: CustomTheme,
    pub title_bar: TitleBar,
    pub turn_style: TurnStyle,
    pub sound: bool,
    pub volume: f32,
    pub display: DisplayMode,
    pub stealth: Stealth,
    pub text: TextStyle,
    /// Invert PDF page colours in dark themes.
    pub pdf_dark_invert: bool,
    pub always_on_top: bool,
    /// Keep small cover thumbnails for the library (PDF only). Off = nothing but metadata is stored.
    pub save_covers: bool,
    /// Reading direction for new books.
    pub default_rtl: bool,
    /// Show the "first blank page" (flyleaf) for new books.
    pub default_first_blank: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::Auto,
            theme: Theme::Light,
            custom_theme: CustomTheme::default(),
            title_bar: TitleBar::Theme,
            turn_style: TurnStyle::Curl,
            sound: true,
            volume: 0.6,
            display: DisplayMode::Auto,
            stealth: Stealth::default(),
            text: TextStyle::default(),
            pdf_dark_invert: false,
            always_on_top: false,
            save_covers: true,
            default_rtl: false,
            default_first_blank: false,
        }
    }
}
