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
