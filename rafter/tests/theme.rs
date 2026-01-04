use rafter::theme;
use tuidom::{Color, Theme};

#[theme]
struct ButtonColors {
    normal: Color,
    hover: Color,
    active: Color,
}

#[theme]
struct BorderColors {
    normal: Color,
    focus: Color,
}

#[theme]
struct InputColors {
    background: Color,
    #[group]
    border: BorderColors,
}

#[theme]
struct MyTheme {
    primary: Color,
    secondary: Color,
    #[group]
    button: ButtonColors,
    #[group]
    input: InputColors,
}

#[test]
fn test_direct_color_resolution() {
    let theme = MyTheme {
        primary: Color::rgb(255, 0, 0),
        secondary: Color::rgb(0, 255, 0),
        button: ButtonColors {
            normal: Color::rgb(100, 100, 100),
            hover: Color::rgb(120, 120, 120),
            active: Color::rgb(80, 80, 80),
        },
        input: InputColors {
            background: Color::rgb(40, 40, 40),
            border: BorderColors {
                normal: Color::rgb(60, 60, 60),
                focus: Color::rgb(0, 100, 200),
            },
        },
    };

    // Direct fields
    assert!(theme.resolve("primary").is_some());
    assert!(theme.resolve("secondary").is_some());

    // Non-existent
    assert!(theme.resolve("nonexistent").is_none());
}

#[test]
fn test_single_level_group() {
    let theme = MyTheme {
        primary: Color::rgb(255, 0, 0),
        secondary: Color::rgb(0, 255, 0),
        button: ButtonColors {
            normal: Color::rgb(100, 100, 100),
            hover: Color::rgb(120, 120, 120),
            active: Color::rgb(80, 80, 80),
        },
        input: InputColors {
            background: Color::rgb(40, 40, 40),
            border: BorderColors {
                normal: Color::rgb(60, 60, 60),
                focus: Color::rgb(0, 100, 200),
            },
        },
    };

    // Button group
    assert!(theme.resolve("button.normal").is_some());
    assert!(theme.resolve("button.hover").is_some());
    assert!(theme.resolve("button.active").is_some());
    assert!(theme.resolve("button.nonexistent").is_none());

    // Input direct field
    assert!(theme.resolve("input.background").is_some());
}

#[test]
fn test_nested_group() {
    let theme = MyTheme {
        primary: Color::rgb(255, 0, 0),
        secondary: Color::rgb(0, 255, 0),
        button: ButtonColors {
            normal: Color::rgb(100, 100, 100),
            hover: Color::rgb(120, 120, 120),
            active: Color::rgb(80, 80, 80),
        },
        input: InputColors {
            background: Color::rgb(40, 40, 40),
            border: BorderColors {
                normal: Color::rgb(60, 60, 60),
                focus: Color::rgb(0, 100, 200),
            },
        },
    };

    // Deeply nested
    assert!(theme.resolve("input.border.normal").is_some());
    assert!(theme.resolve("input.border.focus").is_some());
    assert!(theme.resolve("input.border.nonexistent").is_none());
}

#[test]
fn test_standalone_group() {
    let button = ButtonColors {
        normal: Color::rgb(100, 100, 100),
        hover: Color::rgb(120, 120, 120),
        active: Color::rgb(80, 80, 80),
    };

    // ButtonColors works as a standalone theme
    assert!(button.resolve("normal").is_some());
    assert!(button.resolve("hover").is_some());
    assert!(button.resolve("active").is_some());
}

#[test]
fn test_color_values() {
    let theme = MyTheme {
        primary: Color::rgb(255, 0, 0),
        secondary: Color::rgb(0, 255, 0),
        button: ButtonColors {
            normal: Color::rgb(100, 100, 100),
            hover: Color::rgb(120, 120, 120),
            active: Color::rgb(80, 80, 80),
        },
        input: InputColors {
            background: Color::rgb(40, 40, 40),
            border: BorderColors {
                normal: Color::rgb(60, 60, 60),
                focus: Color::rgb(0, 100, 200),
            },
        },
    };

    // Verify actual color values
    assert_eq!(theme.resolve("primary"), Some(&Color::rgb(255, 0, 0)));
    assert_eq!(theme.resolve("button.hover"), Some(&Color::rgb(120, 120, 120)));
    assert_eq!(theme.resolve("input.border.focus"), Some(&Color::rgb(0, 100, 200)));
}
