// SPDX-License-Identifier: Apache-2.0
//! Reading a technology-only LEF — layers and rules, no masters — as the first LEF of a database.
//!
//! 🔑 OpenROAD reads this fixture as 3 layers and 0 libraries (`read_lef`, then `getLayers` /
//! `getLibs` through its Tcl API): odb creates the library lazily at the first MACRO, so none
//! exists, and that is not an error. The binding used to treat the missing library as a failed
//! read and refused such LEFs.
use vyges_opendb_lib as odb;

#[test]
fn a_technology_only_lef_reads_without_a_library() {
    let db = odb::new_db();
    odb::read_lef(&db, "test/fixtures/tech_only.lef").expect("a tech-only LEF is a valid first LEF");
    assert_eq!(odb::num_v54_spacing_rules(&db, "M1").expect("M1 exists"), 1);
    assert_eq!(odb::num_v54_spacing_rules(&db, "M2").expect("M2 exists"), 1);
}
