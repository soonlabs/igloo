use crate::{
    derive::{DaDerive, InstantDerive},
    l2::Engine,
};

pub trait Runner<E: Engine, ID: InstantDerive, DD: DaDerive> {
    type Error: std::fmt::Display;

    fn register_instant(&mut self, derive: ID);

    fn register_da(&mut self, derive: DD);

    fn get_engine(&self) -> &E;

    async fn advance(&mut self) -> Result<(), Self::Error>;
}
