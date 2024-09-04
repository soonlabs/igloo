use igloo_interface::l2::Transaction;
use solana_sdk::pubkey::Pubkey;

use crate::l1::tx::DepositTx;

#[derive(Clone, Debug)]
pub struct L2Transaction {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub calldata: Vec<u8>,
}

impl Transaction for L2Transaction {
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

impl TryFrom<DepositTx> for L2Transaction {
    type Error = anyhow::Error;

    fn try_from(value: DepositTx) -> Result<Self, Self::Error> {
        Ok(Self {
            from: value.from,
            to: value.to,
            amount: value.amount,
            calldata: value.calldata,
        })
    }
}
