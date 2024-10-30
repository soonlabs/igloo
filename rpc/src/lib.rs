#![allow(dead_code)]
#![allow(clippy::result_large_err)]

#[macro_use]
extern crate log;

#[cfg(test)]
#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate solana_metrics;

mod account_resolver;
mod error;
mod filter;
mod parsed_token_accounts;

pub mod jsonrpc;
pub mod service;

use error::{Error, Result};
