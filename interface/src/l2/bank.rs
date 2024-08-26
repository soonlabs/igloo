pub trait BankOperations {
    type Error: std::fmt::Display;
    type Pubkey;
    type AccountSharedData;

    fn insert_account(
        &mut self,
        key: Self::Pubkey,
        data: Self::AccountSharedData,
    ) -> Result<(), Self::Error>;

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Result<Self::Pubkey, Self::Error>;

    fn set_clock(&mut self) -> Result<(), Self::Error>;

    fn bump(&mut self) -> Result<(), Self::Error>;
}

pub trait BankInfo {
    type Hash;
    type Pubkey;
    type Slot;
    type Error: std::fmt::Display;

    fn last_blockhash(&self) -> Self::Hash;

    fn execution_slot(&self) -> Self::Slot;

    fn collector_id(&self) -> Result<Self::Pubkey, Self::Error>;
}
