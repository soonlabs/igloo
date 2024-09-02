use crate::l2::{stream::SharedStream, tx::L2Transaction};
use igloo_interface::l2::stream::TransactionStream;
use solana_sdk::{signature::Keypair, signer::Signer};

pub struct TxServer {
    stream: SharedStream,
}

impl TxServer {
    pub fn new(stream: SharedStream) -> Self {
        Self { stream }
    }

    pub fn run(&self) {
        let inner = self.stream.clone();
        tokio::spawn(async move {
            loop {
                let tx_count = rand::random::<u8>() % 5;
                let mut stream = inner.write().await;
                for _ in 0..tx_count {
                    let tx = Self::random_l2_tx();
                    stream.insert(tx).await.expect("Failed to insert tx");
                }
                drop(stream);

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
