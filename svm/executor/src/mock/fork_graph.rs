use std::cmp::Ordering;

use solana_program_runtime::loaded_programs::{BlockRelation, ForkGraph};
use solana_sdk::clock::{Epoch, Slot};

#[derive(Default)]
pub struct MockForkGraph {}

impl ForkGraph for MockForkGraph {
    fn relationship(&self, a: Slot, b: Slot) -> BlockRelation {
        match a.cmp(&b) {
            Ordering::Less => BlockRelation::Ancestor,
            Ordering::Equal => BlockRelation::Equal,
            Ordering::Greater => BlockRelation::Descendant,
        }
    }

    fn slot_epoch(&self, _slot: Slot) -> Option<Epoch> {
        Some(0)
    }
}
