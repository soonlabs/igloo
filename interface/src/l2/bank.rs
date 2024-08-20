pub trait BankOperations {
    type Error: std::fmt::Display;
    type Pubkey;
    type AccountSharedData;

    fn insert_account(&mut self, key: Self::Pubkey, data: Self::AccountSharedData);

    fn deploy_program(&mut self, buffer: Vec<u8>) -> Result<Self::Pubkey, Self::Error>;

    fn set_clock(&mut self);

    fn bump(&mut self) -> Result<(), Self::Error>;
}

pub trait BankInfo {
    type Hash;
    type Pubkey;
    type Slot;

    fn last_blockhash(&self) -> Self::Hash;

    fn execution_slot(&self) -> Self::Slot;

    fn collector_id(&self) -> Self::Pubkey;
}

pub trait StorageOperations {}
