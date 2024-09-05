use igloo_interface::l1::L1Head;

use super::{L1Hash, L1Height, L1Timestamp};

#[derive(Debug, Clone)]
pub struct L1HeadImpl {
    pub hash: L1Hash,
    pub height: L1Height,
    pub timestamp: L1Timestamp,
}

impl L1Head for L1HeadImpl {
    type Hash = L1Hash;
    type BlockHeight = L1Height;
    type Timestamp = L1Timestamp;

    fn block_hash(&self) -> Self::Hash {
        self.hash
    }

    fn block_height(&self) -> Self::BlockHeight {
        self.height
    }

    fn timestamp(&self) -> Self::Timestamp {
        self.timestamp
    }
}
