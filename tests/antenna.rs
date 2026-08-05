// SPDX-License-Identifier: Apache-2.0
//! Antenna inputs read off the routed database — the numerator (metal per routing layer)
//! and denominator (pin-model gate area) of the antenna ratio. Consumed by `vyges-ant`.
//!
//! The fixture is a routed `counter` (50 of 52 nets carry metal on met1/met2/met3), so these
//! exercise real router output rather than a synthetic wire. `probe_fixture_routing` exists to
//! keep that verifiable: if it ever reports zero routed nets, every assertion below has gone
//! vacuous and is no longer evidence of anything.

use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

/// Every net that reports a layer must report positive area and perimeter on it, and the
/// layers it reports must be exactly the ones it has metal on — no phantom entries.
#[test]
fn reported_layers_carry_real_metal() {
    let db = odb::open_db(FIXTURE).expect("read .odb");
    let mut routed = 0usize;
    for i in 0..odb::num_nets(&db) {
        let net = odb::nth_net_name(&db, i);
        let nl = odb::num_net_wire_layers(&db, &net);
        if nl == 0 {
            continue;
        }
        routed += 1;
        for k in 0..nl {
            let layer = odb::nth_net_wire_layer(&db, &net, k);
            assert!(!layer.is_empty(), "net {net} layer {k} has no name");
            let area = odb::net_wire_area_on_layer(&db, &net, &layer);
            let perim = odb::net_wire_perimeter_on_layer(&db, &net, &layer);
            assert!(area > 0, "net {net} lists {layer} but reports area {area}");
            assert!(perim > 0, "net {net} lists {layer} but reports perimeter {perim}");
            // A rectangle's perimeter and area are not independent: for any union of
            // axis-aligned rects with integer DBU sides, area <= (perimeter/4)^2. A gross
            // unit slip (DBU vs micron, or area/perimeter transposed) breaks this.
            let bound = (perim / 4) * (perim / 4);
            assert!(
                area <= bound,
                "net {net} {layer}: area {area} exceeds the perimeter bound {bound} — unit slip?"
            );
        }
    }
    assert!(
        routed >= 40,
        "fixture regressed: only {routed} routed nets, so this test proves little"
    );
}

/// The accessors are total: unknown names answer zero rather than panicking or throwing.
/// A checker sweeping every net must not die on a net it cannot resolve.
#[test]
fn unknown_names_answer_zero() {
    let db = odb::open_db(FIXTURE).expect("read .odb");
    assert_eq!(odb::num_net_wire_layers(&db, "no_such_net"), 0);
    assert_eq!(odb::net_wire_area_on_layer(&db, "no_such_net", "met1"), 0);
    assert_eq!(odb::net_wire_perimeter_on_layer(&db, "no_such_net", "met1"), 0);
    assert_eq!(odb::nth_net_wire_layer(&db, "no_such_net", 0), "");
    assert_eq!(odb::layer_thickness(&db, "no_such_layer"), 0);
    assert_eq!(odb::mterm_antenna_gate_area(&db, "no_such_master", "A"), 0.0);

    // A real net, asked for a layer it has no metal on, is also zero — not an error.
    let net = (0..odb::num_nets(&db))
        .map(|i| odb::nth_net_name(&db, i))
        .find(|n| odb::num_net_wire_layers(&db, n) > 0)
        .expect("some routed net");
    assert_eq!(odb::net_wire_area_on_layer(&db, &net, "no_such_layer"), 0);
}

/// Indexing past the end yields "", and the layer set is stable across calls — the
/// accessors re-walk the wire each time, so an unstable order would make
/// `nth_net_wire_layer` and `net_wire_area_on_layer` disagree about the same index.
#[test]
fn layer_indexing_is_stable_and_bounded() {
    let db = odb::open_db(FIXTURE).expect("read .odb");
    let net = (0..odb::num_nets(&db))
        .map(|i| odb::nth_net_name(&db, i))
        .find(|n| odb::num_net_wire_layers(&db, n) > 1)
        .expect("a net routed on more than one layer");
    let nl = odb::num_net_wire_layers(&db, &net);
    let first: Vec<String> = (0..nl).map(|k| odb::nth_net_wire_layer(&db, &net, k)).collect();
    let again: Vec<String> = (0..nl).map(|k| odb::nth_net_wire_layer(&db, &net, k)).collect();
    assert_eq!(first, again, "layer order is not stable across calls");
    assert_eq!(odb::nth_net_wire_layer(&db, &net, nl), "", "past-the-end must be empty");
}

/// Thickness is read from the LEF and is needed for the side-area ratio (PSR/CSR).
/// The fixture's sky130 layers state one; 0 means "not stated" and must not be read as
/// a zero-thickness layer.
#[test]
fn routing_layers_state_a_thickness() {
    let db = odb::open_db(FIXTURE).expect("read .odb");
    for l in ["met1", "met2", "met3"] {
        assert!(
            odb::layer_thickness(&db, l) > 0,
            "{l} reports no LEF thickness, so side-area ratios cannot be computed"
        );
    }
}

/// Probe, not an assertion: reports what the fixture carries. Guards the tests above from
/// silently becoming vacuous, and records whether the masters carry antenna pin models —
/// which decides whether the ratio's denominator is available from this fixture at all.
#[test]
fn probe_fixture_routing() {
    let db = odb::open_db(FIXTURE).expect("read .odb");
    let n = odb::num_nets(&db);
    let (mut routed, mut layers_seen) = (0usize, Vec::<String>::new());
    for i in 0..n {
        let net = odb::nth_net_name(&db, i);
        let nl = odb::num_net_wire_layers(&db, &net);
        if nl > 0 {
            routed += 1;
            for k in 0..nl {
                let l = odb::nth_net_wire_layer(&db, &net, k);
                if !layers_seen.contains(&l) {
                    layers_seen.push(l);
                }
            }
        }
    }
    layers_seen.sort();
    println!(
        "fixture: {n} nets, {routed} routed, total_wire_length={}, layers={:?}",
        odb::total_wire_length(&db),
        layers_seen
    );

    // Denominator availability: how many instance pins carry a gate area?
    let (mut pins, mut with_area) = (0usize, 0usize);
    for i in 0..odb::num_insts(&db) {
        let inst = odb::nth_inst_name(&db, i);
        let master = odb::inst_master(&db, &inst);
        for k in 0..odb::num_iterms(&db, &inst) {
            let pin = odb::nth_iterm_name(&db, &inst, k);
            pins += 1;
            if odb::mterm_antenna_gate_area(&db, &master, &pin) > 0.0 {
                with_area += 1;
            }
        }
    }
    println!("gate area: {with_area} of {pins} instance pins report one");
}
