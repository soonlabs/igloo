use anyhow::Result;
use rollups_interface::derive::DaDerive;
use tokio::sync::mpsc::Receiver;

use crate::l1::attribute::PayloadAttributeImpl;

pub struct DaDeriveImpl {
    receiver: Receiver<Vec<PayloadAttributeImpl>>,
    cached: Vec<PayloadAttributeImpl>,
}

impl DaDerive<PayloadAttributeImpl> for DaDeriveImpl {}

impl Iterator for DaDeriveImpl {
    type Item = PayloadAttributeImpl;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl DaDeriveImpl {
    pub fn new(receiver: Receiver<Vec<PayloadAttributeImpl>>) -> Self {
        Self {
            receiver,
            cached: vec![],
        }
    }

    pub async fn try_update(&mut self) -> Result<()> {
        let payloads = self.receiver.try_recv()?;
        self.cached.extend(payloads);
        Ok(())
    }
}
