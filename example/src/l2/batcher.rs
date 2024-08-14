use crate::l1::attribute::PayloadAttributeImpl;
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};

type ThreadSafeBlocks = Arc<RwLock<Vec<PayloadAttributeImpl>>>;

pub struct Batcher {
    cache: ThreadSafeBlocks,
    next_batch: ThreadSafeBlocks,
    da_sender: Sender<Vec<PayloadAttributeImpl>>,
}

impl Batcher {
    pub fn new(da_sender: Sender<Vec<PayloadAttributeImpl>>) -> Self {
        Self {
            cache: Arc::new(RwLock::new(Vec::new())),
            next_batch: Arc::new(RwLock::new(Vec::new())),
            da_sender,
        }
    }

    pub fn run(&self, receiver: Receiver<PayloadAttributeImpl>) {
        tokio::spawn(Self::receive_tx_loop(receiver, self.cache.clone()));
        tokio::spawn(Self::send_batch_loop(
            self.da_sender.clone(),
            self.cache.clone(),
            self.next_batch.clone(),
        ));
    }

    async fn receive_tx_loop(
        mut receiver: Receiver<PayloadAttributeImpl>,
        cache: ThreadSafeBlocks,
    ) {
        loop {
            let block = receiver.recv().await;
            if let Some(block) = block {
                cache.write().await.push(block);
            }
        }
    }

    async fn send_batch_loop(
        da_sender: Sender<Vec<PayloadAttributeImpl>>,
        cache: ThreadSafeBlocks,
        next_batch: ThreadSafeBlocks,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;

            let has_batch = { next_batch.read().await.len() > 0 };
            if has_batch {
                if let Err(e) = da_sender.send(next_batch.read().await.clone()).await {
                    error!("Failed to send batch: {}", e);
                }
            }

            let batch = { cache.write().await.drain(..).collect::<Vec<_>>() };
            *next_batch.write().await = batch;
        }
    }
}
