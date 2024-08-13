use crate::{
    derive::{DaDerive, InstantDerive},
    l1::PayloadAttribute,
    l2::Engine,
};

pub trait Runner<E: Engine, ID: InstantDerive, DD: DaDerive<P>, P: PayloadAttribute> {
    fn register_instant(&mut self, derive: ID);

    fn register_da(&mut self, derive: DD);

    fn get_engine(&self) -> &E;

    fn advance(&mut self);
}
