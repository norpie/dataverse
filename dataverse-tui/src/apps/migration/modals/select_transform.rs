//! Modal for selecting a transform type to add.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::List;
use rafter::widgets::ListItem;
use rafter::widgets::ListState;
use rafter::widgets::Text;
use tuidom::Color;
use tuidom::Element;
use tuidom::Style;

/// Available transform types for selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformType {
    // Value
    Copy,
    Constant,
    Guid,
    // String
    StringOps,
    Format,
    Replace,
    // Type Conversion
    Convert,
    ParseInt,
    ParseDecimal,
    ParseDate,
    // Data
    ValueMap,
    Math,
    // Control Flow
    Guard,
    Coalesce,
    Match,
    Find,
}

impl TransformType {
    fn label(&self) -> &'static str {
        match self {
            TransformType::Copy => "Copy",
            TransformType::Constant => "Constant",
            TransformType::Guid => "GUID",
            TransformType::StringOps => "String Operations",
            TransformType::Format => "Format",
            TransformType::Replace => "Replace",
            TransformType::Convert => "Convert",
            TransformType::ParseInt => "Parse Integer",
            TransformType::ParseDecimal => "Parse Decimal",
            TransformType::ParseDate => "Parse Date",
            TransformType::ValueMap => "Value Map",
            TransformType::Math => "Math",
            TransformType::Guard => "Guard",
            TransformType::Coalesce => "Coalesce",
            TransformType::Match => "Match",
            TransformType::Find => "Find",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            TransformType::Copy => "Copy value from source field or variable",
            TransformType::Constant => "Use a static constant value",
            TransformType::Guid => "Generate a new random GUID",
            TransformType::StringOps => "Apply string operations (trim, case, etc.)",
            TransformType::Format => "Format string with placeholders",
            TransformType::Replace => "Replace text patterns",
            TransformType::Convert => "Convert to different type",
            TransformType::ParseInt => "Parse string to integer",
            TransformType::ParseDecimal => "Parse string to decimal",
            TransformType::ParseDate => "Parse string to date",
            TransformType::ValueMap => "Map values using lookup table",
            TransformType::Math => "Perform mathematical operation",
            TransformType::Guard => "Conditional early exit",
            TransformType::Coalesce => "First non-null from fallback chains",
            TransformType::Match => "Pattern matching with branches",
            TransformType::Find => "Look up record in target environment",
        }
    }

    /// All transform types in display order with category headers.
    fn all_with_headers() -> Vec<SelectionItem> {
        vec![
            // Value category
            SelectionItem::Header("Value"),
            SelectionItem::Transform(TransformType::Copy),
            SelectionItem::Transform(TransformType::Constant),
            SelectionItem::Transform(TransformType::Guid),
            // String category
            SelectionItem::Header("String"),
            SelectionItem::Transform(TransformType::StringOps),
            SelectionItem::Transform(TransformType::Format),
            SelectionItem::Transform(TransformType::Replace),
            // Type Conversion category
            SelectionItem::Header("Type Conversion"),
            SelectionItem::Transform(TransformType::Convert),
            SelectionItem::Transform(TransformType::ParseInt),
            SelectionItem::Transform(TransformType::ParseDecimal),
            SelectionItem::Transform(TransformType::ParseDate),
            // Data category
            SelectionItem::Header("Data"),
            SelectionItem::Transform(TransformType::ValueMap),
            SelectionItem::Transform(TransformType::Math),
            // Control Flow category
            SelectionItem::Header("Control Flow"),
            SelectionItem::Transform(TransformType::Guard),
            SelectionItem::Transform(TransformType::Coalesce),
            SelectionItem::Transform(TransformType::Match),
            SelectionItem::Transform(TransformType::Find),
        ]
    }
}

/// List item that can be either a category header or a transform type.
#[derive(Debug, Clone)]
pub enum SelectionItem {
    Header(&'static str),
    Transform(TransformType),
}

impl SelectionItem {
    fn as_transform(&self) -> Option<TransformType> {
        match self {
            SelectionItem::Transform(t) => Some(*t),
            SelectionItem::Header(_) => None,
        }
    }

    fn is_header(&self) -> bool {
        matches!(self, SelectionItem::Header(_))
    }
}

impl ListItem for SelectionItem {
    type Key = String;

    fn key(&self) -> String {
        match self {
            SelectionItem::Header(label) => format!("header-{}", label),
            SelectionItem::Transform(t) => format!("transform-{}", t.label()),
        }
    }

    fn render(&self) -> Element {
        match self {
            SelectionItem::Header(label) => {
                Element::text(*label).style(Style::new().foreground(Color::var("muted")).bold())
            }
            SelectionItem::Transform(t) => Element::row()
                .child(Element::text("  "))
                .child(Element::text(t.label())),
        }
    }
}

/// Modal for selecting a transform type.
#[modal(size = Md)]
pub struct SelectTransformModal {
    list_state: ListState<SelectionItem>,
}

impl SelectTransformModal {
    /// Create a new select transform modal.
    pub fn new_modal() -> Self {
        let items = TransformType::all_with_headers();
        Self::new(ListState::new(items))
    }

    fn selected_transform(&self) -> Option<TransformType> {
        self.list_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key))
                .and_then(|item| item.as_transform())
        })
    }

    fn selected_item(&self) -> Option<SelectionItem> {
        self.list_state.with_ref(|s| {
            s.focused_key
                .as_ref()
                .and_then(|key| s.items.iter().find(|item| &item.key() == key).cloned())
        })
    }
}

#[modal_impl]
impl SelectTransformModal {
    fn default_result(&self) -> Option<TransformType> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<TransformType>>) {
        mx.focus("transform-list");
        // Select first actual transform (skip first category header)
        let items = self.list_state.with_ref(|s| s.items.clone());
        if let Some((idx, item)) = items.iter().enumerate().find(|(_, item)| !item.is_header()) {
            self.list_state.update(|s| {
                s.focused_key = Some(item.key());
            });
        }
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("enter", submit);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<TransformType>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<TransformType>>) {
        if let Some(transform_type) = self.selected_transform() {
            mx.close(Some(transform_type));
        }
        // If a category header is selected, do nothing
    }

    #[handler]
    async fn select_item(&self, mx: &ModalContext<Option<TransformType>>) {
        // Same as submit - activate selected item
        self.submit(mx).await;
    }

    fn element(&self) -> Element {
        let selected_transform = self.selected_transform();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Select Transform Type") style (bold, fg: interact)

                row (width: fill, height: fill, gap: 2) {
                    box_ (width: fill, height: fill) {
                        list (state: self.list_state, id: "transform-list", width: fill, height: fill)
                            on_activate: select_item()
                    }

                    column (width: fill, height: fill) {
                        if let Some(t) = selected_transform {
                            column (gap: 1) {
                                text (content: {t.label()}) style (bold, fg: interact)
                                text (content: {t.description()}) style (fg: muted)
                            }
                        } else {
                            text (content: "Select a transform type") style (fg: muted)
                        }
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn") on_activate: cancel()
                    button (label: "Add", hint: "enter", id: "add-btn", disabled: {selected_transform.is_none()}) on_activate: submit()
                }
            }
        }
    }
}
