//! Modal for editing unmatched handling configuration for an entity mapping.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::RadioGroup;
use rafter::widgets::RadioState;
use rafter::widgets::Text;

use crate::apps::migration::types::NoMatchFallback;
use crate::apps::migration::types::OrphanStrategy;

/// Result of the unmatched handling modal.
#[derive(Debug, Clone)]
pub struct UnmatchedHandlingResult {
    pub no_match_fallback: NoMatchFallback,
    pub orphan_strategy: OrphanStrategy,
}

/// Modal for editing unmatched handling configuration.
#[modal(size = Md)]
pub struct UnmatchedHandlingModal {
    #[state(skip)]
    entity_mapping_id: i64,
    /// Source unmatched strategy.
    source_unmatched: RadioState<NoMatchFallback>,
    /// Target unmatched (orphan) strategy.
    target_unmatched: RadioState<OrphanStrategy>,
}

impl UnmatchedHandlingModal {
    /// Create a modal for editing unmatched handling.
    pub fn new_modal(
        entity_mapping_id: i64,
        no_match_fallback: NoMatchFallback,
        orphan_strategy: OrphanStrategy,
    ) -> Self {
        let source_state = RadioState::new([
            (NoMatchFallback::Error, "Error"),
            (NoMatchFallback::Create, "Create"),
            (NoMatchFallback::Ignore, "Ignore"),
        ])
        .with_value(no_match_fallback);

        let target_state = RadioState::new([
            (OrphanStrategy::Delete, "Delete"),
            (OrphanStrategy::Deactivate, "Deactivate"),
            (OrphanStrategy::Ignore, "Ignore"),
            (OrphanStrategy::Error, "Error"),
        ])
        .with_value(orphan_strategy);

        Self::new(entity_mapping_id, source_state, target_state)
    }
}

#[modal_impl]
impl UnmatchedHandlingModal {
    fn default_result(&self) -> Option<UnmatchedHandlingResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<UnmatchedHandlingResult>>) {
        mx.focus("source-unmatched");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<UnmatchedHandlingResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<UnmatchedHandlingResult>>) {
        let source = self.source_unmatched.with_ref(|s| s.value);
        let target = self.target_unmatched.with_ref(|s| s.value);

        // Both should have values since we initialize with_value()
        if let (Some(no_match_fallback), Some(orphan_strategy)) = (source, target) {
            mx.close(Some(UnmatchedHandlingResult {
                no_match_fallback,
                orphan_strategy,
            }));
        }
    }

    fn element(&self) -> Element {
        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Unmatched Handling") style (bold, fg: interact)

                column (gap: 1) style (bg: surface2, padding: 1) {
                    text (content: "Source Unmatched") style (fg: primary)
                    text (content: "Handles source records with no target.") style (fg: muted)

                    radio_group (state: self.source_unmatched, id: "source-unmatched")
                }

                column (gap: 1) style (bg: surface2, padding: 1) {
                    text (content: "Target Unmatched") style (fg: primary)
                    text (content: "Handles target records with no source.") style (fg: muted)

                    radio_group (state: self.target_unmatched, id: "target-unmatched")
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    button (label: "Save", id: "save-btn")
                        on_activate: submit()
                }
            }
        }
    }
}
