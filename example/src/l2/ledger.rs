use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use igloo_interface::l2::{Block, L2Head};
use tokio::sync::RwLock;

use super::{block::BlockImpl, head::L2HeadImpl, L2Hash, L2Height};

pub type SharedLedger = Arc<RwLock<MockLedger>>;

#[derive(Default)]
pub struct MockLedger {
    pub blocks: BTreeMap<L2Height, BlockImpl>,
    pub hash_map: HashMap<L2Hash, L2Height>,
}

impl MockLedger {
    pub fn new_block(&mut self, block: BlockImpl) -> L2HeadImpl {
        let head = block.head().clone();
        self.blocks.insert(head.block_height(), block);
        self.hash_map.insert(head.block_hash(), head.block_height());

        head
    }

    pub fn latest_height(&self) -> L2Height {
        self.blocks.last_key_value().map(|(k, _)| *k).unwrap_or(0)
    }
}
