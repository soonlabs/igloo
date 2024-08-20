use crate::l1::{L1BlockInfo, PayloadAttribute};

/// InstantDerive is a trait that can be implemented by a struct to derive
/// a new block from a L1 block instantly by logs (events).
pub trait InstantDerive {
    type P: PayloadAttribute;
    type L1Info: L1BlockInfo<Self::P>;
    type Error: std::fmt::Display;

    /// Try to derive a new block from the L1 block, return `None` if
    ///  there is no new block to derive.
    async fn get_new_block(&mut self) -> Result<Option<Self::L1Info>, Self::Error>;
}

/// DaDerive is a trait that can be implemented by a struct to derive blocks
///  from DA provider.
pub trait DaDerive {
    type Item: PayloadAttribute;

    /// Fetch next `PayloadAttribute` from DA provider. This method
    /// is similar to `Iterator::next` but in async manner.
    async fn next(&mut self) -> Option<Self::Item>;
}
