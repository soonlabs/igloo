use igloo_interface::l2::executor::Config;
use solana_sdk::clock::Slot;

mod wrapper;

pub use wrapper::BankWrapper;

#[derive(Clone)]
pub struct WrapperConfig {
    pub previous_slot: Slot,
    pub latest_slot: Slot,
    pub mint_lamports: u64,
}

impl Default for WrapperConfig {
    fn default() -> Self {
        Self {
            previous_slot: 20,
            latest_slot: 30,
            mint_lamports: 10_000,
        }
    }
}

impl Config for WrapperConfig {}
