use std::sync::Arc;

use crate::l2;

/// L2 epoch according to L1 block info.
pub trait Epoch {
    type Hash;
    type BlockHeight;
    type Timestamp;

    fn block_hash(&self) -> Self::Hash;

    fn block_height(&self) -> Self::BlockHeight;

    fn timestamp(&self) -> Self::Timestamp;
}

pub trait PayloadAttribute {
    type Transaction: l2::Transaction;
    type Epoch: Epoch;
    type SequenceNumber: Copy;

    fn transactions(&self) -> Arc<Vec<Self::Transaction>>;

    fn epoch_info(&self) -> &Self::Epoch;

    fn sequence_number(&self) -> Self::SequenceNumber;
}
