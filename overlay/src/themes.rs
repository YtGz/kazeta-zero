use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

/// Complete theme definition for overlay UI
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub background_overlay: Color,  // Semi-transparent overlay
    pub panel_background: Color,
    pub panel_border: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub text_disabled: Color,
    pub cursor: Color,
    pub accent: Color,
    pub accent_secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

impl Theme {
    /// Create a theme from individual color components
    pub fn new(
        name: String,
        background: Color,
        background_overlay: Color,
        panel_background: Color,
        panel_border: Color,
        text: Color,
        text_secondary: Color,
        text_disabled: Color,
        cursor: Color,
        accent: Color,
        accent_secondary: Color,
        success: Color,
        warning: Color,
        error: Color,
        info: Color,
    ) -> Self {
        Self {
            name,
            background,
            background_overlay,
            panel_background,
            panel_border,
            text,
            text_secondary,
            text_disabled,
            cursor,
            accent,
            accent_secondary,
            success,
            warning,
            error,
            info,
        }
    }
}

/// Preset themes
impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self::new(
            "Dark".to_string(),
            Color::new(0.1, 0.1, 0.1, 1.0),
            Color::new(0.0, 0.0, 0.0, 0.75),
            Color::new(0.2, 0.2, 0.2, 1.0),
            WHITE,
            WHITE,
            LIGHTGRAY,
            GRAY,
            YELLOW,
            Color::new(0.2, 0.6, 1.0, 1.0),  // Blue accent
            Color::new(0.4, 0.7, 1.0, 1.0),  // Light blue
            GREEN,
            ORANGE,
            RED,
            Color::new(0.2, 0.4, 0.8, 1.0),  // Blue info
        )
    }

    /// Light theme
    pub fn light() -> Self {
        Self::new(
            "Light".to_string(),
            Color::new(0.95, 0.95, 0.95, 1.0),
            Color::new(0.0, 0.0, 0.0, 0.5),
            Color::new(1.0, 1.0, 1.0, 1.0),
            Color::new(0.2, 0.2, 0.2, 1.0),
            Color::new(0.1, 0.1, 0.1, 1.0),
            Color::new(0.3, 0.3, 0.3, 1.0),
            Color::new(0.6, 0.6, 0.6, 1.0),
            Color::new(0.0, 0.4, 0.8, 1.0),  // Blue cursor
            Color::new(0.0, 0.5, 1.0, 1.0),  // Blue accent
            Color::new(0.2, 0.6, 1.0, 1.0),  // Light blue
            Color::new(0.0, 0.7, 0.0, 1.0),  // Green
            Color::new(1.0, 0.6, 0.0, 1.0),  // Orange
            Color::new(0.9, 0.2, 0.2, 1.0),  // Red
            Color::new(0.0, 0.5, 0.9, 1.0),  // Blue info
        )
    }

    /// Retro Green (terminal/Matrix style)
    pub fn retro_green() -> Self {
        Self::new(
            "Retro Green".to_string(),
            BLACK,
            Color::new(0.0, 0.0, 0.0, 0.85),
            Color::new(0.0, 0.1, 0.0, 1.0),
            Color::new(0.0, 1.0, 0.0, 1.0),
            Color::new(0.0, 1.0, 0.0, 1.0),
            Color::new(0.0, 0.8, 0.0, 1.0),
            Color::new(0.0, 0.5, 0.0, 1.0),
            Color::new(0.0, 1.0, 0.5, 1.0),  // Cyan-green cursor
            Color::new(0.0, 1.0, 0.0, 1.0),  // Green accent
            Color::new(0.0, 0.8, 0.5, 1.0),  // Light green
            Color::new(0.0, 1.0, 0.0, 1.0),  // Green
            Color::new(1.0, 1.0, 0.0, 1.0),  // Yellow warning
            Color::new(1.0, 0.0, 0.0, 1.0),  // Red
            Color::new(0.0, 0.8, 1.0, 1.0),  // Cyan info
        )
    }

    /// PlayStation style (blue/purple)
    pub fn playstation() -> Self {
        Self::new(
            "PlayStation".to_string(),
            Color::new(0.05, 0.05, 0.15, 1.0),
            Color::new(0.0, 0.0, 0.0, 0.8),
            Color::new(0.1, 0.1, 0.2, 0.98),
            Color::new(0.0, 0.4, 1.0, 1.0),  // PS Blue
            WHITE,
            LIGHTGRAY,
            GRAY,
            Color::new(0.0, 0.6, 1.0, 1.0),  // PS Blue cursor
            Color::new(0.0, 0.4, 1.0, 1.0),  // PS Blue accent
            Color::new(0.3, 0.5, 1.0, 1.0),  // Light blue
            Color::new(0.0, 0.8, 0.4, 1.0),  // Green
            Color::new(1.0, 0.7, 0.0, 1.0),  // Orange
            Color::new(1.0, 0.2, 0.2, 1.0),  // Red
            Color::new(0.0, 0.5, 1.0, 1.0),  // Blue info
        )
    }

    /// Xbox style (green)
    pub fn xbox() -> Self {
        Self::new(
            "Xbox".to_string(),
            Color::new(0.1, 0.1, 0.1, 1.0),
            Color::new(0.0, 0.0, 0.0, 0.75),
            Color::new(0.15, 0.15, 0.15, 0.98),
            Color::new(0.2, 0.8, 0.2, 1.0),  // Xbox Green
            WHITE,
            LIGHTGRAY,
            GRAY,
            Color::new(0.2, 0.8, 0.2, 1.0),  // Xbox Green cursor
            Color::new(0.2, 0.8, 0.2, 1.0),  // Xbox Green accent
            Color::new(0.4, 0.9, 0.4, 1.0),  // Light green
            Color::new(0.2, 0.8, 0.2, 1.0),  // Green
            Color::new(1.0, 0.7, 0.0, 1.0),  // Orange
            Color::new(1.0, 0.2, 0.2, 1.0),  // Red
            Color::new(0.2, 0.6, 0.9, 1.0),  // Blue info
        )
    }

    /// Get all available preset themes
    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::dark(),
            Self::light(),
            Self::retro_green(),
            Self::playstation(),
            Self::xbox(),
        ]
    }

    /// Find a theme by name
    pub fn by_name(name: &str) -> Option<Self> {
        Self::all_presets()
            .into_iter()
            .find(|theme| theme.name.eq_ignore_ascii_case(name))
    }
}

// Note: Color serialization is handled via serde's skip_serializing for Theme
// Themes are defined as code presets, not serialized from config
// Only the theme name is stored in config

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_presets() {
        let dark = Theme::dark();
        assert_eq!(dark.name, "Dark");
        
        let light = Theme::light();
        assert_eq!(light.name, "Light");
        
        let all = Theme::all_presets();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_theme_by_name() {
        assert!(Theme::by_name("Dark").is_some());
        assert!(Theme::by_name("dark").is_some());
        assert!(Theme::by_name("DARK").is_some());
        assert!(Theme::by_name("Invalid").is_none());
    }
}

