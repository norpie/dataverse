//! Instance registry for managing running app instances.

use std::any::TypeId;
use std::collections::HashMap;

use super::any_instance::{AnyAppInstance, AppInstance};
use super::config::SpawnError;
use super::instance::{InstanceId, InstanceInfo};
use super::traits::App;

/// Registry managing all running app instances.
///
/// The registry tracks:
/// - All running instances (keyed by InstanceId)
/// - The currently focused instance
/// - Most recently used order for focus switching
/// - Instance counts per app type for max_instances enforcement
pub struct InstanceRegistry {
    /// All running instances.
    instances: HashMap<InstanceId, Box<dyn AnyAppInstance>>,

    /// Currently focused instance (if any).
    focused: Option<InstanceId>,

    /// Most recently used order (most recent first).
    /// Used for focus switching when an instance is closed.
    mru_order: Vec<InstanceId>,

    /// Count of instances per app type (for max_instances enforcement).
    instance_counts: HashMap<TypeId, usize>,
}

impl InstanceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            focused: None,
            mru_order: Vec::new(),
            instance_counts: HashMap::new(),
        }
    }

    /// Spawn a new app instance.
    ///
    /// Returns the new instance ID, or an error if max instances reached.
    pub fn spawn<A: App>(&mut self, app: A) -> Result<InstanceId, SpawnError> {
        let config = A::config();
        let type_id = TypeId::of::<A>();

        // Check max instances
        if let Some(max) = config.max_instances {
            let current = self.instance_counts.get(&type_id).copied().unwrap_or(0);
            if current >= max {
                return Err(SpawnError::MaxInstancesReached {
                    app_name: config.name,
                    max,
                });
            }
        }

        // Create the instance
        let instance = AppInstance::new(app);
        let id = instance.id();

        // Store it
        self.instances.insert(id, Box::new(instance));
        *self.instance_counts.entry(type_id).or_insert(0) += 1;

        // Add to MRU order (at the front since it's newest)
        self.mru_order.insert(0, id);

        Ok(id)
    }

    /// Close an instance.
    ///
    /// If `force` is false, respects `on_close_request` and `persistent` flag.
    /// Returns true if the instance was closed.
    pub fn close(&mut self, id: InstanceId, force: bool) -> bool {
        let Some(instance) = self.instances.get(&id) else {
            return false;
        };

        let config = instance.config();

        // Check if we can close
        if !force {
            // Persistent apps can't be force-closed
            if config.persistent {
                return false;
            }

            // TODO: Call on_close_request when we have AppContext available
            // For now, we'll handle this at a higher level
        }

        // Get type ID before removing
        let type_id = instance.type_id();

        // Remove from registry
        self.instances.remove(&id);

        // Update instance count
        if let Some(count) = self.instance_counts.get_mut(&type_id) {
            *count = count.saturating_sub(1);
        }

        // Remove from MRU order
        self.mru_order.retain(|&i| i != id);

        // If this was the focused instance, focus the MRU
        if self.focused == Some(id) {
            self.focused = None;
            self.focus_mru();
        }

        true
    }

    /// Focus an instance.
    ///
    /// Updates the MRU order. Returns false if instance doesn't exist.
    pub fn focus(&mut self, id: InstanceId) -> bool {
        if !self.instances.contains_key(&id) {
            return false;
        }

        // Update focused state on old focused instance
        if let Some(old_id) = self.focused {
            if old_id != id {
                if let Some(old_instance) = self.instances.get_mut(&old_id) {
                    old_instance.set_focused(false);
                }
            }
        }

        // Update focused state on new instance
        if let Some(instance) = self.instances.get_mut(&id) {
            instance.set_focused(true);
            instance.mark_focused();
        }

        self.focused = Some(id);

        // Move to front of MRU order
        self.mru_order.retain(|&i| i != id);
        self.mru_order.insert(0, id);

        true
    }

    /// Focus the most recently used instance.
    ///
    /// Used after closing an instance. Returns true if an instance was focused.
    pub fn focus_mru(&mut self) -> bool {
        // Find first valid instance in MRU order
        for &id in &self.mru_order {
            if self.instances.contains_key(&id) {
                return self.focus(id);
            }
        }
        false
    }

    /// Get the currently focused instance ID.
    pub fn focused(&self) -> Option<InstanceId> {
        self.focused
    }

    /// Get a reference to an instance.
    pub fn get(&self, id: InstanceId) -> Option<&(dyn AnyAppInstance + '_)> {
        self.instances.get(&id).map(|b| b.as_ref())
    }

    /// Get a mutable reference to an instance.
    pub fn get_mut(&mut self, id: InstanceId) -> Option<&mut (dyn AnyAppInstance + '_)> {
        match self.instances.get_mut(&id) {
            Some(b) => Some(b.as_mut()),
            None => None,
        }
    }

    /// Get a reference to the focused instance.
    pub fn focused_instance(&self) -> Option<&(dyn AnyAppInstance + '_)> {
        self.focused.and_then(|id| self.get(id))
    }

    /// Get a mutable reference to the focused instance.
    pub fn focused_instance_mut(&mut self) -> Option<&mut (dyn AnyAppInstance + '_)> {
        let focused_id = self.focused?;
        match self.instances.get_mut(&focused_id) {
            Some(b) => Some(b.as_mut()),
            None => None,
        }
    }

    /// List all running instances.
    pub fn instances(&self) -> Vec<InstanceInfo> {
        self.instances.values().map(|i| i.info()).collect()
    }

    /// List instances of a specific app type.
    pub fn instances_of<A: App>(&self) -> Vec<InstanceInfo> {
        let type_id = TypeId::of::<A>();
        self.instances
            .values()
            .filter(|i| i.type_id() == type_id)
            .map(|i| i.info())
            .collect()
    }

    /// Find the first instance of a specific app type.
    ///
    /// Useful for singleton apps.
    pub fn instance_of<A: App>(&self) -> Option<InstanceId> {
        let type_id = TypeId::of::<A>();
        self.instances
            .values()
            .find(|i| i.type_id() == type_id)
            .map(|i| i.id())
    }

    /// Get the number of instances of a specific app type.
    pub fn instance_count<A: App>(&self) -> usize {
        let type_id = TypeId::of::<A>();
        self.instance_counts.get(&type_id).copied().unwrap_or(0)
    }

    /// Get total number of running instances.
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Check if there are no running instances.
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Iterate over all instances.
    pub fn iter(&self) -> impl Iterator<Item = &dyn AnyAppInstance> {
        self.instances.values().map(|b| b.as_ref())
    }

    /// Iterate over all instances in MRU order.
    pub fn iter_mru(&self) -> impl Iterator<Item = &dyn AnyAppInstance> {
        self.mru_order
            .iter()
            .filter_map(|id| self.instances.get(id).map(|b| b.as_ref()))
    }
}

impl Default for InstanceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full tests would require a mock App implementation
    // which is complex due to the trait requirements.
    // Integration tests should be done at a higher level.
}
