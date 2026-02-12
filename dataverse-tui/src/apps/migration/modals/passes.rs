//! Modal for editing pass configuration for an entity mapping.

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::Button;
use rafter::widgets::Checkbox;
use rafter::widgets::Text;

/// Result of the passes modal - all 7 pass toggles.
#[derive(Debug, Clone)]
pub struct PassesResult {
    pub create_pass: bool,
    pub activate_pass: bool,
    pub update_pass: bool,
    pub delete_pass: bool,
    pub deactivate_pass: bool,
    pub associate_pass: bool,
    pub disassociate_pass: bool,
}

/// Modal for editing passes configuration.
#[modal(size = Md)]
pub struct PassesModal {
    #[state(skip)]
    entity_mapping_id: i64,
    /// Warning acknowledgment checkbox (not persisted).
    acknowledged: bool,
    create_pass: bool,
    activate_pass: bool,
    update_pass: bool,
    delete_pass: bool,
    deactivate_pass: bool,
    associate_pass: bool,
    disassociate_pass: bool,
}

impl PassesModal {
    /// Create a modal for editing passes.
    pub fn new_modal(
        entity_mapping_id: i64,
        create_pass: bool,
        activate_pass: bool,
        update_pass: bool,
        delete_pass: bool,
        deactivate_pass: bool,
        associate_pass: bool,
        disassociate_pass: bool,
    ) -> Self {
        Self::new(
            entity_mapping_id,
            false, // acknowledged starts false
            create_pass,
            activate_pass,
            update_pass,
            delete_pass,
            deactivate_pass,
            associate_pass,
            disassociate_pass,
        )
    }
}

#[modal_impl]
impl PassesModal {
    fn default_result(&self) -> Option<PassesResult> {
        None
    }

    #[on_start]
    async fn on_start(&self, mx: &ModalContext<Option<PassesResult>>) {
        mx.focus("acknowledged-checkbox");
    }

    #[keybinds]
    fn keybinds() {
        bind("escape", cancel);
    }

    #[handler]
    async fn cancel(&self, mx: &ModalContext<Option<PassesResult>>) {
        mx.close(None);
    }

    #[handler]
    async fn submit(&self, mx: &ModalContext<Option<PassesResult>>) {
        mx.close(Some(PassesResult {
            create_pass: self.create_pass.get(),
            activate_pass: self.activate_pass.get(),
            update_pass: self.update_pass.get(),
            delete_pass: self.delete_pass.get(),
            deactivate_pass: self.deactivate_pass.get(),
            associate_pass: self.associate_pass.get(),
            disassociate_pass: self.disassociate_pass.get(),
        }));
    }

    fn element(&self) -> Element {
        let acknowledged = self.acknowledged.get();

        page! {
            column (padding: (1, 2), gap: 1, width: fill, height: fill) style (bg: surface) {
                text (content: "Edit Passes") style (bold, fg: interact)

                text (content: "Warning") style (bold, fg: warning)
                text (content: "Modifying passes can cause inconsistent migrations or errors. Only enable passes you understand and need.") style (fg: muted)

                checkbox (
                    state: self.acknowledged,
                    id: "acknowledged-checkbox",
                    label: "I understand the risks"
                )

                if acknowledged {
                    column (gap: 1) style (bg: surface2, padding: 1) {
                        text (content: "Pass Configuration") style (fg: muted)

                        checkbox (state: self.create_pass, id: "create-pass", label: "Create Pass - Create new target records")
                        checkbox (state: self.activate_pass, id: "activate-pass", label: "Activate Pass - Reactivate inactive targets before update")
                        checkbox (state: self.update_pass, id: "update-pass", label: "Update Pass - Update existing target records")
                        checkbox (state: self.delete_pass, id: "delete-pass", label: "Delete Pass - Delete orphaned target records")
                        checkbox (state: self.deactivate_pass, id: "deactivate-pass", label: "Deactivate Pass - Set inactive state on records and orphans")
                        checkbox (state: self.associate_pass, id: "associate-pass", label: "Associate Pass - Create N:N relationships")
                        checkbox (state: self.disassociate_pass, id: "disassociate-pass", label: "Disassociate Pass - Remove N:N relationships")
                    }
                }

                row (width: fill, justify: between) {
                    button (label: "Cancel", hint: "esc", id: "cancel-btn")
                        on_activate: cancel()

                    button (label: "Save", id: "save-btn", disabled: {!acknowledged})
                        on_activate: submit()
                }
            }
        }
    }
}
