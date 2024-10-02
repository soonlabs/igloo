use solana_sdk::fee::FeeStructure;

#[derive(Debug, Clone)]
pub struct Settings {
    pub max_age: usize,
    pub switchs: Switchs,
    pub fee_structure: FeeStructure,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_age: 150, // set default max_age from solana
            switchs: Default::default(),
            fee_structure: FeeStructure::new(0.0000005, 0.0, vec![(1_400_000, 0.0)]),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Switchs {
    pub tx_sanity_check: bool,
    pub txs_conflict_check: bool,
}
