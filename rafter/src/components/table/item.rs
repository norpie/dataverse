//! TableRow trait and Column types for table display.

use crate::node::Node;

/// Horizontal alignment for column content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Column configuration.
///
/// Columns define the structure of the table: header text, width,
/// alignment, and whether the column is sortable.
///
/// # Examples
///
/// ```ignore
/// let columns = vec![
///     Column::new("ID", 8),
///     Column::new("Name", 30).sortable(),
///     Column::new("Status", 15).align(Alignment::Center),
/// ];
/// ```
#[derive(Debug, Clone)]
pub struct Column {
    /// Column header text
    pub header: String,
    /// Column width in terminal columns (fixed)
    pub width: u16,
    /// Horizontal alignment
    pub align: Alignment,
    /// Whether this column is sortable
    pub sortable: bool,
}

impl Column {
    /// Create a new column with explicit width.
    ///
    /// Width must be specified - no defaults.
    ///
    /// # Arguments
    /// * `header` - The column header text
    /// * `width` - Width in terminal columns
    pub fn new(header: impl Into<String>, width: u16) -> Self {
        Self {
            header: header.into(),
            width,
            align: Alignment::Left,
            sortable: false,
        }
    }

    /// Set the column alignment.
    pub fn align(mut self, align: Alignment) -> Self {
        self.align = align;
        self
    }

    /// Make the column sortable.
    ///
    /// Sortable columns show sort indicators in the header and
    /// respond to clicks to trigger `on_sort` events.
    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }
}

/// Trait for items that can be displayed as rows in a Table.
///
/// Implement this trait to define how your data renders in table rows.
///
/// # Default Styling Helpers
///
/// The trait provides composable helpers for common styling patterns:
///
/// - [`apply_default_row_style`](TableRow::apply_default_row_style) - Wrap row with standard focus/selection colors
/// - [`render_row`](TableRow::render_row) - Override to customize entire row layout
/// - [`selection_indicator`](TableRow::selection_indicator) - Get checkbox string "■" or "□"
///
/// # Examples
///
/// ## Simple row with defaults
/// ```ignore
/// impl TableRow for User {
///     fn id(&self) -> String { self.id.clone() }
///     fn column_count(&self) -> usize { 3 }
///     
///     fn render_cell(&self, col_idx: usize, focused: bool, selected: bool) -> Option<Node> {
///         let content = match col_idx {
///             0 => view! { text { self.name.clone() } },
///             1 => view! { text { self.email.clone() } },
///             2 => view! { text { self.age.to_string() } },
///             _ => return None,
///         };
///         Some(content) // Table applies default row styling automatically
///     }
/// }
/// ```
///
/// ## Row with custom cell styling
/// ```ignore
/// impl TableRow for User {
///     fn id(&self) -> String { self.id.clone() }
///     fn column_count(&self) -> usize { 3 }
///     
///     fn render_cell(&self, col_idx: usize, focused: bool, selected: bool) -> Option<Node> {
///         match col_idx {
///             0 => Some(view! { text (bold) { self.name.clone() } }),
///             1 => Some(view! { text (fg: muted) { self.email.clone() } }),
///             2 => Some(view! {
///                 text (fg: if self.status == "Active" { success } else { warning }) {
///                     self.status.clone()
///                 }
///             }),
///             _ => None,
///         }
///     }
/// }
/// ```
///
/// ## Override entire row layout
/// ```ignore
/// impl TableRow for User {
///     fn id(&self) -> String { self.id.clone() }
///     fn column_count(&self) -> usize { 3 }
///     
///     fn render_cell(&self, col_idx: usize, focused: bool, selected: bool) -> Option<Node> {
///         // ... render cells ...
///     }
///     
///     fn render_row(&self, cells: Vec<Node>, focused: bool, selected: bool) -> Node {
///         // Custom row layout
///         let row_content = view! {
///             row (width: fill, gap: 2) {
///                 @for cell in cells {
///                     cell
///                 }
///             }
///         };
///         Self::apply_default_row_style(row_content, focused, selected)
///     }
/// }
/// ```
pub trait TableRow: Send + Sync + Clone + 'static {
    /// Unique identifier for this row.
    ///
    /// Used for stable selection across row mutations.
    fn id(&self) -> String;

    /// Total number of columns this row has.
    ///
    /// Must match the number of columns in the table.
    fn column_count(&self) -> usize;

    /// Render a specific column cell for this row.
    ///
    /// # Arguments
    /// * `column_index` - The 0-based index of the column to render
    /// * `focused` - Whether this row has the cursor
    /// * `selected` - Whether this row is selected
    ///
    /// Returns `None` if the column index is out of bounds.
    fn render_cell(&self, column_index: usize, focused: bool, selected: bool) -> Option<Node>;

    /// Height of this row in terminal rows (fixed for all rows).
    const HEIGHT: u16 = 1;

    /// Render entire row (override for custom layout).
    ///
    /// Default implementation arranges cells horizontally and applies default styling.
    /// Override to customize row layout (e.g., add separators, padding, custom backgrounds).
    ///
    /// # Arguments
    /// * `cells` - Rendered cells (one per visible column, in order)
    /// * `focused` - Whether this row has the cursor
    /// * `selected` - Whether this row is selected
    fn render_row(&self, cells: Vec<Node>, focused: bool, selected: bool) -> Node {
        use crate::node::Layout;
        use crate::style::Style;

        let row_content = Node::Row {
            children: cells,
            style: Style::new(),
            layout: Layout::default(),
        };
        Self::apply_default_row_style(row_content, focused, selected)
    }

    /// Helper: Apply default row colors to a row container.
    ///
    /// Uses the same purple color scheme as List/Tree components.
    /// Wraps the entire row with standard focus/selection colors.
    ///
    /// Colors:
    /// - Focused: bg=#A277FF (bright purple), fg=background (inverted)
    /// - Selected: bg=#6E5494 (dim purple), fg=background (inverted)
    /// - Neither: no styling applied
    fn apply_default_row_style(child: Node, focused: bool, selected: bool) -> Node {
        use crate::color::{Color, StyleColor};
        use crate::node::{Layout, Size};
        use crate::style::Style;

        let style = if focused || selected {
            // Focused gets brighter purple, selected gets dimmer purple
            let bg_color = if focused {
                Color::hex(0xA277FF) // Bright purple for cursor
            } else {
                Color::hex(0x6E5494) // Dimmer purple for selected
            };
            Style::new()
                .bg(StyleColor::Concrete(bg_color))
                .fg(StyleColor::Named("background".into())) // Inverted: use bg color as fg
        } else {
            Style::new()
        };

        let layout = Layout {
            width: Size::Flex(1),
            ..Default::default()
        };

        Node::Row {
            children: vec![child],
            style,
            layout,
        }
    }

    /// Helper: Get selection indicator (checkbox).
    ///
    /// Returns `"■ "` for selected, `"□ "` for unselected.
    fn selection_indicator(selected: bool) -> &'static str {
        if selected { "■ " } else { "□ " }
    }
}
