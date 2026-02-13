//! Modal for editing a Format transform.

use dataverse_lib::DataverseClient;
use log::debug;
use rafter::page;
use rafter::prelude::*;
use rafter::widgets::AutocompleteState;
use tuidom::Element;

use super::path_suggestions::PathSuggestionGenerator;
use super::path_suggestions::VariableInfo;

/// Modal for editing a Format transform's template.
#[modal(size = Md)]
pub struct FormatTransformModal {
    /// The source Dataverse client for metadata lookups.
    #[state(skip)]
    client: DataverseClient,
    /// The target Dataverse client for entity ref suggestions.
    #[state(skip)]
    target_client: DataverseClient,
    /// The source entity logical name.
    #[state(skip)]
    source_entity: String,
    /// Available variables with type info.
    #[state(skip)]
    variables: Vec<VariableInfo>,

    /// Autocomplete state for template input.
    autocomplete: AutocompleteState<String>,
}

impl FormatTransformModal {
    /// Create a new Format transform modal.
    pub fn new_modal(
        client: DataverseClient,
        target_client: DataverseClient,
        source_entity: String,
        variables: Vec<VariableInfo>,
        current_template: String,
    ) -> Self {
        let mut autocomplete = AutocompleteState::new(Vec::<(String, String)>::new());
        autocomplete.text = current_template;
        autocomplete.cursor = autocomplete.text.len();

        Self::new(
            client,
            target_client,
            source_entity,
            variables,
            autocomplete,
        )
    }

    fn current_template(&self) -> String {
        self.autocomplete.get().text.clone()
    }

    /// Find the current placeholder context for autocomplete.
    ///
    /// Returns `Some((prefix, path_so_far))` if cursor is inside or just after `{`.
    /// - `prefix` is everything before the current path alternative being typed
    /// - `path_so_far` is the current path being typed (after the last `??` if any)
    ///
    /// For coalesce syntax `{a ?? b ?? c`, prefix includes everything up to the last `??`
    /// separator so autocomplete suggestions are built for the current alternative.
    ///
    /// Returns `None` if not in a placeholder context.
    fn find_placeholder_context(text: &str) -> Option<(String, String)> {
        // Find the last unclosed `{`
        let mut brace_depth = 0;
        let mut last_open_brace = None;

        for (i, c) in text.char_indices() {
            match c {
                '{' => {
                    brace_depth += 1;
                    last_open_brace = Some(i);
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                        last_open_brace = None;
                    }
                }
                _ => {}
            }
        }

        if let Some(brace_pos) = last_open_brace {
            let inside = &text[brace_pos + 1..];

            // If there's a `??`, autocomplete applies to the last alternative
            if let Some(last_sep) = inside.rfind("??") {
                let after_sep = inside[last_sep + 2..].trim_start();
                // Prefix includes opening `{` + prior alternatives + last `??`
                let prefix = format!(
                    "{}{{{} ?? ",
                    &text[..brace_pos],
                    inside[..last_sep].trim_end()
                );
                Some((prefix, after_sep.to_string()))
            } else {
                // Prefix includes opening `{`
                let prefix = text[..=brace_pos].to_string();
                let path_so_far = inside.to_string();
                Some((prefix, path_so_far))
            }
        } else {
            None
        }
    }
}

#[modal_impl]
impl FormatTransformModal {
    fn default_result(&self) -> Option<String> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<String>>) {
        // Generate initial suggestions
        self.update_suggestions().await;
        mx.focus("template-autocomplete");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
        bind("ctrl+s", save);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<String>>) {
        mx.close(None);
    }

    #[handler]
    async fn save(&self, mx: &ModalContext<Option<String>>) {
        let template = self.current_template();
        if !template.is_empty() {
            mx.close(Some(template));
        }
    }

    #[handler]
    async fn on_template_change(&self, _cx: &AppContext) {
        debug!(
            "[FormatTransform] on_template_change triggered, template: {:?}",
            self.current_template()
        );
        self.update_suggestions().await;
    }

    async fn update_suggestions(&self) {
        let template = self.current_template();

        // Check if we're inside a placeholder
        let suggestions =
            if let Some((prefix, path_so_far)) = Self::find_placeholder_context(&template) {
                debug!(
                    "[FormatTransform] Inside placeholder. prefix={:?}, path_so_far={:?}",
                    prefix, path_so_far
                );

                // Generate path suggestions
                let generator = PathSuggestionGenerator::new(
                    self.client.clone(),
                    self.target_client.clone(),
                    self.source_entity.clone(),
                    self.variables.clone(),
                );
                let path_suggestions = generator.generate_suggestions(&path_so_far).await;

                // Transform suggestions: value = prefix + path + }
                // Prefix always includes the opening `{` (and prior coalesce alternatives if any)
                // Label must start with full_template so fuzzy filter matches when user types "{f..."
                path_suggestions
                    .into_iter()
                    .map(|(path_value, path_label)| {
                        let full_template = format!("{}{}}}", prefix, path_value);
                        // Extract type info from path_label (format: "fieldname (Type)")
                        let type_info = path_label
                            .rfind(" (")
                            .map(|i| &path_label[i..])
                            .unwrap_or("");
                        let label = format!("{}{}", full_template, type_info);
                        (full_template, label)
                    })
                    .collect()
            } else {
                debug!("[FormatTransform] Not inside placeholder, showing hint to start one");
                // Not in a placeholder - show hint suggestions
                // When user types `{`, they'll get real suggestions
                vec![(
                    format!("{}{{", template),
                    "Type { to start a placeholder".to_string(),
                )]
            };

        debug!(
            "[FormatTransform] Generated {} suggestions",
            suggestions.len()
        );

        self.autocomplete.update(|s| {
            s.options = suggestions;
            s.refilter();
        });
    }

    fn element(&self) -> Element {
        let template = self.current_template();
        let is_empty = template.trim().is_empty();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Format Transform") style (bold, fg: interact)

                column (gap: 0, width: fill) {
                    text (content: "Template") style (fg: muted)
                    autocomplete (
                        state: self.autocomplete,
                        id: "template-autocomplete",
                        placeholder: "e.g., {firstname} {lastname}",
                        width: fill
                    )
                        on_change: on_template_change()
                }

                // Help text
                column (gap: 0, width: fill) {
                    text (content: "Placeholder syntax:") style (fg: muted)
                    text (content: "  {field}         - source field") style (fg: muted)
                    text (content: "  {field.nested}  - nested lookup") style (fg: muted)
                    text (content: "  {$variable}     - user variable") style (fg: muted)
                    text (content: "  {#value}        - pipeline value") style (fg: muted)
                    text (content: "  {a ?? b ?? c}   - first non-null") style (fg: muted)
                }

                // Spacer
                box_ (height: fill) {}

                // Buttons
                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()
                    button (
                        label: "Save",
                        hint: "ctrl+s",
                        id: "save-btn",
                        disabled: {is_empty}
                    )
                        on_activate: save()
                }
            }
        }
    }
}
