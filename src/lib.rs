// SPDX-License-Identifier: Apache-2.0
//! `vyges-opendb-lib` — low-level FFI to OpenROAD's OpenDB (`libodb`).
//!
//! Read + write-path surface over a standalone libodb (no tcl/swig/engines), proven on
//! linux/x86_64, linux/arm64, and macOS/Apple Silicon. Objects are addressed by name so
//! no raw odb pointers cross the FFI boundary. The write primitives are the building
//! blocks for the ECO applier (`InsertECOBuffers`). The safe, ergonomic wrappers live in
//! the sibling crate `vyges-opendb`.

/// The OpenROAD commit this crate's `libodb` is built from.
///
/// 🔑 **One definition for the whole programme.** It comes from `openroad-pin.yaml` via `build.rs`,
/// is re-exported by `vyges-opendb`, and every engine reports it rather than carrying a SHA of its
/// own. A re-pin is then one file plus a rebuild.
///
/// ⚠️ **This says what the BINARY was built against — it cannot tell you the binary is current.**
/// A stale build reports its own stale pin perfectly happily. That is what it is for: a harness
/// compares this against the oracle image it is about to run and refuses on a mismatch. On
/// 2026-08-23 two engines ran a whole gate against the previous pin's oracle, and only one of them
/// happened to fail loudly.
pub const OPENROAD_PIN: &str = env!("VYGES_OPENROAD_PIN");

// Unix-only: libodb is not built on non-unix targets (see build.rs). On Windows this crate
// compiles to an empty stub so a `--features odb` build still succeeds across the dist matrix.
#[cfg(unix)]
#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("shim.h");

        /// Opaque handle owning a `dbDatabase` + its `utl::Logger`.
        type OdbDb;

        // open / read / write
        fn open_db(path: &str) -> Result<UniquePtr<OdbDb>>;
        fn write_db(db: &OdbDb, path: &str) -> Result<()>;
        fn write_def(db: &OdbDb, path: &str) -> Result<()>;
        fn read_lef(db: &OdbDb, lef_path: &str) -> Result<()>;
        fn read_def(db: &OdbDb, def_path: &str, mode: &str) -> Result<()>;

        // read / inspect
        fn block_name(db: &OdbDb) -> String;
        fn num_insts(db: &OdbDb) -> usize;
        fn num_nets(db: &OdbDb) -> usize;
        fn num_bterms(db: &OdbDb) -> usize;
        fn nth_inst_name(db: &OdbDb, i: usize) -> String;
        fn first_master_name(db: &OdbDb) -> String;
        fn find_master(db: &OdbDb, substr: &str) -> String;
        fn input_pin(db: &OdbDb, inst: &str) -> String;
        fn output_pin(db: &OdbDb, inst: &str) -> String;
        fn inst_master(db: &OdbDb, inst: &str) -> String;
        fn num_iterms(db: &OdbDb, inst: &str) -> usize;
        fn nth_iterm_name(db: &OdbDb, inst: &str, i: usize) -> String;
        fn net_of(db: &OdbDb, inst: &str, pin: &str) -> String;
        fn inst_x(db: &OdbDb, inst: &str) -> i32;
        fn inst_y(db: &OdbDb, inst: &str) -> i32;
        fn nth_bterm_name(db: &OdbDb, i: usize) -> String;
        fn bterm_net(db: &OdbDb, bterm: &str) -> String;
        fn bterm_x(db: &OdbDb, bterm: &str) -> i32;
        fn bterm_y(db: &OdbDb, bterm: &str) -> i32;

        // write / ECO primitives
        fn create_net(db: &OdbDb, name: &str) -> Result<()>;
        fn create_inst(db: &OdbDb, master: &str, name: &str) -> Result<()>;
        fn set_inst_location(db: &OdbDb, inst: &str, x: i32, y: i32) -> Result<()>;
        fn set_inst_orient(db: &OdbDb, inst: &str, orient: &str) -> Result<()>;
        fn add_track_pattern_x(db: &OdbDb, layer: &str, origin: i32, count: i32, step: i32) -> Result<()>;
        fn add_track_pattern_y(db: &OdbDb, layer: &str, origin: i32, count: i32, step: i32) -> Result<()>;
        fn add_obstruction(db: &OdbDb, layer: &str, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn num_obstructions(db: &OdbDb) -> usize;
        fn clear_obstructions(db: &OdbDb) -> usize;
        fn bterm_direction(db: &OdbDb, bterm: &str) -> String;
        fn total_wire_length(db: &OdbDb) -> u64;
        fn nth_net_name(db: &OdbDb, i: usize) -> String;
        fn net_sigtype(db: &OdbDb, net: &str) -> String;
        fn net_is_special(db: &OdbDb, net: &str) -> bool;
        fn num_net_iterms(db: &OdbDb, net: &str) -> usize;
        fn nth_net_iterm(db: &OdbDb, net: &str, i: usize) -> String;
        fn num_net_bterms(db: &OdbDb, net: &str) -> usize;
        fn nth_net_bterm(db: &OdbDb, net: &str, i: usize) -> String;
        fn place_bterm(db: &OdbDb, bterm: &str, layer: &str, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn connect(db: &OdbDb, inst: &str, pin: &str, net: &str) -> Result<()>;
        fn disconnect(db: &OdbDb, inst: &str, pin: &str) -> Result<()>;

        // floorplan writes (vyges-ifp). Hand-written rather than generated: setDieArea takes a
        // Rect, setCoreArea a Polygon, and dbRow::create is a static factory.
        fn block_set_die_area(db: &OdbDb, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn block_set_core_area(db: &OdbDb, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn block_set_core_area_from_rows(db: &OdbDb) -> Result<()>;
        fn block_compute_core_area(db: &OdbDb) -> Result<Vec<i32>>;
        fn tech_manufacturing_grid(db: &OdbDb) -> Result<i32>;
        #[allow(clippy::too_many_arguments)]
        fn row_create(db: &OdbDb, name: &str, site: &str, x: i32, y: i32, orient: &str,
                      direction: &str, num_sites: i32, spacing: i32) -> Result<()>;
        fn num_sites(db: &OdbDb) -> Result<usize>;
        fn nth_site_name(db: &OdbDb, i: usize) -> Result<String>;
        fn num_rows(db: &OdbDb) -> Result<usize>;
        fn clear_rows(db: &OdbDb) -> Result<usize>;
        // A hybrid site's row pattern. `site_has_row_pattern` is generated; its contents cannot
        // be, so these three are hand-written — see shim.h.
        fn site_row_pattern_len(db: &OdbDb, site: &str) -> Result<usize>;
        fn site_row_pattern_site(db: &OdbDb, site: &str, i: usize) -> Result<String>;
        fn site_row_pattern_orient(db: &OdbDb, site: &str, i: usize) -> Result<String>;
        // Row cutting around macros (vyges-tap). odb's OWN algorithm from odb/util.h -- the
        // engine chooses the blockages, odb does the cutting. See shim.h.
        fn block_cut_rows(db: &OdbDb, min_row_width: i32, min_row_height: i32,
                          blockage_insts: &[String],
                          halo_x: i32, halo_y: i32) -> Result<()>;
        fn has_one_site_master(db: &OdbDb) -> bool;
        // Row enumeration + site class: the row_get_* accessors are generated but address a row
        // by NAME, and nothing enumerates them. See shim.h.
        fn nth_row_name(db: &OdbDb, i: usize) -> Result<String>;
        fn site_get_class(db: &OdbDb, site: &str) -> Result<String>;
        // Physical-only instance creation (taps, endcaps, fillers) -- see shim.h.
        fn create_physical_inst(db: &OdbDb, master: &str, name: &str) -> Result<()>;
        fn inst_bbox(db: &OdbDb, inst: &str) -> Result<Vec<i32>>;
        // Master enumeration + LEF master type -- see shim.h.
        fn num_masters(db: &OdbDb) -> Result<usize>;
        fn nth_master_name(db: &OdbDb, i: usize) -> Result<String>;
        fn master_get_type(db: &OdbDb, master: &str) -> Result<String>;
        // Row geometry BY INDEX -- row names are NOT unique, so the generated by-name accessors
        // silently return the first match. See shim.h.
        fn nth_row_bbox(db: &OdbDb, i: usize) -> Result<Vec<i32>>;
        fn nth_row_site(db: &OdbDb, i: usize) -> Result<String>;
        fn nth_row_orient(db: &OdbDb, i: usize) -> Result<String>;
        fn nth_row_direction(db: &OdbDb, i: usize) -> Result<String>;
        fn destroy_inst(db: &OdbDb, inst: &str) -> Result<()>;
        // density fill (vyges-fin) -- see shim.h
        fn inst_shapes(db: &OdbDb) -> Result<Vec<i64>>;
        fn obstruction_boxes(db: &OdbDb) -> Result<Vec<i64>>;
        fn blockage_boxes(db: &OdbDb) -> Result<Vec<i32>>;
        fn swire_boxes(db: &OdbDb) -> Result<Vec<i64>>;
        #[allow(clippy::too_many_arguments)]
        fn blockage_create(db: &OdbDb, x1: i32, y1: i32, x2: i32, y2: i32,
                           inst: &str, soft: bool) -> Result<()>;
        fn num_blockages(db: &OdbDb) -> Result<usize>;
        fn blockage_destroy(db: &OdbDb, idx: usize) -> Result<()>;
        #[allow(clippy::too_many_arguments)]
        fn fill_create(db: &OdbDb, needs_opc: bool, mask: u32, layer: &str,
                       x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn num_fills(db: &OdbDb) -> Result<usize>;
        fn clear_fills(db: &OdbDb) -> Result<usize>;
        fn num_layers(db: &OdbDb) -> Result<usize>;
        fn nth_layer_name(db: &OdbDb, i: usize) -> Result<String>;
        fn layer_direction(db: &OdbDb, layer: &str) -> Result<String>;
        // Routing track coordinates (vyges-ppl) -- see shim.h.
        fn track_grid_x(db: &OdbDb, layer: &str) -> Result<Vec<i32>>;
        fn track_grid_y(db: &OdbDb, layer: &str) -> Result<Vec<i32>>;
        fn bterm_constraint_region(db: &OdbDb, bterm: &str) -> Result<Vec<i32>>;
        fn blocked_regions_for_pins(db: &OdbDb) -> Result<Vec<i32>>;
        fn die_area_polygon(db: &OdbDb) -> Result<Vec<i32>>;
        fn master_obstruction_boxes(db: &OdbDb, master: &str) -> Result<Vec<i32>>;
        fn master_pin_boxes(db: &OdbDb, master: &str) -> Result<Vec<i32>>;
        fn mterm_pin_boxes(db: &OdbDb, master: &str, term: &str) -> Result<Vec<i32>>;
        fn mpin_boxes(db: &OdbDb, master: &str, term: &str, idx: usize) -> Result<Vec<i32>>;
        fn inst_halo(db: &OdbDb, inst: &str) -> Result<Vec<i64>>;
        fn net_destroy(db: &OdbDb, net: &str) -> Result<()>;
        fn iterm_get_id(db: &OdbDb, inst: &str, pin: &str) -> Result<u32>;
        fn inst_get_id(db: &OdbDb, inst: &str) -> Result<u32>;
        fn techvialayerrule_rect(db: &OdbDb, gen_idx: usize, layer_idx: usize) -> Result<Vec<i32>>;
        fn via_create_generated(db: &OdbDb, name: &str, rule: &str, bottom: &str, cut: &str,
                                top: &str, cut_w: i32, cut_h: i32, spacing_x: i32, spacing_y: i32,
                                bot_enc_x: i32, bot_enc_y: i32, top_enc_x: i32, top_enc_y: i32,
                                rows: i32, cols: i32, origin_x: i32, origin_y: i32) -> Result<()>;
        fn swire_add_via(db: &OdbDb, net: &str, fixed: bool, via: &str, x: i32, y: i32,
                         shape: &str) -> Result<()>;
        fn layer_get_spacing_for(db: &OdbDb, layer: &str, width: i32, length: i32) -> Result<i32>;
        fn layer_find_v55_spacing(db: &OdbDb, layer: &str, width: i32, prl: i32) -> Result<i32>;
        fn layer_find_tw_spacing(db: &OdbDb, layer: &str, width1: i32, width2: i32, prl: i32) -> Result<i32>;
        fn layer_min_area(db: &OdbDb, layer: &str) -> Result<i64>;
        fn tech_via_boxes(db: &OdbDb, via: &str) -> Result<Vec<i32>>;
        fn bpin_layer_boxes(db: &OdbDb, bterm: &str, pin: usize) -> Result<Vec<i32>>;
        fn region_boundaries(db: &OdbDb, region: &str) -> Result<Vec<i32>>;
        fn net_swire_shapes(db: &OdbDb, net: &str) -> Result<Vec<i64>>;
        fn tech_via_layer(db: &OdbDb, via: &str, which: &str) -> Result<String>;
        fn num_v54_spacing_rules(db: &OdbDb, layer: &str) -> Result<usize>;
        fn v54_spacing_rule_adjacent_cuts(db: &OdbDb, layer: &str, idx: usize) -> Result<u32>;
        fn v54_spacing_rule_adjacent_spacing(db: &OdbDb, layer: &str, idx: usize) -> Result<i32>;
        fn v54_spacing_rule_adjacent_except_same_pgnet(
            db: &OdbDb,
            layer: &str,
            idx: usize,
        ) -> Result<bool>;
        fn num_array_spacing_rules(db: &OdbDb, layer: &str) -> Result<usize>;
        fn array_spacing_rule_cut_class(db: &OdbDb, layer: &str, idx: usize) -> Result<String>;
        fn array_spacing_rule_is_parallel_overlap(db: &OdbDb, layer: &str, idx: usize) -> Result<bool>;
        fn array_spacing_rule_is_long_array(db: &OdbDb, layer: &str, idx: usize) -> Result<bool>;
        fn array_spacing_rule_array_width(db: &OdbDb, layer: &str, idx: usize) -> Result<i32>;
        fn array_spacing_rule_cut_spacing(db: &OdbDb, layer: &str, idx: usize) -> Result<i32>;
        fn array_spacing_rule_cuts_spacing(db: &OdbDb, layer: &str, idx: usize) -> Result<Vec<i32>>;
        fn num_cut_spacing_table_rules(db: &OdbDb, layer: &str) -> Result<usize>;
        fn cut_spacing_table_max_spacing(db: &OdbDb, layer: &str, idx: usize, cls: &str) -> Result<i32>;
        fn cut_spacing_table_spacing(db: &OdbDb, layer: &str, idx: usize, cls: &str, side1: bool, side2: bool) -> Result<i32>;
        fn cut_spacing_table_is_center_and_edge(db: &OdbDb, layer: &str, idx: usize, cls: &str) -> Result<bool>;
        fn cut_spacing_table_is_center_to_center(db: &OdbDb, layer: &str, idx: usize, cls: &str) -> Result<bool>;
        fn num_width_table_rules(db: &OdbDb, layer: &str) -> Result<usize>;
        fn width_table_rule_is_wrong_direction(db: &OdbDb, layer: &str, idx: usize) -> Result<bool>;
        fn width_table_rule_widths(db: &OdbDb, layer: &str, idx: usize) -> Result<Vec<i32>>;
        fn swire_add_box_shaped(db: &OdbDb, net: &str, fixed: bool, layer: &str,
                                x1: i32, y1: i32, x2: i32, y2: i32, shape: &str) -> Result<()>;
        fn swire_add_box(db: &OdbDb, net: &str, fixed: bool, layer: &str,
                         x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn swire_clear_routed(db: &OdbDb, net: &str) -> Result<usize>;
        fn bterm_create(db: &OdbDb, net: &str, name: &str) -> Result<()>;
        fn bterm_create_pin(db: &OdbDb, bterm: &str, layer: &str,
                            x1: i32, y1: i32, x2: i32, y2: i32) -> Result<usize>;
        fn bterm_destroy(db: &OdbDb, bterm: &str) -> Result<()>;
        fn bterm_clear_unfixed_bpins(db: &OdbDb, bterm: &str) -> Result<usize>;
        fn bterm_add_pin_box(db: &OdbDb, bterm: &str, layer: &str,
                             x1: i32, y1: i32, x2: i32, y2: i32) -> Result<bool>;
        fn layer_get_type(db: &OdbDb, layer: &str) -> Result<String>;
        fn bterm_top_layer_grid(db: &OdbDb) -> Result<Vec<i32>>;
        fn bterm_top_layer_grid_layer(db: &OdbDb) -> Result<String>;
        fn bterm_top_layer_grid_is_rect(db: &OdbDb) -> Result<bool>;
        fn fixed_bterm_shapes(db: &OdbDb) -> Result<Vec<i64>>;
        fn num_bterm_groups(db: &OdbDb) -> Result<usize>;
        fn nth_bterm_group(db: &OdbDb, i: usize) -> Result<Vec<String>>;
        fn nth_bterm_group_ordered(db: &OdbDb, i: usize) -> Result<bool>;
        fn track_patterns_x(db: &OdbDb, layer: &str) -> Result<Vec<i32>>;
        fn track_patterns_y(db: &OdbDb, layer: &str) -> Result<Vec<i32>>;

        // antenna inputs (odb substrate) — numerator per routing layer, denominator per pin.
        // Consumed by vyges-ant; see shim.h for the v0 double-counting bound.
        fn num_net_wire_layers(db: &OdbDb, net: &str) -> usize;
        fn nth_net_wire_layer(db: &OdbDb, net: &str, i: usize) -> String;
        fn net_wire_area_on_layer(db: &OdbDb, net: &str, layer: &str) -> i64;
        fn net_wire_perimeter_on_layer(db: &OdbDb, net: &str, layer: &str) -> i64;
        fn layer_thickness(db: &OdbDb, layer: &str) -> i32;
        fn mterm_antenna_gate_area(db: &OdbDb, master: &str, term: &str) -> f64;
        fn mterm_antenna_diff_area(db: &OdbDb, master: &str, term: &str) -> f64;

        // Routed-wire shape graph: 8 i64 per shape (layer, x0,y0,x1,y1, is_via, via_bot,
        // via_top). Flat so one call yields a whole net — per-shape accessors would re-walk
        // the wire per query, which is quadratic on the big nets that matter most. Needed
        // because the antenna ratio is per GATE: the charge a gate collects comes only from
        // the metal reachable from it, not from the net's total on a layer.
        fn net_wire_shapes(db: &OdbDb, net: &str) -> Vec<i64>;
        fn layer_name_by_number(db: &OdbDb, number: i64) -> String;
        /// Every routed box with vias DECOMPOSED onto the layers they occupy: 7 i64 each
        /// (layer, x0, y0, x1, y1, is_routing_layer, came_from_via). A via is a cut plus an
        /// enclosure above and below, and on a net routed at met1 and up the via enclosure is
        /// the ONLY metal on li1 — where the standard-cell pins are.
        fn net_wire_boxes(db: &OdbDb, net: &str) -> Vec<i64>;
        fn iterm_avg_xy(db: &OdbDb, inst: &str, pin: &str, x: &mut i32, y: &mut i32) -> bool;
        /// The pin's own ROUTING-layer boxes in placed coordinates: 5 i64 each
        /// (layer, x0, y0, x1, y1). What the pin physically touches, rather than where it
        /// roughly is — proximity merges conductors that are electrically separate.
        fn iterm_pin_boxes(db: &OdbDb, inst: &str, pin: &str) -> Vec<i64>;

        // Diffusion-dependent (PWL) antenna limits — the form sky130 actually states. `which`
        // is "par"|"car"|"psr"|"csr"|"area_diff_reduce"|"gate_plus_diff"; an unknown value
        // throws rather than reading as "no limit". indices.len()==0 means unset, ==1 means a
        // single constant ratio rather than a curve.
        fn layerantenna_num_diff_pwl(db: &OdbDb, layer: &str, which: &str) -> Result<usize>;
        fn layerantenna_diff_pwl_index(db: &OdbDb, layer: &str, which: &str, i: usize) -> Result<f64>;
        fn layerantenna_diff_pwl_ratio(db: &OdbDb, layer: &str, which: &str, i: usize) -> Result<f64>;

        /// 3D structural lint. Files violations as dbMarker objects under a "3DBlox" category
        /// on the top chip and returns the total count (0 == clean). Errors if there is no
        /// top chip. Annotates the in-memory database; does not modify the design.
        fn check_3dblox(db: &OdbDb) -> Result<usize>;

        /// Rebuild the derived 3D tables (`dbUnfoldedChipInst` / region / bump) from the folded
        /// chip hierarchy.
        ///
        /// These are DERIVED and never serialised — the reader builds them on open, and nothing
        /// rebuilds them when a `dbChipInst` moves, so the unfolded accessors keep answering
        /// from the previous placement until this is called. `check_3dblox` rebuilds the model
        /// itself and is NOT affected; this is for callers reading the unfolded geometry
        /// directly. Errors if there is no top chip.
        /// 3D / chiplet construction — odb's `dbChip*` creation statics, hand-bound because
        /// their signatures are heterogeneous. Without these the 3D surface is read-only in
        /// practice: a design can be inspected and its chips moved, but not created.
        /// `tech` empty selects the database's default; naming one is what lets dies from
        /// different processes coexist.
        fn chip_create(db: &OdbDb, name: &str, tech: &str, chip_type: &str) -> Result<()>;
        fn chip_block_create(db: &OdbDb, chip: &str, name: &str) -> Result<()>;
        fn chip_inst_create(db: &OdbDb, parent_chip: &str, master_chip: &str, name: &str) -> Result<()>;
        fn chip_region_create(db: &OdbDb, chip: &str, name: &str, side: &str, layer: &str) -> Result<()>;
        fn chip_region_set_box(db: &OdbDb, chip: &str, region: &str, x1: i32, y1: i32, x2: i32, y2: i32) -> Result<()>;
        fn chip_bump_create(db: &OdbDb, chip: &str, region: &str, inst: &str) -> Result<()>;
        fn chip_conn_create(db: &OdbDb, name: &str, parent_chip: &str, top_inst: &str, top_region: &str, bottom_inst: &str, bottom_region: &str, thickness: i32) -> Result<()>;
        fn chip_net_create(db: &OdbDb, chip: &str, name: &str) -> Result<()>;
        fn chip_path_create(db: &OdbDb, chip: &str, name: &str) -> Result<()>;

        /// Root the assembly. The unfolded builder starts from the top chip and walks its chip
        /// insts, so with it left pointing elsewhere every unfolded table reads empty.
        /// Associate a bump instance with a logical 3D net, and declare an alignment-marker
        /// rule between two masters. Both exist because the checks that consume them are inert
        /// without them: logical connectivity compares the nets of aligned bump pairs, and the
        /// alignment-marker check returns immediately when no rule is defined.
        /// Database precision. A 3Dblox header declares the precision its (micron) coordinates
        /// are written at; reading one has to reconcile that with the database's own.
        /// An empty database — the starting point when a design is being built rather than read.
        fn new_db() -> UniquePtr<OdbDb>;
        /// A technology carrying only a precision. odb refuses to create a DIE chip without one.
        /// Build a named technology from a LEF file. `lefin` has been compiled into this
        /// library all along; a per-chip tech read from a chiplet's own `APR_tech_file` is what
        /// the 3D model exists to express.
        fn tech_from_lef(db: &OdbDb, name: &str, lef_path: &str) -> Result<()>;
        /// Read the cell MACROs from a LEF into a named library. `createTech` reads LAYERs and
        /// stops; a bump map names cell types, and those masters come from the `LEF_file` a
        /// .3dbv already points at. Idempotent on the library name.
        fn lib_from_lef(db: &OdbDb, lib_name: &str, tech_name: &str, lef_path: &str) -> Result<()>;
        /// A placeholder bump master, for a cell type no LEF on hand defines. Pass 0 x 0 unless
        /// you know the real geometry: odb reads a bump position from the instance bbox CENTRE
        /// while a bump map records its ORIGIN, so any other size moves the bump by half the
        /// master on a round trip.
        fn bump_master_create(db: &OdbDb, name: &str, width: i32, height: i32) -> Result<()>;
        fn tech_create(db: &OdbDb, name: &str) -> Result<()>;
        fn dbu_per_micron(db: &OdbDb) -> i32;
        fn set_dbu_per_micron(db: &OdbDb, dbu: i32);

        fn chip_net_add_bump(db: &OdbDb, chip: &str, net: &str, chip_inst: &str, region: &str, bump_index: usize) -> Result<()>;
        fn alignment_marker_rule_create(db: &OdbDb, master_a: &str, master_b: &str, tolerance: i32) -> Result<()>;

        fn set_top_chip(db: &OdbDb, chip: &str) -> Result<()>;

        fn construct_unfolded_model(db: &OdbDb) -> Result<()>;

        /// Capture libodb diagnostics rather than letting them reach its default stdout sink,
        /// so callers whose stdout is machine-readable stay parseable. End returns the captured text.
        /// Replace an instance's library cell in place (resize / Vt-swap). `false` if odb
        /// refuses (hierarchy-bound); errors on unknown inst/master or a don't-touch instance.
        fn swap_master(db: &OdbDb, inst: &str, master: &str) -> Result<bool>;

        /// ECO journal — speculative edits with a real rollback. Sequence:
        /// `eco_begin` -> edits -> `eco_commit` | `eco_undo`.
        fn eco_begin(db: &OdbDb) -> Result<()>;
        fn eco_end(db: &OdbDb) -> Result<()>;
        fn eco_commit(db: &OdbDb) -> Result<()>;
        fn eco_undo(db: &OdbDb) -> Result<()>;
        fn eco_empty(db: &OdbDb) -> Result<bool>;

        fn log_capture_begin(db: &OdbDb);
        fn log_capture_end(db: &OdbDb) -> String;
    }

    // The C++ side (libodb's utl::Logger via a spdlog callback sink) calls this on every log
    // message, so odb's native diagnostics can be centralized through the same events path as
    // the Rust surface. See `set_log_sink` — the actual emitter is installed by the opendb crate
    // (which owns vyges-events), keeping this low-level crate free of that dependency.
    extern "Rust" {
        fn odb_forward_log(level: i32, message: &str);
    }
}

/// The installed forwarder for libodb log messages (level, formatted "[INFO ODB-0127] …" text).
/// `None` until the engine calls [`set_log_sink`]; unset = libodb logs go only to its own stdout.
#[cfg(unix)]
static LOG_SINK: std::sync::OnceLock<fn(i32, &str)> = std::sync::OnceLock::new();

/// Install the sink that receives libodb's native log messages (the opendb crate points this at a
/// `vyges-events` emitter). Idempotent — only the first call wins.
#[cfg(unix)]
pub fn set_log_sink(f: fn(i32, &str)) {
    let _ = LOG_SINK.set(f);
}

/// Called from C++ per libodb log message; forwards to the installed sink (no-op if unset).
#[cfg(unix)]
fn odb_forward_log(level: i32, message: &str) {
    if let Some(f) = LOG_SINK.get() {
        f(level, message);
    }
}

// Second cxx bridge: machine-generated read accessors (scripts/generate-bindings.py). Kept in a
// separate module + bridge so the generator wholly owns it and never edits this hand-written file.
#[cfg(unix)]
mod generated_bridge;
#[cfg(unix)]
pub use generated_bridge::*;

// Third bridge: the generated setter surface, gated behind `gen-write` (L2/write governance).
#[cfg(all(unix, feature = "gen-write"))]
mod generated_write_bridge;
#[cfg(all(unix, feature = "gen-write"))]
pub use generated_write_bridge::*;

#[cfg(unix)]
pub use ffi::{
    add_obstruction, block_compute_core_area, block_name, block_set_core_area,
    block_set_core_area_from_rows, block_set_die_area, bterm_direction, bterm_net, bterm_x, bterm_y, check_3dblox,
    eco_begin, eco_commit, eco_empty, eco_end, eco_undo,
    clear_obstructions, construct_unfolded_model,
    chip_block_create, chip_bump_create, chip_conn_create, chip_create, chip_inst_create,
    chip_net_create, chip_net_add_bump, chip_path_create, chip_region_create,
    chip_region_set_box, alignment_marker_rule_create, set_top_chip,
    bump_master_create, dbu_per_micron, set_dbu_per_micron, lib_from_lef, new_db, tech_create,
    tech_from_lef,
    connect, create_inst, create_net, disconnect, find_master, first_master_name, input_pin,
    inst_master, inst_x, inst_y, log_capture_begin, log_capture_end, net_of, nth_bterm_name, nth_inst_name, nth_iterm_name, num_bterms,
    num_insts, num_iterms, num_nets, num_obstructions, open_db, output_pin, place_bterm,
    swap_master,
    net_is_special, net_sigtype, nth_net_bterm, nth_net_iterm, nth_net_name, num_net_bterms,
    add_track_pattern_x, add_track_pattern_y,
    read_lef,
    num_net_iterms, read_def, set_inst_location, set_inst_orient, total_wire_length, write_db,
    write_def, OdbDb,
    clear_rows, nth_site_name, num_rows, num_sites, row_create, tech_manufacturing_grid,
    site_row_pattern_len, site_row_pattern_orient, site_row_pattern_site,
    block_cut_rows, has_one_site_master, nth_row_name, site_get_class, create_physical_inst, inst_bbox, num_masters, nth_master_name, master_get_type,
    nth_row_bbox, nth_row_site, nth_row_orient, nth_row_direction, destroy_inst,
    inst_shapes, obstruction_boxes, blockage_boxes, swire_boxes, blockage_create, num_blockages, blockage_destroy,
    fill_create, num_fills, clear_fills,
    num_layers, nth_layer_name, layer_direction, track_grid_x, track_grid_y,
    track_patterns_x, track_patterns_y, bterm_constraint_region,
    num_bterm_groups, nth_bterm_group, nth_bterm_group_ordered,
    blocked_regions_for_pins, fixed_bterm_shapes,
    bterm_top_layer_grid, bterm_top_layer_grid_layer, bterm_top_layer_grid_is_rect,
    die_area_polygon, master_obstruction_boxes, master_pin_boxes, mterm_pin_boxes, mpin_boxes, inst_halo,
    net_destroy, iterm_get_id, inst_get_id, layer_find_v55_spacing, layer_find_tw_spacing, layer_get_spacing_for, layer_min_area, tech_via_boxes, tech_via_layer, bpin_layer_boxes, region_boundaries, net_swire_shapes, num_v54_spacing_rules, v54_spacing_rule_adjacent_cuts, v54_spacing_rule_adjacent_spacing, v54_spacing_rule_adjacent_except_same_pgnet, num_array_spacing_rules, array_spacing_rule_cut_class, array_spacing_rule_is_parallel_overlap, array_spacing_rule_is_long_array, array_spacing_rule_array_width, array_spacing_rule_cut_spacing, array_spacing_rule_cuts_spacing, num_cut_spacing_table_rules, cut_spacing_table_max_spacing, cut_spacing_table_spacing, cut_spacing_table_is_center_and_edge, cut_spacing_table_is_center_to_center, num_width_table_rules, width_table_rule_is_wrong_direction, width_table_rule_widths, techvialayerrule_rect, via_create_generated, swire_add_via, swire_add_box, swire_add_box_shaped, swire_clear_routed, bterm_create, bterm_create_pin, bterm_clear_unfixed_bpins, bterm_add_pin_box, bterm_destroy, layer_get_type,
    layer_thickness, mterm_antenna_gate_area, net_wire_area_on_layer, net_wire_perimeter_on_layer,
    nth_net_wire_layer, num_net_wire_layers,
    mterm_antenna_diff_area, layerantenna_diff_pwl_index, layerantenna_diff_pwl_ratio,
    layerantenna_num_diff_pwl,
    iterm_avg_xy, iterm_pin_boxes, layer_name_by_number, net_wire_boxes, net_wire_shapes,
};
