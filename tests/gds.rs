// SPDX-License-Identifier: Apache-2.0
// odb's GDS reader/writer (gdsin / gdsout) through the bridge. The three cases are upstream's own
// odb GDS unit tests (reader, writer, edit) ported one for one; the fixture is the sky130 inv_1
// cell those tests read.
#![cfg(unix)]

use vyges_opendb_lib as ffi;

const INV1: &str = "test/fixtures/sky130_fd_sc_hd__inv_1.gds";

type Xy = Vec<(i64, i64)>;
type Props = Vec<(i64, String)>;

/// One structure's elements, split by kind as the flat encoding lays them out (shim.h).
#[derive(Default, Debug)]
struct Elems {
    boundaries: Vec<(i64, i64, Xy)>,
    boxes: Vec<(i64, i64, [i64; 4], Props)>,
    paths: Vec<(i64, i64, Xy)>,
    srefs: Vec<(String, [f64; 3])>,
    texts: Vec<(i64, String, [f64; 3])>,
}

struct Cur<'a> {
    v: &'a [i64],
    reals: &'a [f64],
    strs: &'a [String],
    i: usize,
    r: usize,
    k: usize,
}

impl Cur<'_> {
    fn next(&mut self) -> i64 {
        self.i += 1;
        self.v[self.i - 1]
    }
    fn xy(&mut self) -> Xy {
        (0..self.next()).map(|_| (self.next(), self.next())).collect()
    }
    fn trans(&mut self) -> [f64; 3] {
        self.r += 3;
        [self.reals[self.r - 3], self.reals[self.r - 2], self.reals[self.r - 1]]
    }
    fn str(&mut self) -> String {
        self.k += 1;
        self.strs[self.k - 1].clone()
    }
    fn props(&mut self) -> Props {
        (0..self.next()).map(|_| (self.next(), self.str())).collect()
    }
}

fn elements(db: &ffi::OdbDb, lib: usize, s: &str) -> Elems {
    let (mut reals, mut strs) = (Vec::new(), Vec::new());
    let v = ffi::gds_elements(db, lib, s, &mut reals, &mut strs).expect("elements");
    let mut c = Cur { v: &v, reals: &reals, strs: &strs, i: 0, r: 0, k: 0 };
    let mut e = Elems::default();
    while c.i < v.len() {
        match c.next() {
            0 => {
                let (l, d) = (c.next(), c.next());
                e.boundaries.push((l, d, c.xy()));
                c.props();
            }
            1 => {
                let (l, d) = (c.next(), c.next());
                let b = [c.next(), c.next(), c.next(), c.next()];
                let props = c.props();
                e.boxes.push((l, d, b, props));
            }
            2 => {
                let (l, d, _path_type, _width) = (c.next(), c.next(), c.next(), c.next());
                e.paths.push((l, d, c.xy()));
                c.props();
            }
            3 => {
                let _origin = (c.next(), c.next());
                let t = c.trans();
                e.srefs.push((c.str(), t));
                c.props();
            }
            5 => {
                let (l, _d, _x, _y, _v, _h) = (c.next(), c.next(), c.next(), c.next(), c.next(), c.next());
                let t = c.trans();
                e.texts.push((l, c.str(), t));
                c.props();
            }
            other => panic!("unexpected element kind {other}"),
        }
    }
    e
}

/// Upstream `reader` and `writer` assert the same facts about inv_1 — once on the file as read,
/// once on the writer's output read back.
fn assert_inv1(db: &ffi::OdbDb, lib: usize) {
    assert_eq!(ffi::gds_lib_name(db, lib).unwrap(), "sky130_fd_sc_hd__inv_1");
    assert_eq!(ffi::gds_lib_units(db, lib).unwrap(), vec![1e-3, 1e-9]);
    assert_eq!(ffi::gds_structure_names(db, lib).unwrap(), vec!["sky130_fd_sc_hd__inv_1"]);
    let e = elements(db, lib, "sky130_fd_sc_hd__inv_1");
    assert_eq!((e.boundaries.len(), e.boxes.len(), e.paths.len(), e.srefs.len(), e.texts.len()), (44, 0, 2, 0, 8));
    for p in &e.paths {
        assert_eq!((p.0, p.1, p.2.len()), (68, 20, 2));
    }
    // getXY returns the closing point: a GDS BOUNDARY repeats its first vertex.
    assert_eq!(e.boundaries[0].2, vec![(0, 0), (1380, 0), (1380, 2720), (0, 2720), (0, 0)]);
    assert_eq!((e.texts[0].0, e.texts[0].1.as_str()), (67, "Y"));
    // MAG is a real8 read back exactly: 0.17 survives the writer's truncating encoder.
    assert_eq!((e.texts[0].2[1], e.texts[0].2[2]), (0.17, 0.0));
}

#[test]
fn reader() {
    let db = ffi::new_db();
    let lib = ffi::gds_read(&db, INV1).expect("read inv_1");
    assert_inv1(&db, lib);
}

#[test]
fn writer() {
    let db = ffi::new_db();
    let old = ffi::gds_read(&db, INV1).unwrap();
    let out = std::env::temp_dir().join(format!("vyges_gds_writer_{}.gds", std::process::id()));
    ffi::gds_write(&db, old, out.to_str().unwrap()).expect("write");
    let lib = ffi::gds_read(&db, out.to_str().unwrap()).expect("read back");
    assert_ne!(lib, old);
    assert_inv1(&db, lib);
}

#[test]
fn edit() {
    let db = ffi::new_db();
    let lib = ffi::gds_create_lib(&db, "test_lib").unwrap();
    for s in ["str1", "str2", "str3"] {
        ffi::gds_structure_create(&db, lib, s).unwrap();
    }
    ffi::gds_structure_destroy(&db, lib, "str2").unwrap();
    // box layer 3 datatype 4, bounds (0,0)-(1000,1000), one property (12, "test")
    ffi::gds_add(&db, lib, "str1", &[1, 3, 4, 0, 0, 1000, 1000, 1, 12], &[], &["test".into()]).unwrap();
    // sref str3 -> str1 with STRANS (no reflection, mag 2, angle 90), origin left at (0,0)
    ffi::gds_add(&db, lib, "str3", &[3, 0, 0, 0], &[0.0, 2.0, 90.0], &["str1".into()]).unwrap();

    let out = std::env::temp_dir().join(format!("vyges_gds_edit_{}.gds", std::process::id()));
    ffi::gds_write(&db, lib, out.to_str().unwrap()).unwrap();
    let lib2 = ffi::gds_read(&db, out.to_str().unwrap()).unwrap();

    assert_eq!(ffi::gds_lib_name(&db, lib2).unwrap(), "test_lib");
    // The destroyed structure is not written.
    assert_eq!(ffi::gds_structure_names(&db, lib2).unwrap(), vec!["str1", "str3"]);
    let s1 = elements(&db, lib2, "str1");
    assert_eq!(s1.boxes.len(), 1);
    assert_eq!((s1.boxes[0].0, s1.boxes[0].1), (3, 4));
    // Not asserted upstream; without it a writer that drops the bounds passes this case.
    assert_eq!(s1.boxes[0].2, [0, 0, 1000, 1000]);
    assert_eq!(s1.boxes[0].3, vec![(12, "test".to_string())]);
    let s3 = elements(&db, lib2, "str3");
    assert_eq!(s3.srefs.len(), 1);
    assert_eq!(s3.srefs[0].0, "str1");
    assert_eq!((s3.srefs[0].1[1], s3.srefs[0].1[2]), (2.0, 90.0));
}

#[test]
fn duplicate_structure_is_err() {
    // dbGDSStructure::create returns null for a name already in the library; the shim turns
    // that into an error rather than a silent no-op.
    let db = ffi::new_db();
    let lib = ffi::gds_create_lib(&db, "l").unwrap();
    ffi::gds_structure_create(&db, lib, "a").unwrap();
    assert!(ffi::gds_structure_create(&db, lib, "a").is_err());
}

#[test]
fn bad_inputs_are_err() {
    let db = ffi::new_db();
    assert!(ffi::gds_read(&db, "/nonexistent/x.gds").is_err());
    assert!(ffi::gds_lib_name(&db, 7).is_err());
    let lib = ffi::gds_create_lib(&db, "l").unwrap();
    ffi::gds_structure_create(&db, lib, "a").unwrap();
    // truncated boundary, unknown kind, sref to a structure that does not exist
    assert!(ffi::gds_add(&db, lib, "a", &[0, 1, 0, 2, 0, 0], &[], &[]).is_err());
    assert!(ffi::gds_add(&db, lib, "a", &[9], &[], &[]).is_err());
    assert!(ffi::gds_add(&db, lib, "a", &[3, 0, 0, 0], &[0.0, 1.0, 0.0], &["nope".into()]).is_err());
}

#[test]
fn elements_round_trip_through_gds_add() {
    // Every element of inv_1, re-created from its own encoding, encodes the same again.
    let db = ffi::new_db();
    let src = ffi::gds_read(&db, INV1).unwrap();
    let (mut reals, mut strs) = (Vec::new(), Vec::new());
    let ints = ffi::gds_elements(&db, src, "sky130_fd_sc_hd__inv_1", &mut reals, &mut strs).unwrap();
    let dst = ffi::gds_create_lib(&db, "copy").unwrap();
    ffi::gds_structure_create(&db, dst, "c").unwrap();
    ffi::gds_add(&db, dst, "c", &ints, &reals, &strs).unwrap();
    let (mut reals2, mut strs2) = (Vec::new(), Vec::new());
    let ints2 = ffi::gds_elements(&db, dst, "c", &mut reals2, &mut strs2).unwrap();
    assert_eq!((ints, reals, strs), (ints2, reals2, strs2));
}
