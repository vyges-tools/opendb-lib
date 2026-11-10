// SPDX-License-Identifier: Apache-2.0
//! LEF58_MINIMUMCUT given per CUTCLASS reads back as a per-cut-class rule (OpenROAD #12176).
//!
//! ⛔ **This pins OUR fix, not the reference as shipped.** The rule as written: a minimum-cut rule
//! given as `MINIMUMCUT CUTCLASS <class> <n> ...` counts cuts per cut class, so the rule is
//! per-cut-class. The pinned upstream LEF reader records each class's count but never sets the
//! rule's per-cut-class flag (OpenROAD issue #12176), so the CLEAN reference reports `false` for M2
//! below. Our LEF read sets it (`fix_min_cut_per_cut_class` in src/shim.cc), and so does the
//! -vyges OpenROAD build, which carries the fix as a patch; that build is the reference this field
//! is scored against. Witnessed 2026-11-10 with the same rule through the reference's Tcl API:
//! unpatched pin `isPerCutClass` = 0, fixed build = 1.
//!
//! Delete this file's ⛔ and flip nothing else when upstream merges a fix: the expectations stay.
use vyges_opendb_lib as odb;

const LEF: &str = "test/fixtures/mincut_cutclass.lef";

fn check(db: &odb::OdbDb) {
    // Control: a plain `MINIMUMCUT 2` rule has no classes and stays a plain rule.
    assert_eq!(odb::num_layer_get_tech_layer_min_cut_rules(db, "M1"), 1);
    assert!(odb::mincutrule_cut_class_cuts(db, "M1", 0).is_empty());
    assert!(!odb::mincutrule_is_per_cut_class(db, "M1", 0), "a plain MINIMUMCUT rule must not be per-cut-class");

    // The rule under test: the class counts are recorded (upstream does this) AND the flag is set
    // (upstream does not, #12176).
    assert_eq!(odb::num_layer_get_tech_layer_min_cut_rules(db, "M2"), 1);
    assert_eq!(odb::mincutrule_cut_class_cuts(db, "M2", 0), vec!["VA\t2".to_string(), "VB\t1".to_string()]);
    assert!(odb::mincutrule_is_per_cut_class(db, "M2", 0), "MINIMUMCUT CUTCLASS must read back per-cut-class (OpenROAD #12176)");
}

#[test]
fn read_lef_sets_per_cut_class_on_a_cutclass_min_cut_rule() {
    let db = odb::new_db();
    odb::read_lef(&db, LEF).expect("read_lef");
    check(&db);
}

#[test]
fn tech_from_lef_sets_per_cut_class_on_a_cutclass_min_cut_rule() {
    let db = odb::new_db();
    odb::tech_from_lef(&db, "t", LEF).expect("tech_from_lef");
    check(&db);
}

#[test]
fn tech_and_lib_from_lef_sets_per_cut_class_on_a_cutclass_min_cut_rule() {
    let db = odb::new_db();
    odb::tech_and_lib_from_lef(&db, "t", "l", LEF).expect("tech_and_lib_from_lef");
    check(&db);
}
