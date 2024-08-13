use anyhow::Result;
use derive::{da::DaDeriveImpl, instant::InstantDeriveImpl};
use mock::chain::MockLayer1;
use rollups_interface::runner::Runner;
use runner::SimpleRunner;
use solana_sdk::{signature::Keypair, signer::Signer};
use tokio::sync::mpsc::channel;

mod derive;
mod l1;
mod l2;
mod mock;
mod runner;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let deposit_kp = Keypair::new();
    let deposit_from = deposit_kp.pubkey();

    let (instant_sender, instant_receiver) = channel(1024);
    let instanct_driver = InstantDeriveImpl::new(instant_receiver);

    let (da_sender, da_receiver) = channel(1);
    let da_driver = DaDeriveImpl::new(da_receiver);

    let mut runner = SimpleRunner::new();

    runner.register_instant(instanct_driver);
    runner.register_da(da_driver);

    MockLayer1::new(deposit_from, 1000, instant_sender).run();

    loop {
        if let Err(e) = runner.advance().await {
            // We should match the error type and panic accordingly in production code
            error!("Error: {}", e);
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}
