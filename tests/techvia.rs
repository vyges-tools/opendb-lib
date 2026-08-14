// SPDX-License-Identifier: Apache-2.0
//! Tech via geometry — the boxes a technology declares for a via it names.
//!
//! A `-fixed_vias` power-grid connect names vias outright (`VIA12`, `VIA23`) instead of deriving
//! them from a VIARULE GENERATE, and ASAP7's `noviarules` technology offers nothing else. The
//! whole of such a via is these boxes: cuts on a cut layer, metal on the two routing layers.
//!
//! ℹ️ Fixture and expectation are committed together and neither is regenerated at run time.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/asap7_tech.odb";

/// `(via, [(layer, x0, y0, x1, y1), ...])` exactly as the technology declares them.
///
/// ⚠️ **The order is the technology's**, and it matters: `DbTechVia` takes the LAST cut box as its
/// single cut, so a reordering changes the via that gets built.
const EXPECTED: &[(&str, &[(&str, i32, i32, i32, i32)])] = &[
    (
        "VIA12",
        &[
            ("M1", -9, -11, 9, 11),
            ("M2", -14, -9, 14, 9),
            ("V1", -9, -9, 9, 9),
        ],
    ),
    (
        "VIA23",
        &[
            ("M2", -14, -9, 14, 9),
            ("M3", -9, -14, 9, 14),
            ("V2", -9, -9, 9, 9),
        ],
    ),
    (
        "VIA34",
        &[
            ("M3", -9, -17, 9, 17),
            ("M4", -20, -12, 20, 12),
            ("V3", -9, -12, 9, 12),
        ],
    ),
];

#[test]
fn a_named_tech_via_reports_its_boxes() {
    let db = odb::open_db(FIXTURE).expect("read");
    for (via, want) in EXPECTED {
        let raw = odb::tech_via_boxes(&db, via).expect("boxes");
        let got: Vec<(String, i32, i32, i32, i32)> = raw
            .chunks(5)
            .map(|c| {
                (
                    odb::layer_name_by_number(&db, c[0] as i64),
                    c[1],
                    c[2],
                    c[3],
                    c[4],
                )
            })
            .collect();
        let want: Vec<(String, i32, i32, i32, i32)> = want
            .iter()
            .map(|(l, a, b, c, d)| (l.to_string(), *a, *b, *c, *d))
            .collect();
        assert_eq!(got, want, "{via}");
    }
}

#[test]
fn a_tech_via_names_its_two_routing_layers() {
    let db = odb::open_db(FIXTURE).expect("read");
    for (via, bottom, top) in [("VIA12", "M1", "M2"), ("VIA23", "M2", "M3")] {
        assert_eq!(odb::tech_via_layer(&db, via, "bottom").unwrap(), bottom);
        assert_eq!(odb::tech_via_layer(&db, via, "top").unwrap(), top);
    }
}

#[test]
fn a_via_the_technology_does_not_have_reports_nothing() {
    let db = odb::open_db(FIXTURE).expect("read");
    // ⚠️ Empty rather than an error: `pdn.tcl` looks a fixed-via name up as a tech via AND as a
    // generate rule, and either may legitimately come back empty.
    assert!(odb::tech_via_boxes(&db, "VIA_NO_SUCH_THING")
        .unwrap()
        .is_empty());
    assert!(odb::tech_via_layer(&db, "VIA_NO_SUCH_THING", "bottom")
        .unwrap()
        .is_empty());
}
