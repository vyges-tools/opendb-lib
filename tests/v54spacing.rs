// SPDX-License-Identifier: Apache-2.0
//! A routing layer's LEF 5.4 spacing rules — `SPACING <s> [RANGE <min> <max>]` — as a global
//! router's line-to-via pitch reads them: every rule's spacing, and the width range it is limited
//! to where it has one.
//!
//! 🔑 The spacings and the has-range flags below are **OpenROAD's own answers** for this LEF, taken
//! through its Tcl API (`getV54SpacingRules` → `getSpacing`, `hasRange`). Its Tcl binding cannot
//! return `getRange`'s out-parameters, so the bounds are the LEF's own (in database units). Adding
//! two vias to the LEF left those answers unchanged (re-checked against the reference).
use vyges_opendb_lib as odb;

const LEF: &str = "test/fixtures/v54_range.lef";

#[test]
fn v54_spacing_rules_carry_their_spacing_and_range() {
    let db = odb::new_db();
    odb::read_lef(&db, LEF).expect("lef");
    let rules = |layer: &str| -> Vec<(u32, Vec<u32>)> {
        let n = odb::num_v54_spacing_rules(&db, layer).expect("count");
        (0..n).map(|i| (odb::v54_spacing_rule_spacing(&db, layer, i), odb::v54_spacing_rule_range(&db, layer, i))).collect()
    };
    assert_eq!(rules("M1"), vec![(100, vec![]), (150, vec![300, 600])]);
    assert_eq!(rules("V1"), vec![(120, vec![])]);
    assert_eq!(rules("M2"), vec![(100, vec![100, 200])]);
    // An index past the end, or an unknown layer, reads as no rule.
    assert_eq!((odb::v54_spacing_rule_spacing(&db, "M1", 9), odb::v54_spacing_rule_range(&db, "nope", 0)), (0, vec![]));
}

/// ⛔ `dbBlock::getDefaultVias` reads the `OR_DEFAULT` STRING PROPERTY, not `isDefault`: OpenROAD
/// reports both vias here as `isDefault` 1, and only `V1_B` as carrying the property.
#[test]
fn or_default_is_a_property_not_the_default_flag() {
    let db = odb::new_db();
    odb::read_lef(&db, LEF).expect("lef");
    assert_eq!(
        (odb::techvia_has_string_property(&db, "V1_A", "OR_DEFAULT"), odb::techvia_has_string_property(&db, "V1_B", "OR_DEFAULT")),
        (false, true)
    );
    assert!(!odb::techvia_has_string_property(&db, "nope", "OR_DEFAULT"));
}
