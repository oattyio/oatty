use ratatui::style::Color;

use super::{
    Ansi256Theme, Ansi256ThemeHighContrast, CyberpunkTheme, CyberpunkThemeHighContrast, DraculaTheme, DraculaThemeHighContrast, NordTheme,
    NordThemeHighContrast, Theme,
};

/// Describes a selectable theme inside the TUI.
#[derive(Clone, Copy, Debug)]
pub struct ThemeDefinition {
    /// Canonical identifier used for persistence.
    pub id: &'static str,
    /// Human-friendly display name.
    pub label: &'static str,
    /// Short description rendered in the picker.
    #[allow(dead_code)]
    pub description: &'static str,
    /// Hex-style color chips shown inside the picker.
    pub swatch: ThemeSwatch,
    /// Theme aliases (e.g., env overrides) that map back to this definition.
    pub aliases: &'static [&'static str],
    /// Indicates whether the definition represents a high-contrast variant.
    pub is_high_contrast: bool,
    /// Whether the palette targets ANSI/8-bit terminals.
    pub is_ansi_fallback: bool,
    factory: fn() -> Box<dyn Theme>,
}

impl ThemeDefinition {
    /// Instantiate the theme represented by this definition.
    pub fn build(&self) -> Box<dyn Theme> {
        (self.factory)()
    }
}

/// Minimal set of colors that summarize each palette inside the picker.
#[derive(Clone, Copy, Debug)]
pub struct ThemeSwatch {
    pub background: Color,
    pub accent: Color,
    pub selection: Color,
}

/// Ordered list of selectable themes surfaced inside the picker and loaders.
pub const THEME_DEFINITIONS: &[ThemeDefinition] = &[
    ThemeDefinition {
        id: "dracula",
        label: "Dracula",
        description: "High-contrast default tuned for dark terminals.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x28, 0x2A, 0x36),
            accent: Color::Rgb(0xFF, 0x79, 0xC6),
            selection: Color::Rgb(0x44, 0x47, 0x5A),
        },
        aliases: &["dracula"],
        is_high_contrast: false,
        is_ansi_fallback: false,
        factory: || Box::new(DraculaTheme::new()),
    },
    ThemeDefinition {
        id: "dracula_hc",
        label: "Dracula High Contrast",
        description: "Sharper borders and brighter copy for dim displays.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x28, 0x2A, 0x36),
            accent: Color::Rgb(0xBD, 0x93, 0xF9),
            selection: Color::Rgb(0x44, 0x47, 0x5A),
        },
        aliases: &["dracula_hc", "dracula-high-contrast", "dracula-hc", "draculahc"],
        is_high_contrast: true,
        is_ansi_fallback: false,
        factory: || Box::new(DraculaThemeHighContrast::new()),
    },
    ThemeDefinition {
        id: "nord",
        label: "Nord",
        description: "Calm polar blues with aurora semantic accents.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x2E, 0x34, 0x40),
            accent: Color::Rgb(0x88, 0xC0, 0xD0),
            selection: Color::Rgb(0x5E, 0x81, 0xAC),
        },
        aliases: &["nord"],
        is_high_contrast: false,
        is_ansi_fallback: false,
        factory: || Box::new(NordTheme::new()),
    },
    ThemeDefinition {
        id: "nord_hc",
        label: "Nord High Contrast",
        description: "Nord surfaces with stronger borders and body text.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x2E, 0x34, 0x40),
            accent: Color::Rgb(0x5E, 0x81, 0xAC),
            selection: Color::Rgb(0x7D, 0x88, 0x9D),
        },
        aliases: &["nord_hc", "nord-high-contrast", "nord-hc", "nordhc"],
        is_high_contrast: true,
        is_ansi_fallback: false,
        factory: || Box::new(NordThemeHighContrast::new()),
    },
    ThemeDefinition {
        id: "cyberpunk",
        label: "Cyberpunk",
        description: "Neon purples with electric cyan focus cues.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x0D, 0x02, 0x21),
            accent: Color::Rgb(0x00, 0xF6, 0xFF),
            selection: Color::Rgb(0x2A, 0x1A, 0x5E),
        },
        aliases: &["cyberpunk"],
        is_high_contrast: false,
        is_ansi_fallback: false,
        factory: || Box::new(CyberpunkTheme::new()),
    },
    ThemeDefinition {
        id: "cyberpunk_hc",
        label: "Cyberpunk High Contrast",
        description: "Neon palette with amplified borders and text weight.",
        swatch: ThemeSwatch {
            background: Color::Rgb(0x0D, 0x02, 0x21),
            accent: Color::Rgb(0xFF, 0x4E, 0xCD),
            selection: Color::Rgb(0x3C, 0x1F, 0x7B),
        },
        aliases: &["cyberpunk_hc", "cyberpunk-high-contrast", "cyberpunk-hc", "cyberpunkhc"],
        is_high_contrast: true,
        is_ansi_fallback: false,
        factory: || Box::new(CyberpunkThemeHighContrast::new()),
    },
    ThemeDefinition {
        id: "ansi256",
        label: "ANSI 256",
        description: "Indexed fallback for 8-bit terminals.",
        swatch: ThemeSwatch {
            background: Color::Indexed(236),
            accent: Color::Indexed(212),
            selection: Color::Indexed(239),
        },
        aliases: &["ansi256"],
        is_high_contrast: false,
        is_ansi_fallback: true,
        factory: || Box::new(Ansi256Theme::new()),
    },
    ThemeDefinition {
        id: "ansi256_hc",
        label: "ANSI 256 High Contrast",
        description: "ANSI fallback with brighter borders and text.",
        swatch: ThemeSwatch {
            background: Color::Indexed(236),
            accent: Color::Indexed(141),
            selection: Color::Indexed(239),
        },
        aliases: &["ansi256_hc", "ansi256-high-contrast", "ansi256-hc", "ansi256hc"],
        is_high_contrast: true,
        is_ansi_fallback: true,
        factory: || Box::new(Ansi256ThemeHighContrast::new()),
    },
];

/// Iterate over all available definitions.
pub fn all() -> &'static [ThemeDefinition] {
    THEME_DEFINITIONS
}

/// Locate a definition by canonical id.
pub fn find_by_id(id: &str) -> Option<&'static ThemeDefinition> {
    THEME_DEFINITIONS.iter().find(|definition| definition.id.eq_ignore_ascii_case(id))
}

/// Locate a definition by alias (case-insensitive).
pub fn resolve(name: &str) -> Option<&'static ThemeDefinition> {
    let normalized = name.to_ascii_lowercase();
    THEME_DEFINITIONS.iter().find(|definition| {
        definition.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(&normalized)) || definition.id.eq_ignore_ascii_case(&normalized)
    })
}

/// Preferred default for truecolor terminals.
pub fn default_truecolor() -> &'static ThemeDefinition {
    THEME_DEFINITIONS
        .iter()
        .find(|definition| definition.id == "dracula")
        .expect("dracula theme registered")
}

/// Preferred default for ANSI-only terminals.
pub fn default_ansi() -> &'static ThemeDefinition {
    THEME_DEFINITIONS
        .iter()
        .find(|definition| definition.id == "ansi256")
        .expect("ansi256 theme registered")
}
