use assert_matches::assert_matches;
use solana_ledger::genesis_utils::GenesisConfigInfo;
use solana_program_runtime::loaded_programs::{BlockRelation, ForkGraph};
use solana_runtime::{
    accounts_background_service::AbsRequestSender, bank::Bank, bank_forks::BankForks,
    genesis_utils::create_genesis_config, installed_scheduler_pool::BankWithScheduler,
};
use solana_sdk::{clock::Slot, pubkey::Pubkey};
use std::sync::{Arc, RwLock};

use crate::ledger::find_ancestors;

fn extend_bank_forks(bank_forks: Arc<RwLock<BankForks>>, parent_child_pairs: &[(Slot, Slot)]) {
    for (parent, child) in parent_child_pairs.iter() {
        let parent: Arc<Bank> = bank_forks.read().unwrap().get(*parent).unwrap();
        bank_forks.write().unwrap().insert(Bank::new_from_parent(
            parent,
            &Pubkey::default(),
            *child,
        ));
    }
}

fn new_bank_forks() -> Arc<RwLock<BankForks>> {
    let GenesisConfigInfo { genesis_config, .. } = create_genesis_config(10_000);
    let bank = Bank::new_for_tests(&genesis_config);
    let bank_forks = BankForks::new_rw_arc(bank);

    let parent_child_pairs = vec![
        (0, 1),
        (1, 3),
        (3, 8),
        (0, 2),
        (2, 4),
        (4, 5),
        (5, 10),
        (4, 6),
        (6, 12),
    ];
    extend_bank_forks(bank_forks.clone(), &parent_child_pairs);

    // Fork graph created for the test
    //                   0
    //                 /   \
    //                1     2
    //                |     |
    //                3     4
    //                |     | \
    //                8     5  6
    //                      |   |
    //                      10  12
    {
        let forks = bank_forks.read().unwrap();
        assert_matches!(forks.relationship(0, 3), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(0, 10), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(0, 12), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(1, 3), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(2, 10), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(2, 12), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(4, 10), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(4, 12), BlockRelation::Ancestor);
        assert_matches!(forks.relationship(6, 10), BlockRelation::Unrelated);
        assert_matches!(forks.relationship(5, 12), BlockRelation::Unrelated);
        assert_matches!(forks.relationship(6, 12), BlockRelation::Ancestor);

        assert_matches!(forks.relationship(6, 2), BlockRelation::Descendant);
        assert_matches!(forks.relationship(10, 2), BlockRelation::Descendant);
        assert_matches!(forks.relationship(8, 3), BlockRelation::Descendant);
        assert_matches!(forks.relationship(6, 3), BlockRelation::Unrelated);
        assert_matches!(forks.relationship(12, 2), BlockRelation::Descendant);
        assert_matches!(forks.relationship(12, 1), BlockRelation::Unrelated);
        assert_matches!(forks.relationship(1, 2), BlockRelation::Unrelated);

        assert_matches!(forks.relationship(1, 13), BlockRelation::Unknown);
        assert_matches!(forks.relationship(13, 2), BlockRelation::Unknown);
    }

    bank_forks
}

fn assert_banks_equal(mut expect: Vec<u64>, removed: &[BankWithScheduler]) {
    let mut removed_slots = removed.iter().map(|b| b.slot()).collect::<Vec<_>>();
    removed_slots.sort();
    expect.sort();
    assert_eq!(removed_slots, expect);
}

#[test]
fn find_ancestors_works() {
    let bank_forks = new_bank_forks();
    let removed = {
        let mut forks = bank_forks.write().unwrap();
        forks
            .set_root(
                2,
                &AbsRequestSender::default(),
                Some(1), // highest confirmed root
            )
            .unwrap()
    };
    let ancestors = find_ancestors(2, Some(1), bank_forks.clone(), &removed);
    assert_banks_equal(vec![0, 1, 3, 8], &removed);
    assert_eq!(ancestors.len(), 1);
    assert!(ancestors.contains(&0));

    let bank_forks = new_bank_forks();
    let removed = {
        let mut forks = bank_forks.write().unwrap();
        forks.set_root(12, &Default::default(), None).unwrap()
    };
    let ancestors = find_ancestors(12, None, bank_forks.clone(), &removed);
    assert_banks_equal(vec![0, 1, 2, 3, 4, 5, 6, 8, 10], &removed);
    assert_eq!(ancestors.len(), 4);
    assert!(ancestors.contains(&6));
    assert!(ancestors.contains(&4));
    assert!(ancestors.contains(&2));
    assert!(ancestors.contains(&0));

    let bank_forks = new_bank_forks();
    let removed = {
        let mut forks = bank_forks.write().unwrap();
        forks.set_root(5, &Default::default(), Some(2)).unwrap()
    };
    let ancestors = find_ancestors(5, Some(2), bank_forks.clone(), &removed);
    assert_banks_equal(vec![0, 1, 3, 6, 8, 12], &removed);
    assert_eq!(ancestors.len(), 1);
    assert!(ancestors.contains(&0));
}
