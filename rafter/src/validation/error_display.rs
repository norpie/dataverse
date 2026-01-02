/// Where to display validation errors for a widget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ErrorDisplay {
    /// Show error message below the widget (default).
    #[default]
    Below,
    /// Show error message inline/to the right of the widget.
    Inline,
    /// Don't display error message - widget only shows error styling.
    None,
}
