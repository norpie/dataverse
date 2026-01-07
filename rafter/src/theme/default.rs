//! Default rafter theme with dark purple accents.

use rafter_derive::theme;
use tuidom::Color;

#[theme]
pub struct ButtonColors {
    pub normal: Color,
    pub hover: Color,
    pub active: Color,
    pub disabled: Color,
}

#[theme]
pub struct InputColors {
    pub background: Color,
    pub border: Color,
    pub border_focus: Color,
    pub placeholder: Color,
}

#[theme]
pub struct TextColors {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub disabled: Color,
}

#[theme]
pub struct ScrollbarColors {
    pub track: Color,
    pub thumb: Color,
}

#[theme]
pub struct RafterTheme {
    // Core colors
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub accent: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,

    // Component groups
    #[group]
    pub button: ButtonColors,
    #[group]
    pub input: InputColors,
    #[group]
    pub text: TextColors,
    #[group]
    pub scrollbar: ScrollbarColors,
}

/// Create the default rafter theme.
/// Dark theme with purple accents.
pub fn default_theme() -> RafterTheme {
    RafterTheme {
        // Core: dark purple-gray bg, black surface, purple accent
        background: Color::oklch(0.15, 0.01, 280.0),
        surface: Color::oklch(0.0, 0.0, 0.0),
        border: Color::oklch(0.3, 0.02, 280.0),
        primary: Color::oklch(0.9, 0.0, 0.0),
        secondary: Color::oklch(0.75, 0.02, 280.0),
        muted: Color::oklch(0.5, 0.02, 280.0),
        accent: Color::oklch(0.6, 0.15, 280.0),

        // Semantic colors
        success: Color::oklch(0.7, 0.15, 145.0), // green
        warning: Color::oklch(0.75, 0.15, 85.0), // yellow/orange
        error: Color::oklch(0.65, 0.2, 25.0),    // red

        button: ButtonColors {
            normal: Color::oklch(0.25, 0.03, 280.0),
            hover: Color::oklch(0.35, 0.05, 280.0),
            active: Color::oklch(0.45, 0.08, 280.0),
            disabled: Color::oklch(0.15, 0.01, 280.0),
        },

        input: InputColors {
            background: Color::oklch(0.1, 0.01, 280.0),
            border: Color::oklch(0.3, 0.02, 280.0),
            border_focus: Color::oklch(0.6, 0.15, 280.0),
            placeholder: Color::oklch(0.5, 0.02, 280.0),
        },

        text: TextColors {
            primary: Color::oklch(0.9, 0.0, 0.0),
            secondary: Color::oklch(0.75, 0.02, 280.0),
            muted: Color::oklch(0.5, 0.02, 280.0),
            disabled: Color::oklch(0.35, 0.01, 280.0),
        },

        scrollbar: ScrollbarColors {
            track: Color::oklch(0.15, 0.01, 280.0),
            thumb: Color::oklch(0.4, 0.05, 280.0),
        },
    }
}
