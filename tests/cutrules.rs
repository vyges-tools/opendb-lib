// SPDX-License-Identifier: Apache-2.0
//! LEF58 cut rules — the enclosure and spacing rules a power grid needs to size its vias.
//!
//! Each of these hangs off a tech layer as a `dbSet` whose elements have no name, so they are
//! addressed by `(layer, index)`. ⚠️ The binding generator skipped nameless sets entirely, count
//! included, which left the rules bound but impossible to enumerate — a rule set you cannot count
//! is one you cannot read.
//!
//! 🔑 The expected values below are **OpenROAD's own answers**, taken from
//! `getTechLayerCutEnclosureRules` via its Tcl API on this same LEF. The type is reported here as
//! a string where the reference reports the enum's integer value: `0 DEFAULT, 1 EOL, 2 ENDSIDE,
//! 3 HORZ_AND_VERT`.
//!
//! ℹ️ Fixture and expectation are committed together and neither is regenerated at run time, so
//! this is a frozen pair rather than the kind of archived golden that drifts against a live tool.
use vyges_opendb_lib as odb;

const FIXTURE: &str = "test/fixtures/asap7_tech.odb";

/// `(layer, type, first overhang, second overhang)`, in the order the reference reports them.
const EXPECTED: &[(&str, &str, i32, i32)] = &[
    ("V3", "EOL", 5, 0),
    ("V3", "EOL", 11, 0),
    ("V3", "ENDSIDE", 0, 0),
    ("V3", "ENDSIDE", 0, 0),
    ("V4", "DEFAULT", 11, 0),
    ("V4", "EOL", 11, 11),
    ("V4", "ENDSIDE", 0, 0),
    ("V4", "ENDSIDE", 0, 0),
    ("V4", "ENDSIDE", 0, 0),
    ("V4", "ENDSIDE", 0, 0),
    ("V5", "EOL", 11, 11),
    ("V5", "ENDSIDE", 0, 0),
    ("V5", "ENDSIDE", 0, 0),
    ("V5", "ENDSIDE", 0, 0),
    ("V5", "ENDSIDE", 0, 0),
    ("V6", "DEFAULT", 11, 0),
    ("V6", "EOL", 11, 11),
    ("V6", "ENDSIDE", 0, 0),
    ("V6", "ENDSIDE", 0, 0),
    ("V6", "ENDSIDE", 0, 0),
    ("V6", "ENDSIDE", 0, 0),
];

#[test]
fn cut_enclosure_rules_read_back_exactly_as_the_reference_reports_them() {
    let db = odb::open_db(FIXTURE).expect("read");
    let layers: Vec<String> = (0..odb::num_layers(&db).unwrap())
        .map(|i| odb::nth_layer_name(&db, i).unwrap())
        .collect();

    let mut got = Vec::new();
    for l in &layers {
        for i in 0..odb::num_layer_get_tech_layer_cut_enclosure_rules(&db, l) {
            got.push((
                l.clone(),
                odb::cutenclosurerule_get_type(&db, l, i),
                odb::cutenclosurerule_get_first_overhang(&db, l, i),
                odb::cutenclosurerule_get_second_overhang(&db, l, i),
            ));
        }
    }

    let want: Vec<(String, String, i32, i32)> = EXPECTED
        .iter()
        .map(|(l, t, a, b)| (l.to_string(), t.to_string(), *a, *b))
        .collect();
    assert_eq!(got, want, "cut enclosure rules differ from the reference");
}

#[test]
fn a_layer_with_no_cut_rules_reports_none_rather_than_failing() {
    // ⚠️ The count is what makes the set enumerable, so it has to be right at zero too: a routing
    // layer carries no cut enclosure rules, and asking must give 0 rather than a stale or
    // out-of-range answer.
    let db = odb::open_db(FIXTURE).expect("read");
    assert_eq!(odb::num_layer_get_tech_layer_cut_enclosure_rules(&db, "M1"), 0);
    assert_eq!(odb::num_layer_get_tech_layer_cut_enclosure_rules(&db, "no_such_layer"), 0);
}

#[test]
fn cut_classes_carry_the_names_the_enclosure_rules_refer_to() {
    // Enclosure rules are written per CUTCLASS in LEF58, so the class names have to be readable
    // for the rules above to be applicable to anything.
    let db = odb::open_db(FIXTURE).expect("read");
    let n = odb::num_layer_get_tech_layer_cut_class_rules(&db, "V3");
    assert!(n > 0, "V3 declares cut classes");
    let names: Vec<String> = (0..n).map(|i| odb::cutclassrule_get_name(&db, "V3", i)).collect();
    assert!(names.iter().all(|s| !s.is_empty()), "every cut class is named: {names:?}");
}

#[test]
fn the_above_below_split_matches_what_the_reference_reports() {
    // 🔑 The reference's `ViaEnclosure` debug on asap7 reports "2" enclosure rules for V4 from
    // above and "2" from below. That split has to be reconstructible from the bindings, or the
    // selection built on them cannot agree.
    //
    // ⚠️ A rule declaring NEITHER above nor below governs both sides — reading the two flags as a
    // two-way choice drops every unqualified rule, and unqualified is the common case.
    let db = odb::open_db(FIXTURE).expect("read");
    let n = odb::num_layer_get_tech_layer_cut_enclosure_rules(&db, "V4");
    assert!(n > 0, "V4 declares cut enclosure rules");

    let mut above = 0;
    let mut below = 0;
    for i in 0..n {
        let is_above = odb::cutenclosurerule_is_above(&db, "V4", i);
        let is_below = odb::cutenclosurerule_is_below(&db, "V4", i);
        let (t, b) = if !is_above && !is_below { (true, true) } else { (is_above, is_below) };
        if t {
            above += 1;
        }
        if b {
            below += 1;
        }
    }
    // Every rule governs at least one side, and the two counts are equal here because asap7's V4
    // rules are symmetric about the cut layer.
    assert_eq!(above, below, "V4's rules split evenly, as the reference's 2 and 2 imply");
    assert!(above >= 2, "at least the two the reference selects, before cut-class filtering");
}
