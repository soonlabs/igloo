use crate::l2::{pool::SharedPool, tx::L2Transaction};
use rollups_interface::l2::pool::TransactionPool;
use solana_sdk::{signature::Keypair, signer::Signer};

pub struct TxServer {
    pool: SharedPool,
}

impl TxServer {
    pub fn new(pool: SharedPool) -> Self {
        Self { pool }
    }

    pub fn run(&self) {
        let inner = self.pool.clone();
        tokio::spawn(async move {
            loop {
                let tx_count = rand::random::<u8>() % 5;
                let mut pool = inner.write().await;
                for _ in 0..tx_count {
                    let tx = Self::random_l2_tx();
                    pool.insert(tx).expect("Failed to insert tx");
                }
                drop(pool);

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    fn random_l2_tx() -> L2Transaction {
        let from_kp = Keypair::new();
        let to_kp = Keypair::new();
        L2Transaction {
            from: from_kp.pubkey(),
            to: to_kp.pubkey(),
            amount: rand::random::<u64>() % 100,
            calldata: vec![],
        }
    }
}
