use igloo_interface::l2::L2Head;

use super::{L2Hash, L2Height, L2Timestamp};

#[derive(Default, Debug, Clone)]
pub struct L2HeadImpl {
    pub hash: L2Hash,
    pub height: L2Height,
    pub timestamp: L2Timestamp,
}

impl L2Head for L2HeadImpl {
    type Hash = L2Hash;
    type BlockHeight = L2Height;
    type Timestamp = L2Timestamp;

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
