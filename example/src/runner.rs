use anyhow::Result;
use rollups_interface::{
    derive::InstantDerive,
    l1::{L1BlockInfo, L1Head},
    l2::{EngineApi, L2Head},
    runner::Runner,
};

use crate::{
    derive::{da::DaDeriveImpl, instant::InstantDeriveImpl},
    l1::{attribute::PayloadAttributeImpl, head::L1HeadImpl},
    l2::engine::SvmEngine,
};

pub struct SimpleRunner {
    engine: SvmEngine,
    instant_derive: Option<InstantDeriveImpl>,
    da_derive: Option<DaDeriveImpl>,
    current_head: Option<L1HeadImpl>,
}

impl Runner<SvmEngine, InstantDeriveImpl, DaDeriveImpl, PayloadAttributeImpl> for SimpleRunner {
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
        self.advance_safe().await
    }
}

impl SimpleRunner {
    pub fn new() -> Self {
        Self {
            engine: SvmEngine::new(),
            instant_derive: None,
            da_derive: None,
            current_head: None,
        }
    }

    async fn advance_safe(&mut self) -> Result<()> {
        let info = self.instant_derive()?.get_new_block()?;
        let block = if let Some(i) = info {
            self.current_head = Some(i.l1_head().clone());

            self.engine.produce_block(i.try_into()?).await?
        } else if let Some(safe_head) = self.current_head.as_ref() {
            self.engine
                .produce_block(safe_head.clone().try_into()?)
                .await?
        } else {
            return Ok(());
        };

        let safe_head = self.engine.new_block(block.try_into()?).await?;
        info!(
            "new block at: {}, derive from: {}",
            safe_head.block_height(),
            self.current_head
                .as_ref()
                .ok_or(anyhow::anyhow!("No current head"))?
                .block_height()
        );
        Ok(())
    }

    fn instant_derive(&mut self) -> Result<&mut InstantDeriveImpl> {
        self.instant_derive
            .as_mut()
            .ok_or(anyhow::anyhow!("Instant derive not registered"))
    }
}
