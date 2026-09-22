// SPDX-License-Identifier: Apache-2.0
//! Whether a net has a routed-wire object (`dbNet::getWire() != nullptr`) — a global router's
//! "has wires" flag, which keeps a pre-routed net out of the router.
//!
//! 🔑 The counts are **OpenROAD's own answers** on this fixture, through its Tcl API
//! (`[$net getWire] ne "NULL"` over `getNets`): 52 nets, 50 with a wire; `_00_` has one, the supply
//! net `VPWR` (special wiring only) does not.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

#[test]
fn net_has_wire_matches_the_reference() {
    let db = odb::open_db(FIXTURE).expect("fixture");
    let nets: Vec<String> = (0..odb::num_nets(&db)).map(|i| odb::nth_net_name(&db, i)).collect();
    assert_eq!(nets.len(), 52);
    assert_eq!(nets.iter().filter(|n| odb::net_has_wire(&db, n)).count(), 50);
    assert!(odb::net_has_wire(&db, "_00_"));
    assert!(!odb::net_has_wire(&db, "VPWR"));
    assert!(!odb::net_has_wire(&db, "no_such_net"));
}
