use std::path::Path;

use crate::{config::GlobalConfig, RollupStorage};
use anyhow::Result;

#[test]
fn init_with_all_default_works() -> Result<()> {
    let mut store = RollupStorage::new(GlobalConfig::new_temp()?)?;
    store.init()?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    Ok(())
}

#[test]
fn init_with_given_config_works() -> Result<()> {
    println!(
        "current dir: {:?}",
        std::env::current_dir()?.as_os_str().to_str()
    );
    // TODO use premade genesis storage
    let mut store = RollupStorage::new(GlobalConfig::new(&Path::new(
        "/home/raindust_x/github/soon/soon/config/ledger",
    ))?)?;
    store.init()?;

    let (bank_height, store_height) = store.get_mixed_heights()?;
    assert_eq!(bank_height, 0);
    assert_eq!(store_height, Some(0));
    Ok(())
}
