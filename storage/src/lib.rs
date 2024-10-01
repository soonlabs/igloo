pub mod accounts;
pub mod background;
pub mod blockstore;
pub mod config;
pub mod error;
pub mod events;
pub mod execution;
pub mod history;
pub mod impls;
pub mod init;
pub mod ledger;
pub mod sig_hub;
#[cfg(test)]
mod tests;

use solana_sdk::clock::Slot;
pub use {
    error::{Error, Result},
    impls::RollupStorage,
};

#[macro_use]
extern crate log;

pub trait BankInfo {
    type Hash;
    type Pubkey;
    type Slot;
    type Error: std::fmt::Display;

    fn last_blockhash(&self, slot: Option<Slot>) -> std::result::Result<Self::Hash, Self::Error>;

    fn execution_slot(&self) -> Self::Slot;

    fn collector_id(&self) -> std::result::Result<Self::Pubkey, Self::Error>;
}
