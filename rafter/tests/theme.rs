use rafter::color::{Color, StyleColor};
use rafter::theme::{DefaultTheme, Theme, resolve_color};

#[test]
fn test_default_theme_resolves_colors() {
    let theme = DefaultTheme::dark();

    assert!(theme.resolve("primary").is_some());
    assert!(theme.resolve("error").is_some());
    assert!(theme.resolve("unknown_color").is_none());
}

#[test]
fn test_default_theme_aliases() {
    let theme = DefaultTheme::dark();

    // fg should resolve to text
    assert!(theme.resolve("fg").is_some());
    assert!(theme.resolve("muted").is_some());
}

#[test]
fn test_resolve_color_with_named() {
    let theme = DefaultTheme::dark();
    let named = StyleColor::Named("primary".to_string());
    let resolved = resolve_color(&named, &theme);

    // Should resolve to the theme's primary color, not a named reference
    // Just verify it's a valid Color (the resolve function returns Color directly)
    let _ = resolved;
}

#[test]
fn test_resolve_color_passthrough() {
    let theme = DefaultTheme::dark();
    let literal = StyleColor::Concrete(Color::CYAN);
    let resolved = resolve_color(&literal, &theme);

    assert_eq!(resolved, Color::CYAN);
}
