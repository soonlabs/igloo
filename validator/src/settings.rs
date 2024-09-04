#[derive(Debug, Clone)]
pub struct Settings {
    pub max_age: usize,
    pub switchs: Switchs,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_age: 150, // set default max_age from solana
            switchs: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Switchs {
    pub tx_sanity_check: bool,
    pub txs_conflict_check: bool,
}
