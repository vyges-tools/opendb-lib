// SPDX-License-Identifier: Apache-2.0
//! Placement blockages — the write side `mpl` needs.
//!
//! 🔑 **Why this shim exists.** The binding generator emits by-name accessors for *instance*
//! methods; `dbBlockage::create` is a **static factory**, and the generator emits none of those.
//! The read side (`blockage_is_soft`, `blockage_get_instance`, …) was already generated, which is
//! exactly the asymmetry that made this look bridged when it was not.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

#[test]
fn blockages_can_be_created_counted_and_read_back() {
    let db = odb::open_db(FIXTURE).expect("read");
    let before = odb::num_blockages(&db).unwrap();

    // Upstream rule (mpl `HierRTLMP::commitMacroPlacementToDb`): create over the macro's box,
    // then `setSoft()`. Not associated with an instance here — that is the next test.
    odb::blockage_create(&db, 10, 20, 30, 40, "", true).expect("created");
    assert_eq!(odb::num_blockages(&db).unwrap(), before + 1);

    let boxes = odb::blockage_boxes(&db).unwrap();
    assert_eq!(boxes.len() % 4, 0, "four values per box");
    let last = &boxes[boxes.len() - 4..];
    assert_eq!(last, &[10, 20, 30, 40], "the box we just added, in order");

    // ⚠️ The DEF the mpl goldens compare carries `+ SOFT`, so the flag must actually land —
    // a blockage created hard would still count and still have the right box.
    let idx = odb::num_blockages(&db).unwrap() - 1;
    assert!(odb::blockage_is_soft(&db, idx), "soft round-trips");
}

#[test]
fn a_hard_blockage_is_distinguishable_from_a_soft_one() {
    // Teeth for the assertion above: if `soft` were ignored, this test fails.
    let db = odb::open_db(FIXTURE).expect("read");
    odb::blockage_create(&db, 0, 0, 5, 5, "", false).expect("created");
    let idx = odb::num_blockages(&db).unwrap() - 1;
    assert!(!odb::blockage_is_soft(&db, idx), "hard stays hard");
}

#[test]
fn a_blockage_can_be_associated_with_an_instance() {
    // Upstream passes the `dbInst` so the blockage is attributed to the macro it guards; the DEF
    // golden spells it `+ COMPONENT <inst>`. An unassociated blockage would still place metal
    // but would lose that attribution.
    let db = odb::open_db(FIXTURE).expect("read");
    let inst = odb::nth_inst_name(&db, 0);
    assert!(!inst.is_empty(), "the fixture has instances");

    odb::blockage_create(&db, 1, 2, 3, 4, &inst, true).expect("created");
    let idx = odb::num_blockages(&db).unwrap() - 1;
    assert_eq!(
        odb::blockage_get_instance(&db, idx),
        inst,
        "the association round-trips"
    );
}

#[test]
fn an_unknown_instance_is_an_error_not_a_silent_orphan() {
    // 🔑 The standing rule: an input that fails to arrive must ERROR, never quietly do something
    // else. A blockage created unassociated because the name did not resolve would place correct
    // metal and lose the attribution, and nothing downstream would say so.
    let db = odb::open_db(FIXTURE).expect("read");
    let before = odb::num_blockages(&db).unwrap();
    assert!(odb::blockage_create(&db, 1, 2, 3, 4, "no_such_inst_xyz", true).is_err());
    assert_eq!(
        odb::num_blockages(&db).unwrap(),
        before,
        "a refused create leaves nothing behind"
    );
}
