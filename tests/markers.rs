// SPDX-License-Identifier: Apache-2.0
//! Reading a top-level marker category with everything under it, as a detailed router reads its
//! "DRC" markers back from the database: every marker in `getAllMarkers` order, its layer and box,
//! and its NET sources only.
//!
//! 🔑 The fixture is the `tiny` design (LEF/DEF from vyges-tools/opendb tests) with four markers
//! imported by the reference's own `odb::dbMarkerCategory_fromTR` into category `DRC`
//! (subcategories `Short` and `Cut Short`, two markers each). The expected values are the
//! reference's answers on that database: each subcategory written back by its own `writeTR`, and
//! the ids it reports.
//! - A marker id is per category (1 and 2 in EACH subcategory), and `getAllMarkers` is a set
//!   ordered by id, then by owner, so the subcategories INTERLEAVE: M1, V1, M2, V1.
//! - `getSources` is a set ordered the same way: the third marker was imported with sources
//!   `net:n1 inst:u1 net:in`, and the reference writes it back as `inst:u1 net:n1` — instance `u1`
//!   and net `in` both have id 1 under the same block, so the set holds one of them. Read through
//!   libodb, ours drops `in` exactly as the reference does.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/tiny_drc_markers.odb";

#[test]
fn every_marker_of_a_category_and_its_subcategories_in_get_all_markers_order() {
    let db = odb::open_db(FIXTURE).expect("fixture");
    let layers = odb::marker_category_all_layers(&db, "DRC").expect("layers");
    assert_eq!(layers, ["M1", "V1", "M2", "V1"]);
    let recs = odb::marker_category_all_records(&db, "DRC").expect("records");
    assert_eq!(
        recs.chunks(6).map(|r| r.to_vec()).collect::<Vec<_>>(),
        vec![
            vec![1, 1000, 200, 1500, 400, 2],
            vec![1, 1100, 300, 1170, 370, 2],
            vec![1, 2000, 1000, 2300, 1140, 1],
            vec![1, 3000, 1500, 3070, 1570, 1],
        ]
    );
    // Net sources only, each marker's in getSources order; the instance source is left out (and on
    // the third marker it has displaced net `in`, as in the reference).
    let nets = odb::marker_category_all_source_nets(&db, "DRC").expect("nets");
    assert_eq!(nets, ["in", "n1", "in", "n1", "n1", "n1"]);
}

#[test]
fn a_missing_category_or_a_subcategory_name_reads_as_no_markers() {
    let db = odb::open_db(FIXTURE).expect("fixture");
    // Only a TOP-level name is looked up (dbBlock::findMarkerCategory), as the router does.
    for name in ["NO_SUCH", "Short"] {
        assert!(odb::marker_category_all_records(&db, name).expect("records").is_empty());
        assert!(odb::marker_category_all_layers(&db, name).expect("layers").is_empty());
        assert!(odb::marker_category_all_source_nets(&db, name).expect("nets").is_empty());
    }
    let plain = odb::open_db("test/fixtures/counter.odb").expect("fixture");
    assert!(odb::marker_category_all_records(&plain, "DRC").expect("records").is_empty());
}
