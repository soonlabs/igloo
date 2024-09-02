use igloo_interface::derive::InstantDerive;
use tokio::sync::mpsc::{error::TryRecvError, Receiver};

use crate::l1::{attribute::PayloadAttributeImpl, block::L1BlockInfoImpl};

pub struct InstantDeriveImpl {
    receiver: Receiver<L1BlockInfoImpl>,
}

impl InstantDerive for InstantDeriveImpl {
    type P = PayloadAttributeImpl;
    type L1Info = L1BlockInfoImpl;
    type Error = anyhow::Error;

    async fn get_new_block(&mut self) -> anyhow::Result<Option<Self::L1Info>> {
        match self.receiver.try_recv() {
            Ok(info) => Ok(Some(info)),
            Err(err) => match err {
                TryRecvError::Empty => Ok(None),
                TryRecvError::Disconnected => Err(anyhow::anyhow!("Receiver disconnected")),
            },
        }
    }
}

impl InstantDeriveImpl {
    pub fn new(receiver: Receiver<L1BlockInfoImpl>) -> Self {
        Self { receiver }
    }
}
