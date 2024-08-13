use crate::error::Result;
use crate::l1::{L1BlockInfo, PayloadAttribute};

pub trait InstantDerive {
    type P: PayloadAttribute;
    type L1Info: L1BlockInfo<Self::P>;

    fn on_new_block_received(&self) -> Result<Self::L1Info>;
}

pub trait DaDerive<P: PayloadAttribute>: Iterator<Item = P> {}
