use std::path::Path;

use anyhow::Result;
use igloo_interface::{
    derive::{DaDerive, InstantDerive},
    l1::{Epoch, L1BlockInfo, L1Head},
    l2::{EngineApi, L2Head},
    runner::Runner,
};
use tokio::sync::mpsc::Sender;

use crate::{
    derive::{da::DaDeriveImpl, instant::InstantDeriveImpl},
    l1::{attribute::PayloadAttributeImpl, head::L1HeadImpl},
    l2::{block::BlockPayloadImpl, engine::SvmEngine, head::L2HeadImpl},
};

pub struct SimpleRunner {
    engine: SvmEngine,
    instant_derive: Option<InstantDeriveImpl>,
    da_derive: Option<DaDeriveImpl>,
    current_head: Option<L1HeadImpl>,
    sequence_number: u8,
}

impl Runner<SvmEngine, InstantDeriveImpl, DaDeriveImpl> for SimpleRunner {
    type Error = anyhow::Error;

    fn register_instant(&mut self, derive: InstantDeriveImpl) {
        self.instant_derive = Some(derive);
    }

    fn register_da(&mut self, derive: DaDeriveImpl) {
        self.da_derive = Some(derive);
    }

    fn get_engine(&self) -> &SvmEngine {
        &self.engine
    }

    async fn advance(&mut self) -> Result<(), Self::Error> {
        self.advance_safe().await?;
        self.advance_unsafe().await
    }
}

impl SimpleRunner {
    pub fn new(
        base_path: &Path,
        attribute_sender: Sender<PayloadAttributeImpl>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            engine: SvmEngine::new(base_path, attribute_sender)?,
            instant_derive: None,
            da_derive: None,
            current_head: None,
            sequence_number: 0,
        })
    }

    async fn advance_unsafe(&mut self) -> Result<()> {
        let info = self.instant_derive()?.get_new_block().await?;
        let block = if let Some(i) = info {
            self.current_head = Some(i.l1_head().clone());
            self.sequence_number = 0;

            self.engine.produce_block(i.try_into()?).await?
        } else if let Some(safe_head) = self.current_head.as_ref() {
            self.sequence_number += 1;

            let mut attribute: PayloadAttributeImpl = safe_head.clone().try_into()?;
            attribute.sequence_number = self.sequence_number;
            self.engine.produce_block(attribute).await?
        } else {
            return Ok(());
        };

        self.new_block(block).await?;
        Ok(())
    }

    #[allow(clippy::unnecessary_fallible_conversions)]
    async fn new_block(&mut self, block: BlockPayloadImpl) -> Result<L2HeadImpl> {
        let new_head = self.engine.new_block(block.try_into()?).await?;
        info!(
            "new block at: {}, derive from: {}",
            new_head.block_height(),
            self.current_head
                .as_ref()
                .ok_or(anyhow::anyhow!("No current head"))?
                .block_height()
        );
        Ok(new_head)
    }

    async fn advance_safe(&mut self) -> Result<()> {
        trace!("begin of da derive");
        while let Some(attribute) = self.da_derive()?.next().await {
            if self.has_executed(&attribute) {
                debug!(
                    "skip executed attribute at L1 height {} sequence number {}",
                    attribute.epoch.block_height(),
                    attribute.sequence_number
                );
                continue;
            }

            let block = self.engine.produce_block(attribute).await?;
            self.new_block(block).await?;
        }
        trace!("end of da derive");
        Ok(())
    }

    fn has_executed(&self, attribute: &PayloadAttributeImpl) -> bool {
        if let Some(head) = self.current_head.as_ref() {
            if head.block_height() > attribute.epoch.block_height() {
                return true;
            }
            return self.sequence_number >= attribute.sequence_number;
        }
        false
    }

    fn instant_derive(&mut self) -> Result<&mut InstantDeriveImpl> {
        self.instant_derive
            .as_mut()
            .ok_or(anyhow::anyhow!("Instant derive not registered"))
    }

    fn da_derive(&mut self) -> Result<&mut DaDeriveImpl> {
        self.da_derive
            .as_mut()
            .ok_or(anyhow::anyhow!("DA derive not registered"))
    }
}
