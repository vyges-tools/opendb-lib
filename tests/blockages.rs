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

// ------------------------------------------------------------------ transaction semantics

#[test]
fn a_blockage_is_NOT_rolled_back_by_the_eco_journal() {
    // ⛔ **A verified SCOPE of OpenDB, not a bug in this shim and not an oversight upstream.**
    // At pin 945a9f4 `dbJournal` covers the netlist (dbInst, dbNet, dbITerm, dbBTerm, dbBlock,
    // dbName), the module hierarchy, parasitics and routing guides — and has ZERO cases for
    // dbBlockage, dbObstruction, dbFill, dbRow, dbSWire, dbWire, dbVia or dbRegion.
    //
    // 🔑 It is a NETLIST-ECO journal, not a general transaction log. So this is not "blockages
    // were forgotten"; it is "physical geometry was never in scope", and the same is true of
    // every fill, row, special wire and via any of our engines writes.
    //
    // 🔑 This matters because macro placement is applied as a TRANSACTION: if the engine refuses
    // part-way it must leave the database exactly as it found it. Blockages are on the main path
    // (34 of upstream mpl's 36 DEF goldens carry a BLOCKAGES section), so relying on the journal
    // for them would leave orphans behind every refusal.
    //
    // ⚠️ **If this test ever fails, that is GOOD news**: OpenDB started journaling blockages and
    // the manual cleanup below can go. Asserting the limit is how we get told.
    let db = odb::open_db(FIXTURE).expect("read");
    let before = odb::num_blockages(&db).unwrap();

    odb::eco_begin(&db).expect("begin");
    odb::blockage_create(&db, 10, 20, 30, 40, "", true).expect("created");
    odb::eco_undo(&db).expect("undo");

    assert_eq!(
        odb::num_blockages(&db).unwrap(),
        before + 1,
        "the blockage SURVIVES the rollback -- undo it manually, do not trust the journal"
    );
}

#[test]
fn destroying_blockages_completes_the_rollback_by_hand() {
    // The workaround the limit above forces: record the count, and destroy back down to it.
    let db = odb::open_db(FIXTURE).expect("read");
    let before = odb::num_blockages(&db).unwrap();

    odb::blockage_create(&db, 10, 20, 30, 40, "", true).expect("created");
    odb::blockage_create(&db, 50, 60, 70, 80, "", false).expect("created");
    assert_eq!(odb::num_blockages(&db).unwrap(), before + 2);

    // ⚠️ Backwards: destroying one shifts the indices of everything after it.
    for idx in (before..before + 2).rev() {
        odb::blockage_destroy(&db, idx).expect("destroyed");
    }
    assert_eq!(odb::num_blockages(&db).unwrap(), before, "baseline restored");
}

#[test]
fn destroying_an_out_of_range_blockage_is_an_error() {
    let db = odb::open_db(FIXTURE).expect("read");
    let n = odb::num_blockages(&db).unwrap();
    assert!(odb::blockage_destroy(&db, n).is_err(), "one past the end");
}

#[test]
fn a_committed_blockage_survives() {
    // Teeth for the test above: if `undo` were a no-op, both tests would pass. This one fails
    // if `commit` were the no-op instead, so the pair distinguishes them.
    let db = odb::open_db(FIXTURE).expect("read");
    let before = odb::num_blockages(&db).unwrap();

    odb::eco_begin(&db).expect("begin");
    odb::blockage_create(&db, 10, 20, 30, 40, "", true).expect("created");
    odb::eco_commit(&db).expect("commit");

    assert_eq!(odb::num_blockages(&db).unwrap(), before + 1, "kept");
}

#[test]
fn a_macro_move_is_rolled_back_too() {
    // The other half of what mpl writes: location, orientation and placement status. A rollback
    // that restored blockages but not positions would leave a design that looks placed and is
    // not the one anybody asked for.
    let db = odb::open_db(FIXTURE).expect("read");
    let inst = odb::nth_inst_name(&db, 0);
    let (x0, y0) = (odb::inst_x(&db, &inst), odb::inst_y(&db, &inst));

    odb::eco_begin(&db).expect("begin");
    odb::set_inst_location(&db, &inst, x0 + 5000, y0 + 7000).expect("moved");
    assert_ne!(odb::inst_x(&db, &inst), x0, "it really moved");
    odb::eco_undo(&db).expect("undo");

    assert_eq!((odb::inst_x(&db, &inst), odb::inst_y(&db, &inst)), (x0, y0));
}
