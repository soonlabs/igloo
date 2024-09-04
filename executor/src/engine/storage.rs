use igloo_storage::RollupStorage;

use super::Engine;

impl Engine {
    pub fn store(&self) -> &RollupStorage {
        &self.storage
    }
}
