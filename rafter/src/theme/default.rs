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
    pub cursor_fg: Color,
    pub cursor_bg: Color,
    pub selection_fg: Color,
    pub selection_bg: Color,
}

#[theme]
pub struct TextColors {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub disabled: Color,
    pub inverted: Color,
}

#[theme]
pub struct ScrollbarColors {
    pub track: Color,
    pub thumb: Color,
}

#[theme]
pub struct CheckboxColors {
    pub focused: Color,
    pub disabled: Color,
}

#[theme]
pub struct SelectColors {
    pub focused: Color,
    pub disabled: Color,
    pub dropdown_bg: Color,
    pub item_focused: Color,
}

#[theme]
pub struct ListColors {
    pub item_focused: Color,
    pub item_selected: Color,
}

#[theme]
pub struct RadioColors {
    pub focused: Color,
    pub disabled: Color,
}

#[theme]
pub struct CardColors {
    pub focused: Color,
    pub disabled: Color,
}

#[theme]
pub struct CollapsibleColors {
    pub focused: Color,
}

#[theme]
pub struct AutocompleteColors {
    pub focused: Color,
    pub disabled: Color,
    pub dropdown_bg: Color,
    pub item_focused: Color,
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
    #[group]
    pub checkbox: CheckboxColors,
    #[group]
    pub select: SelectColors,
    #[group]
    pub list: ListColors,
    #[group]
    pub radio: RadioColors,
    #[group]
    pub card: CardColors,
    #[group]
    pub collapsible: CollapsibleColors,
    #[group]
    pub autocomplete: AutocompleteColors,
}

/// Create the default rafter theme.
/// Dark theme with purple accents.
pub fn default_theme() -> RafterTheme {
    RafterTheme {
        // Core: dark purple-gray bg, black surface, purple accent
        background: Color::oklch(0.1684, 0.0, 0.0),
        surface: Color::oklch(0.1957, 0.014, 291.6),
        border: Color::oklch(0.3, 0.02, 280.0),
        primary: Color::oklch(0.9, 0.0, 0.0),
        secondary: Color::oklch(0.75, 0.02, 280.0),
        muted: Color::oklch(0.5, 0.02, 280.0),
        accent: Color::oklch(0.8044, 0.1774, 323.99),

        // Semantic colors
        success: Color::oklch(0.7, 0.15, 145.0), // green
        warning: Color::oklch(0.75, 0.15, 85.0), // yellow/orange
        error: Color::oklch(0.65, 0.2, 25.0),    // red

        button: ButtonColors {
            normal: Color::var("surface").lighten(0.05),
            hover: Color::var("surface").lighten(0.15),
            active: Color::var("surface").lighten(0.25),
            disabled: Color::var("surface").darken(0.05),
        },

        input: InputColors {
            background: Color::var("surface").darken(0.02),
            border: Color::var("surface").lighten(0.1),
            border_focus: Color::var("accent").darken(0.2),
            placeholder: Color::var("text.muted"),
            cursor_fg: Color::var("text.inverted"),
            cursor_bg: Color::var("text.primary"),
            selection_fg: Color::var("text.inverted"),
            selection_bg: Color::var("accent"),
        },

        text: TextColors {
            primary: Color::oklch(0.9, 0.0, 0.0),
            secondary: Color::oklch(0.75, 0.02, 280.0),
            muted: Color::oklch(0.5, 0.02, 280.0),
            disabled: Color::oklch(0.35, 0.01, 280.0),
            inverted: Color::oklch(0.15, 0.0, 0.0),
        },

        scrollbar: ScrollbarColors {
            track: Color::oklch(0.15, 0.01, 280.0),
            thumb: Color::oklch(0.4, 0.05, 280.0),
        },

        checkbox: CheckboxColors {
            focused: Color::var("surface").lighten(0.1),
            disabled: Color::var("surface").darken(0.1),
        },

        select: SelectColors {
            focused: Color::var("surface").lighten(0.1),
            disabled: Color::var("surface").darken(0.1),
            dropdown_bg: Color::var("surface"),
            item_focused: Color::var("accent"),
        },

        list: ListColors {
            item_focused: Color::var("accent"),
            item_selected: Color::var("accent").darken(0.3),
        },

        radio: RadioColors {
            focused: Color::var("surface").lighten(0.1),
            disabled: Color::var("surface").darken(0.1),
        },

        card: CardColors {
            focused: Color::var("surface").lighten(0.1),
            disabled: Color::var("surface").darken(0.1),
        },

        collapsible: CollapsibleColors {
            focused: Color::var("surface").lighten(0.1),
        },

        autocomplete: AutocompleteColors {
            focused: Color::var("surface").lighten(0.1),
            disabled: Color::var("surface").darken(0.1),
            dropdown_bg: Color::var("surface"),
            item_focused: Color::var("accent"),
        },
    }
}
