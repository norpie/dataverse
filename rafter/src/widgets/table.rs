//! Table widget - a virtualized table with rows and columns.
//!
//! This widget only creates Element objects for visible rows, enabling
//! smooth scrolling with large datasets. Supports frozen columns and
//! horizontal scrolling.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use tuidom::{Color, Element, Overflow, Size, Style, Transitions};

use crate::state::State;
use crate::{HandlerRegistry, WidgetHandlers};

use super::scroll::{
    register_scrollbar_handlers, ScrollRequest, ScrollState, ScrollableWidgetState, Scrollbar,
};
use super::selection::{Selection, SelectionMode};

// =============================================================================
// Column
// =============================================================================

/// Column width specification.
#[derive(Clone, Debug)]
pub enum ColumnWidth {
    /// Fixed width in characters.
    Fixed(u16),
    /// Flexible width with weight.
    Flex(u16),
    /// Auto-size to content (not yet implemented - treated as Flex(1)).
    Auto,
}

impl Default for ColumnWidth {
    fn default() -> Self {
        ColumnWidth::Flex(1)
    }
}

/// A table column definition.
#[derive(Clone, Debug)]
pub struct Column {
    /// Unique identifier for this column.
    pub id: String,
    /// Header text displayed at the top.
    pub header: String,
    /// Width specification.
    pub width: ColumnWidth,
}

impl Column {
    /// Create a new column with the given id and header.
    pub fn new(id: impl Into<String>, header: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            header: header.into(),
            width: ColumnWidth::default(),
        }
    }

    /// Set a fixed width for this column.
    pub fn fixed(mut self, width: u16) -> Self {
        self.width = ColumnWidth::Fixed(width);
        self
    }

    /// Set a flex width for this column.
    pub fn flex(mut self, weight: u16) -> Self {
        self.width = ColumnWidth::Flex(weight);
        self
    }

    /// Set auto width for this column.
    pub fn auto(mut self) -> Self {
        self.width = ColumnWidth::Auto;
        self
    }
}

// =============================================================================
// TableRow Trait
// =============================================================================

/// Trait for rows that can be displayed in a Table widget.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone)]
/// struct User {
///     id: u32,
///     name: String,
///     email: String,
/// }
///
/// impl TableRow for User {
///     type Key = u32;
///
///     fn key(&self) -> u32 {
///         self.id
///     }
///
///     fn cell(&self, column_id: &str) -> Element {
///         match column_id {
///             "name" => Element::text(&self.name),
///             "email" => Element::text(&self.email),
///             _ => Element::text(""),
///         }
///     }
/// }
/// ```
pub trait TableRow: Clone + Send + Sync + 'static {
    /// The key type used to identify this row.
    type Key: Clone + Eq + Hash + ToString + Send + Sync + 'static;

    /// Return a unique key for this row.
    fn key(&self) -> Self::Key;

    /// Render the cell content for the given column.
    fn cell(&self, column_id: &str) -> Element;

    /// Height of this row in terminal rows.
    fn height(&self) -> u16 {
        1 // Default: 1 row
    }
}

// =============================================================================
// TableState
// =============================================================================

/// State for a virtualized Table widget.
///
/// Uses cumulative height caching for O(1) position lookups.
#[derive(Clone, Debug)]
pub struct TableState<T: TableRow> {
    /// The rows in the table.
    pub rows: Vec<T>,
    /// Column definitions.
    pub columns: Vec<Column>,
    /// Selection state.
    pub selection: Selection<T::Key>,
    /// Vertical scroll state for virtualization.
    pub scroll: ScrollState,
    /// The key of the last activated row.
    pub last_activated: Option<T::Key>,

    /// Cached cumulative heights for O(1) position lookups.
    cumulative_heights: Vec<u16>,

    /// Set of frozen column IDs.
    frozen_column_ids: HashSet<String>,

    /// Number of frozen columns (columns are reordered so frozen are first).
    frozen_count: usize,

    /// Scrollbar screen rect for drag calculations.
    scrollbar_rect: Option<(u16, u16, u16, u16)>,

    /// Grab offset within thumb for smooth dragging.
    drag_grab_offset: Option<u16>,

    /// Horizontal scroll offset for the scrollable (non-frozen) section.
    pub horizontal_scroll_offset: u16,

    /// Currently focused row key (for syncing focus styling across frozen/scrollable panels).
    pub focused_key: Option<T::Key>,
}

impl<T: TableRow> Default for TableState<T> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            columns: Vec::new(),
            selection: Selection::none(),
            scroll: ScrollState::new(),
            last_activated: None,
            cumulative_heights: vec![0],
            frozen_column_ids: HashSet::new(),
            frozen_count: 0,
            scrollbar_rect: None,
            drag_grab_offset: None,
            horizontal_scroll_offset: 0,
            focused_key: None,
        }
    }
}

impl<T: TableRow> TableState<T> {
    /// Create a new TableState with the given rows and columns.
    pub fn new(rows: Vec<T>, columns: Vec<Column>) -> Self {
        let mut state = Self::default();
        state.set_columns(columns);
        state.set_rows(rows);
        state
    }

    /// Set the selection mode.
    pub fn with_selection(mut self, mode: SelectionMode) -> Self {
        self.selection = match mode {
            SelectionMode::None => Selection::none(),
            SelectionMode::Single => Selection::single(),
            SelectionMode::Multi => Selection::multi(),
        };
        self
    }

    /// Freeze the specified columns (by ID).
    /// Columns will be reordered so frozen columns appear first.
    pub fn with_frozen(mut self, column_ids: &[&str]) -> Self {
        self.frozen_column_ids = column_ids.iter().map(|s| s.to_string()).collect();
        self.reorder_columns_for_frozen();
        self
    }

    /// Set columns and reorder for frozen.
    pub fn set_columns(&mut self, columns: Vec<Column>) {
        self.columns = columns;
        self.reorder_columns_for_frozen();
    }

    /// Reorder columns so frozen columns come first.
    fn reorder_columns_for_frozen(&mut self) {
        if self.frozen_column_ids.is_empty() {
            self.frozen_count = 0;
            return;
        }

        // Partition columns: frozen first, then non-frozen
        let mut frozen = Vec::new();
        let mut non_frozen = Vec::new();

        for col in self.columns.drain(..) {
            if self.frozen_column_ids.contains(&col.id) {
                frozen.push(col);
            } else {
                non_frozen.push(col);
            }
        }

        self.frozen_count = frozen.len();
        self.columns = frozen;
        self.columns.extend(non_frozen);
    }

    /// Set rows and rebuild cumulative height cache.
    pub fn set_rows(&mut self, rows: Vec<T>) {
        self.cumulative_heights = Vec::with_capacity(rows.len() + 1);
        self.cumulative_heights.push(0);

        let mut total: u16 = 0;
        for row in &rows {
            total = total.saturating_add(row.height());
            self.cumulative_heights.push(total);
        }

        self.rows = rows;
        self.scroll.set_content_height(total);
    }

    /// Get Y offset for row at index. O(1).
    pub fn row_y_offset(&self, index: usize) -> u16 {
        self.cumulative_heights.get(index).copied().unwrap_or(0)
    }

    /// Get total content height. O(1).
    pub fn total_height(&self) -> u16 {
        self.cumulative_heights.last().copied().unwrap_or(0)
    }

    /// Find row index at given Y offset. O(log n) binary search.
    pub fn row_at_offset(&self, y: u16) -> usize {
        self.cumulative_heights
            .partition_point(|&h| h <= y)
            .saturating_sub(1)
    }

    /// Get height of row at index. O(1).
    pub fn row_height(&self, index: usize) -> u16 {
        if index + 1 < self.cumulative_heights.len() {
            self.cumulative_heights[index + 1] - self.cumulative_heights[index]
        } else {
            1
        }
    }

    /// Get the index of a row by key.
    pub fn index_of(&self, key: &T::Key) -> Option<usize> {
        self.rows.iter().position(|row| &row.key() == key)
    }

    /// Get frozen columns (first N columns).
    pub fn frozen_columns(&self) -> &[Column] {
        &self.columns[..self.frozen_count]
    }

    /// Get scrollable columns (after frozen columns).
    pub fn scrollable_columns(&self) -> &[Column] {
        &self.columns[self.frozen_count..]
    }

    /// Check if there are frozen columns.
    pub fn has_frozen_columns(&self) -> bool {
        self.frozen_count > 0
    }

    /// Process any pending scroll request.
    pub fn process_scroll(&mut self) -> bool {
        let old_offset = self.scroll.offset;

        if let Some(request) = self.scroll.process_request() {
            if let ScrollRequest::IntoView(index) = request {
                let y = self.row_y_offset(index);
                let row_h = self.row_height(index);
                let viewport = self.scroll.viewport;
                let offset = self.scroll.offset;

                if y < offset {
                    self.scroll.offset = y;
                } else if y + row_h > offset + viewport {
                    self.scroll.offset = (y + row_h).saturating_sub(viewport);
                }
            }
        }

        self.scroll.offset != old_offset
    }

    /// Get the index of the first visible row based on current scroll offset.
    pub fn first_visible_index(&self) -> usize {
        self.row_at_offset(self.scroll.offset)
    }
}

// Implement ScrollableWidgetState for TableState
impl<T: TableRow> ScrollableWidgetState for TableState<T> {
    fn scroll(&self) -> &ScrollState {
        &self.scroll
    }

    fn scroll_mut(&mut self) -> &mut ScrollState {
        &mut self.scroll
    }

    fn scrollbar_rect(&self) -> Option<(u16, u16, u16, u16)> {
        self.scrollbar_rect
    }

    fn set_scrollbar_rect(&mut self, rect: Option<(u16, u16, u16, u16)>) {
        self.scrollbar_rect = rect;
    }

    fn drag_grab_offset(&self) -> Option<u16> {
        self.drag_grab_offset
    }

    fn set_drag_grab_offset(&mut self, offset: Option<u16>) {
        self.drag_grab_offset = offset;
    }
}

// =============================================================================
// VisibleRow (internal)
// =============================================================================

/// Information about a visible row for rendering.
struct VisibleRow {
    /// Index in the rows array.
    index: usize,
}

// =============================================================================
// Table Widget
// =============================================================================

/// Typestate marker: table needs a state reference.
pub struct NeedsState;

/// Typestate marker: table has a state reference.
pub struct HasTableState<'a, T: TableRow>(pub(crate) &'a State<TableState<T>>);

/// A virtualized table widget builder.
///
/// Uses typestate pattern to enforce `state()` is called before `build()`.
pub struct Table<S = NeedsState> {
    state_marker: S,
    id: Option<String>,
    style: Option<Style>,
    header_style: Option<Style>,
    row_style: Option<Style>,
    row_style_selected: Option<Style>,
    row_style_focused: Option<Style>,
    cell_style: Option<Style>,
    transitions: Option<Transitions>,
    /// Whether to show vertical scrollbar (default: true).
    show_scrollbar: bool,
    /// Whether to show the header row (default: true).
    show_header: bool,
}

impl Default for Table<NeedsState> {
    fn default() -> Self {
        Self::new()
    }
}

impl Table<NeedsState> {
    /// Create a new table builder.
    pub fn new() -> Self {
        Self {
            state_marker: NeedsState,
            id: None,
            style: None,
            header_style: None,
            row_style: None,
            row_style_selected: None,
            row_style_focused: None,
            cell_style: None,
            transitions: None,
            show_scrollbar: true,
            show_header: true,
        }
    }

    /// Set the state reference. Required before calling `build()`.
    pub fn state<T: TableRow>(self, s: &State<TableState<T>>) -> Table<HasTableState<'_, T>> {
        Table {
            state_marker: HasTableState(s),
            id: self.id,
            style: self.style,
            header_style: self.header_style,
            row_style: self.row_style,
            row_style_selected: self.row_style_selected,
            row_style_focused: self.row_style_focused,
            cell_style: self.cell_style,
            transitions: self.transitions,
            show_scrollbar: self.show_scrollbar,
            show_header: self.show_header,
        }
    }
}

impl<S> Table<S> {
    /// Set the table id.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the table container style.
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }

    /// Set the header row style.
    pub fn header_style(mut self, s: Style) -> Self {
        self.header_style = Some(s);
        self
    }

    /// Set the style for each data row.
    pub fn row_style(mut self, s: Style) -> Self {
        self.row_style = Some(s);
        self
    }

    /// Set the style for selected rows.
    pub fn row_style_selected(mut self, s: Style) -> Self {
        self.row_style_selected = Some(s);
        self
    }

    /// Set the style when a row is focused.
    pub fn row_style_focused(mut self, s: Style) -> Self {
        self.row_style_focused = Some(s);
        self
    }

    /// Set the style for cells.
    pub fn cell_style(mut self, s: Style) -> Self {
        self.cell_style = Some(s);
        self
    }

    /// Set transitions.
    pub fn transitions(mut self, t: Transitions) -> Self {
        self.transitions = Some(t);
        self
    }

    /// Set whether to show the vertical scrollbar.
    pub fn show_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set whether to show the header row.
    pub fn show_header(mut self, show: bool) -> Self {
        self.show_header = show;
        self
    }
}

impl<'a, T: TableRow> Table<HasTableState<'a, T>> {
    /// Build the table element.
    pub fn build(self, registry: &HandlerRegistry, handlers: &WidgetHandlers) -> Element {
        let state = self.state_marker.0;
        let table_id = self.id.clone().unwrap_or_else(|| "table".into());

        // Process any pending scroll request
        state.update(|s| {
            s.process_scroll();
        });

        let current = state.get();

        // Calculate visible rows
        let visible = self.calculate_visible_rows(&current);

        // Build the table structure based on whether we have frozen columns
        if current.has_frozen_columns() {
            self.build_with_frozen_columns(
                &current,
                &visible,
                &table_id,
                registry,
                handlers,
                state,
            )
        } else {
            self.build_simple_table(&current, &visible, &table_id, registry, handlers, state)
        }
    }

    /// Calculate which rows are visible given current scroll state.
    fn calculate_visible_rows(&self, state: &TableState<T>) -> Vec<VisibleRow> {
        let scroll_y = state.scroll.offset;
        let viewport = state.scroll.viewport;

        if state.rows.is_empty() {
            return Vec::new();
        }

        let effective_viewport = if viewport == 0 { 200 } else { viewport };

        let first_visible = state.row_at_offset(scroll_y);

        let mut end_idx = first_visible;
        let mut total_height: u16 = 0;
        while end_idx < state.rows.len() && total_height < effective_viewport {
            total_height += state.row_height(end_idx);
            end_idx += 1;
        }

        let mut rows = Vec::with_capacity(end_idx - first_visible);
        for i in first_visible..end_idx {
            rows.push(VisibleRow { index: i });
        }

        rows
    }

    /// Build a simple table without frozen columns.
    fn build_simple_table(
        &self,
        current: &TableState<T>,
        visible: &[VisibleRow],
        table_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<TableState<T>>,
    ) -> Element {
        let content_id = format!("{}-content", table_id);
        let header_id = format!("{}-header", table_id);
        let body_id = format!("{}-body", table_id);

        // Build header row
        let header = if self.show_header {
            Some(self.build_header_row(&current.columns, &header_id))
        } else {
            None
        };

        // Build data rows
        let visible_count = visible.len();
        let total_rows = current.rows.len();
        let mut row_elements = Vec::with_capacity(visible.len());

        for (pos_in_visible, vis_row) in visible.iter().enumerate() {
            let row = &current.rows[vis_row.index];
            let row_element = self.build_data_row(
                row,
                &current.columns,
                vis_row.index,
                pos_in_visible,
                visible_count,
                total_rows,
                &current.selection,
                table_id,
                registry,
                handlers,
                state,
            );
            row_elements.push(row_element);
        }

        // Build body
        let body = Element::col()
            .id(&body_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Hidden)
            .scrollable(true)
            .children(row_elements);

        // Register scroll handlers
        self.register_scroll_handlers(&body_id, &content_id, registry, state, table_id, "row");

        // Register layout handler
        self.register_layout_handler(&body_id, registry, state);

        // Build content column (header + body)
        let mut content_children = Vec::new();
        if let Some(h) = header {
            content_children.push(h);
        }
        content_children.push(body);

        let mut content = Element::col()
            .id(&content_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .children(content_children);

        if let Some(ref style) = self.style {
            content = content.style(style.clone());
        }
        if let Some(ref transitions) = self.transitions {
            content = content.transitions(transitions.clone());
        }

        // Add scrollbar if needed
        let show_scrollbar = self.show_scrollbar && current.scroll.can_scroll();
        if show_scrollbar {
            let scrollbar_id = format!("{}-scrollbar", table_id);
            let scrollbar = Scrollbar::vertical()
                .id(&scrollbar_id)
                .scroll_state(&current.scroll)
                .build();

            register_scrollbar_handlers(&scrollbar_id, registry, state);

            Element::row()
                .id(table_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .child(content)
                .child(scrollbar)
        } else {
            content.id(table_id)
        }
    }

    /// Build a table with frozen columns.
    fn build_with_frozen_columns(
        &self,
        current: &TableState<T>,
        visible: &[VisibleRow],
        table_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<TableState<T>>,
    ) -> Element {
        let frozen_panel_id = format!("{}-frozen", table_id);
        let scrollable_panel_id = format!("{}-scrollable", table_id);
        let frozen_header_id = format!("{}-frozen-header", table_id);
        let frozen_body_id = format!("{}-frozen-body", table_id);
        let scrollable_header_id = format!("{}-scrollable-header", table_id);
        let scrollable_body_id = format!("{}-scrollable-body", table_id);

        let frozen_columns = current.frozen_columns();
        let scrollable_columns = current.scrollable_columns();

        let visible_count = visible.len();
        let total_rows = current.rows.len();

        // Build frozen header
        let frozen_header = if self.show_header {
            Some(self.build_header_row(frozen_columns, &frozen_header_id))
        } else {
            None
        };

        // Build scrollable header
        let scrollable_header = if self.show_header {
            Some(self.build_header_row(scrollable_columns, &scrollable_header_id))
        } else {
            None
        };

        // Build frozen body rows
        let mut frozen_rows = Vec::with_capacity(visible.len());
        for (pos_in_visible, vis_row) in visible.iter().enumerate() {
            let row = &current.rows[vis_row.index];
            let row_element = self.build_data_row_for_columns(
                row,
                frozen_columns,
                vis_row.index,
                pos_in_visible,
                visible_count,
                total_rows,
                &current.selection,
                table_id,
                "frozen",
                registry,
                handlers,
                state,
            );
            frozen_rows.push(row_element);
        }

        // Build scrollable body rows
        let mut scrollable_rows = Vec::with_capacity(visible.len());
        for (pos_in_visible, vis_row) in visible.iter().enumerate() {
            let row = &current.rows[vis_row.index];
            let row_element = self.build_data_row_for_columns(
                row,
                scrollable_columns,
                vis_row.index,
                pos_in_visible,
                visible_count,
                total_rows,
                &current.selection,
                table_id,
                "scrollable",
                registry,
                handlers,
                state,
            );
            scrollable_rows.push(row_element);
        }

        // Build frozen body (no horizontal scroll)
        let frozen_body = Element::col()
            .id(&frozen_body_id)
            .width(Size::Auto)
            .height(Size::Fill)
            .overflow_x(Overflow::Hidden)
            .overflow_y(Overflow::Hidden)
            .scrollable(true) // Enable scroll events for Page/Home/End
            .children(frozen_rows);

        // Build scrollable body (horizontal scroll)
        let scrollable_body = Element::col()
            .id(&scrollable_body_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .overflow_x(Overflow::Auto)
            .overflow_y(Overflow::Hidden)
            .scrollable(true)
            .children(scrollable_rows);

        // Register scroll handlers on both bodies
        self.register_scroll_handlers(
            &frozen_body_id,
            &frozen_panel_id,
            registry,
            state,
            table_id,
            "frozen",
        );
        self.register_scroll_handlers(
            &scrollable_body_id,
            &scrollable_panel_id,
            registry,
            state,
            table_id,
            "scrollable",
        );

        // Register layout handler (only need one - they share scroll state)
        self.register_layout_handler(&scrollable_body_id, registry, state);

        // Note: Horizontal scroll sync for header would require tuidom support
        // for programmatic scroll positioning. For now, the scrollable header
        // and body scroll independently (horizontal scroll via overflow_x: Auto).

        // Build frozen panel (header + body)
        let mut frozen_panel_children = Vec::new();
        if let Some(h) = frozen_header {
            frozen_panel_children.push(h);
        }
        frozen_panel_children.push(frozen_body);

        let frozen_panel = Element::col()
            .id(&frozen_panel_id)
            .width(Size::Auto)
            .height(Size::Fill)
            .children(frozen_panel_children);

        // Build scrollable panel (header + body)
        let mut scrollable_panel_children = Vec::new();
        if let Some(h) = scrollable_header {
            scrollable_panel_children.push(h);
        }
        scrollable_panel_children.push(scrollable_body);

        let scrollable_panel = Element::col()
            .id(&scrollable_panel_id)
            .width(Size::Fill)
            .height(Size::Fill)
            .children(scrollable_panel_children);

        // Build main container
        let mut main_content = Element::row()
            .width(Size::Fill)
            .height(Size::Fill)
            .child(frozen_panel)
            .child(scrollable_panel);

        if let Some(ref style) = self.style {
            main_content = main_content.style(style.clone());
        }
        if let Some(ref transitions) = self.transitions {
            main_content = main_content.transitions(transitions.clone());
        }

        // Add scrollbar if needed
        let show_scrollbar = self.show_scrollbar && current.scroll.can_scroll();
        if show_scrollbar {
            let scrollbar_id = format!("{}-scrollbar", table_id);
            let scrollbar = Scrollbar::vertical()
                .id(&scrollbar_id)
                .scroll_state(&current.scroll)
                .build();

            register_scrollbar_handlers(&scrollbar_id, registry, state);

            Element::row()
                .id(table_id)
                .width(Size::Fill)
                .height(Size::Fill)
                .child(main_content)
                .child(scrollbar)
        } else {
            main_content.id(table_id)
        }
    }

    /// Build a header row for the given columns.
    fn build_header_row(&self, columns: &[Column], header_id: &str) -> Element {
        let mut row = Element::row().id(header_id).width(Size::Auto);

        for col in columns {
            let cell_width = match &col.width {
                ColumnWidth::Fixed(w) => Size::Fixed(*w),
                ColumnWidth::Flex(w) => Size::Flex(*w),
                ColumnWidth::Auto => Size::Flex(1), // Treat Auto as Flex(1) for now
            };

            let mut cell = Element::box_()
                .width(cell_width)
                .height(Size::Fixed(1))
                .flex_shrink(0) // Don't shrink fixed-width columns
                .child(Element::text(&col.header));

            if let Some(ref style) = self.header_style {
                cell = cell.style(style.clone());
            } else {
                cell = cell.style(
                    Style::new()
                        .background(Color::var("table.header_bg"))
                        .bold(),
                );
            }

            row = row.child(cell);
        }

        row
    }

    /// Build a data row for all columns.
    #[allow(clippy::too_many_arguments)]
    fn build_data_row(
        &self,
        row_data: &T,
        columns: &[Column],
        row_index: usize,
        pos_in_visible: usize,
        visible_count: usize,
        total_rows: usize,
        selection: &Selection<T::Key>,
        table_id: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<TableState<T>>,
    ) -> Element {
        self.build_data_row_for_columns(
            row_data,
            columns,
            row_index,
            pos_in_visible,
            visible_count,
            total_rows,
            selection,
            table_id,
            "row",
            registry,
            handlers,
            state,
        )
    }

    /// Build a data row for specific columns.
    #[allow(clippy::too_many_arguments)]
    fn build_data_row_for_columns(
        &self,
        row_data: &T,
        columns: &[Column],
        row_index: usize,
        pos_in_visible: usize,
        visible_count: usize,
        total_rows: usize,
        selection: &Selection<T::Key>,
        table_id: &str,
        row_type: &str,
        registry: &HandlerRegistry,
        handlers: &WidgetHandlers,
        state: &State<TableState<T>>,
    ) -> Element {
        let key = row_data.key();
        let row_id = format!("{}-{}-{}", table_id, row_type, key.to_string());
        let is_selected = selection.is_selected(&key);
        // Check if this row has "fake" focus (both sides show focus based on focused_key)
        let current_state = state.get();
        let has_fake_focus = current_state
            .focused_key
            .as_ref()
            .map(|fk| fk == &key)
            .unwrap_or(false);

        // Check boundary conditions
        let is_at_top_boundary = pos_in_visible == 0 && row_index > 0;
        let is_at_bottom_boundary =
            pos_in_visible == visible_count - 1 && row_index < total_rows - 1;

        // Build cells - both sides are focusable, focus styling is based on focused_key
        let mut row = Element::row()
            .id(&row_id)
            .width(Size::Auto)
            .focusable(true)
            .clickable(true);

        for col in columns {
            let cell_width = match &col.width {
                ColumnWidth::Fixed(w) => Size::Fixed(*w),
                ColumnWidth::Flex(w) => Size::Flex(*w),
                ColumnWidth::Auto => Size::Flex(1),
            };

            let mut cell = Element::box_()
                .width(cell_width)
                .height(Size::Fixed(row_data.height()))
                .flex_shrink(0) // Don't shrink fixed-width columns
                .child(row_data.cell(&col.id));

            if let Some(ref style) = self.cell_style {
                cell = cell.style(style.clone());
            }

            row = row.child(cell);
        }

        // Apply base row style
        if let Some(ref style) = self.row_style {
            row = row.style(style.clone());
        }

        // Apply selected style
        if is_selected {
            if let Some(ref style) = self.row_style_selected {
                row = row.style(style.clone());
            } else {
                row = row.style(
                    Style::new()
                        .background(Color::var("table.row_selected"))
                        .foreground(Color::var("text.inverted")),
                );
            }
        }

        // Apply focused style based on focused_key (both sides use fake focus)
        if has_fake_focus {
            if let Some(ref style) = self.row_style_focused {
                row = row.style(style.clone());
            } else {
                row = row.style(
                    Style::new()
                        .background(Color::var("table.row_focused"))
                        .foreground(Color::var("text.inverted")),
                );
            }
        }

        // Set explicit height
        row = row.height(Size::Fixed(row_data.height()));

        // Register boundary scroll handlers only (like List does)
        // Default focus navigation handles non-boundary movement
        if is_at_top_boundary {
            let state_clone = state.clone();
            let table_id_clone = table_id.to_string();
            let row_type_clone = row_type.to_string();
            let target_index = row_index.saturating_sub(1);
            registry.register(
                &row_id,
                "on_key_up",
                Arc::new(move |hx| {
                    let target_key = {
                        let current = state_clone.get();
                        current.rows.get(target_index).map(|r| r.key())
                    };
                    if let Some(key) = target_key {
                        // Update focused_key and scroll
                        state_clone.update(|s| {
                            s.scroll.apply_request(ScrollRequest::Delta(-1));
                            s.focused_key = Some(key.clone());
                        });
                        let row_id =
                            format!("{}-{}-{}", table_id_clone, row_type_clone, key.to_string());
                        hx.cx().focus(&row_id);
                    }
                }),
            );
        }

        if is_at_bottom_boundary {
            let state_clone = state.clone();
            let table_id_clone = table_id.to_string();
            let row_type_clone = row_type.to_string();
            let target_index = row_index + 1;
            registry.register(
                &row_id,
                "on_key_down",
                Arc::new(move |hx| {
                    let target_key = {
                        let current = state_clone.get();
                        current.rows.get(target_index).map(|r| r.key())
                    };
                    if let Some(key) = target_key {
                        // Update focused_key and scroll
                        state_clone.update(|s| {
                            s.scroll.apply_request(ScrollRequest::Delta(1));
                            s.focused_key = Some(key.clone());
                        });
                        let row_id =
                            format!("{}-{}-{}", table_id_clone, row_type_clone, key.to_string());
                        hx.cx().focus(&row_id);
                    }
                }),
            );
        }

        // Register activation handler
        {
            let state_clone = state.clone();
            let key_clone = key.clone();
            let on_select = handlers.get("on_select").cloned();
            let on_activate = handlers.get("on_activate").cloned();

            registry.register(
                &row_id,
                "on_activate",
                Arc::new(move |hx| {
                    state_clone.update(|s| {
                        s.last_activated = Some(key_clone.clone());
                        s.selection.toggle(key_clone.clone());
                    });
                    if let Some(ref handler) = on_select {
                        handler(hx);
                    }
                    if let Some(ref handler) = on_activate {
                        handler(hx);
                    }
                }),
            );
        }

        // Register focus/blur handlers to track focused_key (both sides)
        {
            // on_focus: update focused_key
            let state_clone = state.clone();
            let key_clone = key.clone();
            registry.register(
                &row_id,
                "on_focus",
                Arc::new(move |_hx| {
                    state_clone.update(|s| {
                        s.focused_key = Some(key_clone.clone());
                    });
                }),
            );

            // on_blur: clear focused_key
            let state_clone = state.clone();
            let key_clone = key.clone();
            registry.register(
                &row_id,
                "on_blur",
                Arc::new(move |_hx| {
                    state_clone.update(|s| {
                        // Only clear if this row was the focused one
                        if s.focused_key.as_ref() == Some(&key_clone) {
                            s.focused_key = None;
                        }
                    });
                }),
            );
        }

        row
    }

    /// Register scroll handlers for the body.
    fn register_scroll_handlers(
        &self,
        body_id: &str,
        _content_id: &str,
        registry: &HandlerRegistry,
        state: &State<TableState<T>>,
        table_id: &str,
        row_type: &str,
    ) {
        let state_clone = state.clone();
        let table_id_clone = table_id.to_string();
        let row_type_clone = row_type.to_string();

        registry.register(
            body_id,
            "on_scroll",
            Arc::new(move |hx| {
                // Mouse wheel: delta
                if let Some((_, delta_y)) = hx.event().scroll_delta() {
                    state_clone.update(|s| {
                        s.scroll.scroll_by(delta_y);
                    });
                }
                // Page Up/Down/Home/End
                if let Some(action) = hx.event().scroll_action() {
                    let current = state_clone.get();
                    if current.rows.is_empty() {
                        return;
                    }

                    let target_index = match action {
                        tuidom::ScrollAction::Home => 0,
                        tuidom::ScrollAction::End => current.rows.len() - 1,
                        tuidom::ScrollAction::PageUp => {
                            // Move up by viewport size
                            current.first_visible_index().saturating_sub(current.scroll.viewport as usize)
                        }
                        tuidom::ScrollAction::PageDown => {
                            let first = current.first_visible_index();
                            let viewport = current.scroll.viewport as usize;
                            (first + viewport).min(current.rows.len() - 1)
                        }
                    };

                    let target_key = current.rows.get(target_index).map(|r| r.key());
                    drop(current);

                    if let Some(key) = target_key {
                        // Update focused_key and scroll
                        state_clone.update(|s| {
                            let scroll_request = match action {
                                tuidom::ScrollAction::PageUp => ScrollRequest::PageUp,
                                tuidom::ScrollAction::PageDown => ScrollRequest::PageDown,
                                tuidom::ScrollAction::Home => ScrollRequest::Home,
                                tuidom::ScrollAction::End => ScrollRequest::End,
                            };
                            s.scroll.apply_request(scroll_request);
                            s.focused_key = Some(key.clone());
                        });

                        let row_id = format!("{}-{}-{}", table_id_clone, row_type_clone, key.to_string());
                        hx.cx().focus(&row_id);
                    }
                }
            }),
        );
    }

    /// Register layout handler for viewport discovery.
    fn register_layout_handler(
        &self,
        body_id: &str,
        registry: &HandlerRegistry,
        state: &State<TableState<T>>,
    ) {
        let state_clone = state.clone();
        registry.register(
            body_id,
            "on_layout",
            Arc::new(move |hx| {
                if let Some((_, _, _, height)) = hx.event().layout() {
                    state_clone.update(|s| {
                        s.scroll.set_viewport(height);
                    });
                }
            }),
        );
    }
}
