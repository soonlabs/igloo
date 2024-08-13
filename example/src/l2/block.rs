use rollups_interface::l2::{Block, BlockPayload, Entry};

use super::head::L2HeadImpl;

pub struct SimpleEntry {}

#[derive(Default)]
pub struct BlockPayloadImpl {
    pub head: L2HeadImpl,
    pub entries: Vec<SimpleEntry>,
}

impl BlockPayload for BlockPayloadImpl {
    type Entry = SimpleEntry;

    fn entries(&self) -> &[Self::Entry] {
        &self.entries
    }
}

impl Entry for SimpleEntry {}

pub struct BlockImpl {
    pub head: L2HeadImpl,
    // TODO: add extra fields
}

impl Block for BlockImpl {
    type Head = L2HeadImpl;

    fn head(&self) -> &Self::Head {
        &self.head
    }
}

impl From<BlockPayloadImpl> for BlockImpl {
    fn from(value: BlockPayloadImpl) -> Self {
        Self { head: value.head }
    }
}
