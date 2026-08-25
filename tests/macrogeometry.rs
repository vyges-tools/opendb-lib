// SPDX-License-Identifier: Apache-2.0
//! Master pin geometry per MPin, and the instance halo — the read side `mpl` needs to build a
//! macro's halo before it can decide whether the movable cells fit.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

fn insts(db: &odb::OdbDb) -> Vec<String> {
    (0..odb::num_block_get_insts(db)).map(|i| odb::nth_block_get_insts(db, i)).collect()
}

fn terms(db: &odb::OdbDb, master: &str) -> Vec<String> {
    (0..odb::num_master_get_m_terms(db, master))
        .map(|i| odb::nth_master_get_m_terms(db, master, i))
        .collect()
}

/// Every MPin of a terminal, concatenated in order.
fn by_pin(db: &odb::OdbDb, master: &str, term: &str) -> Vec<i32> {
    let n = odb::num_mterm_get_m_pins(db, master, term);
    (0..n).flat_map(|i| odb::mpin_boxes(db, master, term, i).unwrap()).collect()
}

#[test]
fn the_mpins_of_a_terminal_partition_its_flattened_pin_boxes() {
    // 🔑 The grouping is the whole point of this accessor: `mterm_pin_boxes` returns the same
    // rectangles with the MPin boundaries erased. If the two ever disagree, one of them is
    // dropping or reordering shapes — and the flattened form is the one already in use.
    let db = odb::open_db(FIXTURE).expect("read");
    let mut checked = 0;
    for master in insts(&db).iter().map(|i| odb::inst_get_master(&db, i)) {
        for term in terms(&db, &master) {
            let flat = odb::mterm_pin_boxes(&db, &master, &term).unwrap();
            assert_eq!(by_pin(&db, &master, &term), flat, "{master}/{term}");
            if !flat.is_empty() {
                checked += 1;
            }
        }
    }
    // ⛔ Without this the loop proves nothing: a fixture whose terminals all had zero shapes
    // would compare empty against empty and pass.
    assert!(checked > 0, "the fixture must have at least one terminal with pin shapes");
}

#[test]
fn an_out_of_range_mpin_is_empty_rather_than_the_first_one() {
    // ⚠️ The index walks a list; returning the first pin for an index past the end would make a
    // one-MPin terminal look like it had many, all identical.
    let db = odb::open_db(FIXTURE).expect("read");
    let inst = insts(&db).into_iter().next().expect("an instance");
    let master = odb::inst_get_master(&db, &inst);
    let term = terms(&db, &master).into_iter().next().expect("a terminal");
    let n = odb::num_mterm_get_m_pins(&db, &master, &term);
    assert!(!odb::mpin_boxes(&db, &master, &term, 0).unwrap().is_empty());
    assert!(odb::mpin_boxes(&db, &master, &term, n).unwrap().is_empty());
}

#[test]
fn an_instance_with_no_halo_reports_nothing_rather_than_zeroes() {
    // 🔑 Absent and `(0,0,0,0)` mean different things upstream: no halo falls back to the
    // command's base halo, while an explicit zero halo overrides it. Collapsing them would apply
    // the base halo to a macro the user deliberately gave none. The empty vector is how absence
    // crosses the bridge; the typed wrapper turns it into `None`.
    let db = odb::open_db(FIXTURE).expect("read");
    for inst in insts(&db) {
        assert!(odb::inst_halo(&db, &inst).unwrap().is_empty(), "{inst}");
    }
}
