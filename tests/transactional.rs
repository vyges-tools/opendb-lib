// SPDX-License-Identifier: Apache-2.0
//! Which operations are covered by the ECO journal, established by MEASUREMENT.
//!
//! 🔑 **The database is used with transaction semantics: a run that fails or refuses must leave it
//! exactly as it was found.** `eco_begin` / `eco_undo` provide that — but only for the operations
//! the journal records, and it does not record all of them.
//!
//! ⚠️ **This file is the authority for which is which.** Every write API carries a one-line marker
//! in its own documentation; those markers are only trustworthy because these tests check them
//! against the real database rather than against a reading of anything.
//!
//! The shape of the answer: **the netlist rolls back, the geometry does not.** Instances, nets and
//! their connectivity are journaled; blockages, fills, rows, obstructions and special wires are
//! not. An engine writing geometry has to undo it itself.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

/// Run `edit` inside an ECO, roll it back, and report whether `measure` returned to its start.
fn rolls_back(measure: impl Fn(&odb::OdbDb) -> i64, edit: impl Fn(&odb::OdbDb)) -> bool {
    let db = odb::open_db(FIXTURE).expect("read");
    let before = measure(&db);
    odb::eco_begin(&db).expect("begin");
    edit(&db);
    let during = measure(&db);
    assert_ne!(during, before, "the edit must actually change something, or this proves nothing");
    odb::eco_undo(&db).expect("undo");
    measure(&db) == before
}

// ------------------------------------------------------------------ journaled: the netlist

#[test]
fn moving_an_instance_rolls_back() {
    assert!(rolls_back(
        |db| odb::inst_x(db, &odb::nth_inst_name(db, 0)) as i64,
        |db| {
            let n = odb::nth_inst_name(db, 0);
            let x = odb::inst_x(db, &n);
            odb::set_inst_location(db, &n, x + 5000, odb::inst_y(db, &n)).unwrap();
        }
    ));
}

#[test]
fn reorienting_an_instance_rolls_back() {
    // Orientation is a string, so it is probed by mapping it to a number rather than by a
    // length -- a length probe would pass for R0 vs MY and prove nothing.
    let db = odb::open_db(FIXTURE).expect("read");
    let name = odb::nth_inst_name(&db, 0);
    let before = odb::inst_get_orient(&db, &name);
    let want = if before == "MY" { "R0" } else { "MY" };

    odb::eco_begin(&db).expect("begin");
    odb::set_inst_orient(&db, &name, want).expect("reoriented");
    assert_eq!(odb::inst_get_orient(&db, &name), want, "it really changed");
    odb::eco_undo(&db).expect("undo");

    assert_eq!(odb::inst_get_orient(&db, &name), before, "rolled back");
}

#[test]
fn creating_an_instance_rolls_back() {
    assert!(rolls_back(
        |db| odb::num_insts(db) as i64,
        |db| {
            let master = odb::first_master_name(db);
            odb::create_inst(db, &master, "vyges_eco_probe_inst").unwrap();
        }
    ));
}

#[test]
fn creating_a_net_rolls_back() {
    assert!(rolls_back(
        |db| odb::num_nets(db) as i64,
        |db| { odb::create_net(db, "vyges_eco_probe_net").unwrap(); }
    ));
}

// ------------------------------------------------------------------ NOT journaled: geometry

#[test]
fn a_blockage_does_not_roll_back() {
    // ⛔ Undo it by hand -- see `truncate_blockages`.
    assert!(!rolls_back(
        |db| odb::num_blockages(db).unwrap() as i64,
        |db| { odb::blockage_create(db, 10, 20, 30, 40, "", true).unwrap(); }
    ));
}

#[test]
fn a_fill_does_not_roll_back() {
    // ⛔ Density fill must clear its own work; `clear_fills` exists for exactly this.
    assert!(!rolls_back(
        |db| odb::num_fills(db).unwrap() as i64,
        |db| {
            let layer = odb::layer_name_by_number(db, odb::inst_shapes(db).unwrap()[0]);
            odb::fill_create(db, false, 0, &layer, 0, 0, 100, 100).unwrap();
        }
    ));
}

#[test]
fn an_obstruction_does_not_roll_back() {
    assert!(!rolls_back(
        |db| odb::num_obstructions(db) as i64,
        |db| {
            let layer = odb::layer_name_by_number(db, odb::inst_shapes(db).unwrap()[0]);
            odb::add_obstruction(db, &layer, 10, 20, 30, 40).unwrap();
        }
    ));
}

#[test]
fn a_row_does_not_roll_back() {
    // ⛔ Floorplan initialisation rebuilds rows rather than rolling them back.
    assert!(!rolls_back(
        |db| odb::num_rows(db).unwrap() as i64,
        |db| {
            let site = odb::nth_site_name(db, 0).unwrap();
            odb::row_create(db, "vyges_eco_probe_row", &site, 0, 0, "R0", "HORIZONTAL", 10, 200).unwrap();
        }
    ));
}

// ------------------------------------------------------------------ the rule, stated once

#[test]
fn the_boundary_is_netlist_versus_geometry() {
    // 🔑 The one-sentence summary the API markers rely on. If a future database version starts
    // journaling geometry, the "does not roll back" tests above fail -- which is good news to be
    // told about, and the manual cleanup in every engine can then go.
    let netlist_rolls_back = rolls_back(
        |db| odb::num_nets(db) as i64,
        |db| { odb::create_net(db, "vyges_eco_probe_net2").unwrap(); },
    );
    let geometry_rolls_back = rolls_back(
        |db| odb::num_blockages(db).unwrap() as i64,
        |db| { odb::blockage_create(db, 1, 2, 3, 4, "", false).unwrap(); },
    );
    assert!(netlist_rolls_back, "netlist edits are transactional");
    assert!(!geometry_rolls_back, "geometry edits are NOT -- undo them yourself");
}
