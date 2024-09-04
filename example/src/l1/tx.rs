use igloo_interface::l1::DepositTransaction;
use solana_sdk::pubkey::Pubkey;

pub struct DepositTx {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub calldata: Vec<u8>,
}

impl DepositTransaction for DepositTx {
    // use same type as l2 here
    type Address = Pubkey;
    type Amount = u64;

    fn from(&self) -> &Self::Address {
        &self.from
    }

    fn to(&self) -> &Self::Address {
        &self.to
    }

    fn amount(&self) -> Self::Amount {
        self.amount
    }

    fn calldata(&self) -> &[u8] {
        &self.calldata
    }
}
