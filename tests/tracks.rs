// SPDX-License-Identifier: Apache-2.0
//! Routing track coordinates — the foundation of pin placement.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/counter.odb";

#[test]
fn a_routing_layer_reports_the_tracks_pins_may_sit_on() {
    // Every legal pin slot is a track. `getGridX` fills a vector by reference, which the schema
    // generator cannot express, so nothing reached these before.
    let db = odb::open_db(FIXTURE).expect("read");
    let layers: Vec<String> = (0..odb::num_layers(&db).unwrap())
        .map(|i| odb::nth_layer_name(&db, i).unwrap())
        .collect();

    let mut with_tracks = 0;
    for l in &layers {
        let (xs, ys) = (
            odb::track_grid_x(&db, l).unwrap(),
            odb::track_grid_y(&db, l).unwrap(),
        );
        if xs.is_empty() && ys.is_empty() {
            continue;
        }
        with_tracks += 1;
        // Tracks are a grid: sorted, and evenly spaced within a pattern.
        for axis in [&xs, &ys] {
            if axis.len() < 3 {
                continue;
            }
            assert!(
                axis.windows(2).all(|w| w[1] > w[0]),
                "{l}: tracks are ascending"
            );
            let pitch = axis[1] - axis[0];
            assert!(pitch > 0, "{l}: a track pitch is positive");
        }
    }
    assert!(
        with_tracks > 0,
        "a routed design has track grids: {layers:?}"
    );
}

#[test]
fn a_layer_without_a_track_grid_reports_nothing_rather_than_failing() {
    // "No grid" is an answer — a cut layer has none — and must not read as an error.
    let db = odb::open_db(FIXTURE).expect("read");
    assert!(odb::track_grid_x(&db, "no_such_layer_xyz")
        .unwrap()
        .is_empty());
    assert!(odb::track_grid_y(&db, "no_such_layer_xyz")
        .unwrap()
        .is_empty());
}
