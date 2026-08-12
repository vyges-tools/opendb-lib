// SPDX-License-Identifier: Apache-2.0
//! The density-fill substrate: what is already on a layer, and the fill placed onto it.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

#[test]
fn placed_instances_report_the_metal_they_occupy() {
    // The reason this shim exists: a standard cell's pins and internal metal are non-fill area,
    // and nothing else in the bridge exposes them. Without it, fill lands on top of the design.
    let db = odb::open_db(FIXTURE).expect("read");
    let flat = odb::inst_shapes(&db).unwrap();
    assert_eq!(flat.len() % 5, 0, "five values per shape");
    assert!(!flat.is_empty(), "a placed design has instance metal");

    let mut layers = std::collections::BTreeSet::new();
    for s in flat.chunks(5) {
        let (layer, x0, y0, x1, y1) = (s[0], s[1], s[2], s[3], s[4]);
        assert!(x1 >= x0 && y1 >= y0, "a shape box is not inverted: {s:?}");
        layers.insert(layer);
    }
    assert!(layers.len() > 1, "cells occupy more than one layer");
}

#[test]
fn fills_can_be_created_counted_and_cleared() {
    let db = odb::open_db(FIXTURE).expect("read");
    assert_eq!(
        odb::num_fills(&db).unwrap(),
        0,
        "the fixture starts unfilled"
    );

    // A layer the technology really has — discovered, not guessed.
    let layer = odb::layer_name_by_number(&db, odb::inst_shapes(&db).unwrap()[0]);
    assert!(!layer.is_empty(), "the shape's layer resolves to a name");

    odb::fill_create(&db, false, 0, &layer, 0, 0, 100, 100).expect("created");
    odb::fill_create(&db, true, 2, &layer, 200, 0, 300, 100).expect("created with a mask");
    assert_eq!(odb::num_fills(&db).unwrap(), 2);

    assert_eq!(
        odb::clear_fills(&db).unwrap(),
        2,
        "clearing reports what it removed"
    );
    assert_eq!(odb::num_fills(&db).unwrap(), 0);

    // An unknown layer is an error, not a fill placed nowhere.
    assert!(odb::fill_create(&db, false, 0, "no_such_layer_xyz", 0, 0, 1, 1).is_err());
}

#[test]
fn special_wire_boxes_are_readable_and_carry_via_enclosures() {
    // Special wires are the power grid. Density fill that misses them fills over the PDN, and
    // nothing else in the bridge exposes them.
    let db = odb::open_db(FIXTURE).expect("read");
    let flat = odb::swire_boxes(&db).unwrap();
    assert_eq!(flat.len() % 5, 0, "five values per box");
    for b in flat.chunks(5) {
        assert!(b[3] >= b[1] && b[4] >= b[2], "a box is not inverted: {b:?}");
    }
}

#[test]
fn obstruction_geometry_is_readable_and_matches_the_count() {
    let db = odb::open_db(FIXTURE).expect("read");
    let boxes = odb::obstruction_boxes(&db).unwrap();
    assert_eq!(boxes.len() % 5, 0);
    // Every obstruction with a layer is reported; the count includes any without one.
    assert!(boxes.len() / 5 <= odb::num_obstructions(&db));

    odb::add_obstruction(
        &db,
        &odb::layer_name_by_number(&db, odb::inst_shapes(&db).unwrap()[0]),
        10,
        20,
        30,
        40,
    )
    .expect("added");
    let after = odb::obstruction_boxes(&db).unwrap();
    assert_eq!(after.len(), boxes.len() + 5);
    let last = &after[after.len() - 5..];
    assert_eq!(
        &last[1..],
        &[10, 20, 30, 40],
        "the box we just added, in order"
    );
}

#[test]
fn layers_can_be_enumerated_and_state_their_routing_direction() {
    // Direction decides how a fill shape is oriented and which axis the line-end spacing applies
    // to; nothing generated exposes it, and nothing enumerated layers at all.
    let db = odb::open_db(FIXTURE).expect("read");
    let n = odb::num_layers(&db).unwrap();
    assert!(n > 0, "the technology has layers");

    let names: Vec<String> = (0..n)
        .map(|i| odb::nth_layer_name(&db, i).unwrap())
        .collect();
    assert!(names.iter().all(|s| !s.is_empty()));
    assert!(
        odb::nth_layer_name(&db, n).unwrap().is_empty(),
        "out of range is empty"
    );

    let dirs: Vec<String> = names
        .iter()
        .map(|l| odb::layer_direction(&db, l).unwrap())
        .collect();
    assert!(
        dirs.iter().all(|d| !d.is_empty()),
        "every layer states a direction"
    );
    assert!(
        dirs.iter().any(|d| d == "HORIZONTAL") && dirs.iter().any(|d| d == "VERTICAL"),
        "a routing stack alternates: {dirs:?}"
    );
    assert!(odb::layer_direction(&db, "no_such_layer_xyz")
        .unwrap()
        .is_empty());
}
