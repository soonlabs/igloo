use {
    solana_sdk::{
        instruction::Instruction as SolanaInstruction,
        pubkey::Pubkey,
        system_instruction,
        transaction::{
            SanitizedTransaction as SolanaSanitizedTransaction, Transaction as SolanaTransaction,
        },
    },
    std::collections::HashSet,
};

/// A simple transaction. Transfers SOL from one account
/// to another.
///
/// A `None` value for `mint` represents native SOL.
pub struct MockTransaction {
    pub payer: Option<Pubkey>,
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

impl From<&MockTransaction> for SolanaInstruction {
    fn from(value: &MockTransaction) -> Self {
        let MockTransaction {
            payer: _,
            from,
            to,
            amount,
        } = value;
        system_instruction::transfer(from, to, *amount)
    }
}

impl From<&MockTransaction> for SolanaTransaction {
    fn from(value: &MockTransaction) -> Self {
        SolanaTransaction::new_with_payer(
            &[SolanaInstruction::from(value)],
            Some(&value.payer.unwrap_or(value.from)),
        )
    }
}

impl From<&MockTransaction> for SolanaSanitizedTransaction {
    fn from(value: &MockTransaction) -> Self {
        SolanaSanitizedTransaction::try_from_legacy_transaction(
            SolanaTransaction::from(value),
            &HashSet::new(),
        )
        .unwrap()
    }
}

/// Create a batch of Solana transactions, for the Solana SVM's transaction
/// processor, from a batch of PayTube instructions.
pub fn create_svm_transactions(
    transactions: &[MockTransaction],
) -> Vec<SolanaSanitizedTransaction> {
    transactions
        .iter()
        .map(SolanaSanitizedTransaction::from)
        .collect()
}
