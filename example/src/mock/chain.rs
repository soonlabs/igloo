use crate::l1::{block::L1BlockInfoImpl, head::L1HeadImpl, tx::DepositTx};
use chrono::Utc;
use solana_sdk::{signature::Keypair, signer::Signer};
use tokio::sync::mpsc::Sender;

pub struct MockLayer1 {
    sender: Sender<L1BlockInfoImpl>,
    start_height: u64,
}

impl MockLayer1 {
    pub fn new(start_height: u64, sender: Sender<L1BlockInfoImpl>) -> Self {
        Self {
            sender,
            start_height,
        }
    }

    pub fn run(&mut self) {
        let sender = self.sender.clone();
        let start = self.start_height;

        tokio::spawn(async move {
            let mut height = start;
            loop {
                height += 1;

                if let Err(e) = sender.send(Self::generate_block(height)).await {
                    error!("Error sending block: {}", e);
                }

                tokio::time::sleep(std::time::Duration::from_secs(12)).await;
            }
        });
    }

    fn generate_block(height: u64) -> L1BlockInfoImpl {
        L1BlockInfoImpl {
            l1_head: Self::random_head(height),
            batch: None,
            deposit_txs: Self::random_deposit_txs(),
        }
    }

    fn random_head(height: u64) -> L1HeadImpl {
        L1HeadImpl {
            height,
            hash: Default::default(),
            timestamp: Utc::now().timestamp() as u64,
        }
    }

    fn random_deposit_txs() -> Vec<DepositTx> {
        let rand_count = rand::random::<u64>() % 10;
        let mut rtn = vec![];
        for _ in 0..rand_count {
            rtn.push(Self::random_deposit_tx());
        }
        rtn
    }

    fn random_deposit_tx() -> DepositTx {
        let from_kp = Keypair::new();
        let to_kp = Keypair::new();
        DepositTx {
            from: from_kp.pubkey(),
            to: to_kp.pubkey(),
            amount: rand::random::<u64>() % 100,
            calldata: vec![],
        }
    }
}
