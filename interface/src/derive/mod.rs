use crate::l1::{L1BlockInfo, PayloadAttribute};

pub trait InstantDerive {
    type P: PayloadAttribute;
    type L1Info: L1BlockInfo<Self::P>;
    type Error;

    fn get_new_block(&mut self) -> Result<Option<Self::L1Info>, Self::Error>;
}

pub trait DaDerive<P: PayloadAttribute>: Iterator<Item = P> {}
