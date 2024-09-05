use igloo_interface::l1::BatchInfo;

use super::L1Hash;

pub struct Batch {}

impl BatchInfo for Batch {
    type Hash = L1Hash;

    fn root_hash(&self) -> Self::Hash {
        Default::default()
    }
}
