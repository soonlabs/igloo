use crate::l1::{block::L1BlockInfoImpl, head::L1HeadImpl, tx::DepositTx};
use chrono::Utc;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use tokio::sync::mpsc::Sender;

pub struct MockLayer1 {
    sender: Sender<L1BlockInfoImpl>,
    deposit_from: Pubkey,
    start_height: u64,
}

impl MockLayer1 {
    pub fn new(deposit_from: Pubkey, start_height: u64, sender: Sender<L1BlockInfoImpl>) -> Self {
        Self {
            sender,
            deposit_from,
            start_height,
        }
    }

    pub fn run(&mut self) {
        let sender = self.sender.clone();
        let from = self.deposit_from;
        let start = self.start_height;

        tokio::spawn(async move {
            let mut height = start;
            loop {
                height += 1;

                if let Err(e) = sender.send(Self::generate_block(height, from)).await {
                    error!("Error sending block: {}", e);
                }

                tokio::time::sleep(std::time::Duration::from_secs(12)).await;
            }
        });
    }

    fn generate_block(height: u64, deposit_from: Pubkey) -> L1BlockInfoImpl {
        L1BlockInfoImpl {
            l1_head: Self::random_head(height),
            batch: None,
            deposit_txs: Self::random_deposit_txs(deposit_from),
        }
    }

    fn random_head(height: u64) -> L1HeadImpl {
        L1HeadImpl {
            height,
            hash: Default::default(),
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    fn random_deposit_txs(from: Pubkey) -> Vec<DepositTx> {
        let rand_count = rand::random::<u64>() % 10;
        let mut rtn = vec![];
        for _ in 0..rand_count {
            rtn.push(Self::random_deposit_tx(from));
        }
        rtn
    }

    fn random_deposit_tx(from: Pubkey) -> DepositTx {
        let kp = Keypair::new();
        DepositTx {
            from,
            to: kp.pubkey(),
            amount: rand::random::<u64>() % 100,
            calldata: vec![],
        }
    }
}
