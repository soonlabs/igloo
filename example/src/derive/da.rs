use crate::l1::attribute::PayloadAttributeImpl;
use igloo_interface::derive::DaDerive;
use std::sync::Arc;
use tokio::sync::{mpsc::Receiver, RwLock};

#[derive(Clone, Default)]
pub struct DaDeriveImpl {
    cached: Arc<RwLock<Vec<PayloadAttributeImpl>>>,
}

impl DaDerive for DaDeriveImpl {
    type Item = PayloadAttributeImpl;

    async fn next(&mut self) -> Option<Self::Item> {
        self.cached.write().await.pop()
    }
}

impl DaDeriveImpl {
    pub fn run(&self, receiver: Receiver<Vec<PayloadAttributeImpl>>) {
        tokio::spawn(Self::try_update(self.cached.clone(), receiver));
    }

    pub async fn try_update(
        cached: Arc<RwLock<Vec<PayloadAttributeImpl>>>,
        mut receiver: Receiver<Vec<PayloadAttributeImpl>>,
    ) {
        loop {
            let payloads = receiver.recv().await;

            if let Some(payloads) = payloads {
                cached.write().await.extend(payloads);
            }
        }
    }
}
