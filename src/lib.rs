// SPDX-License-Identifier: Apache-2.0
//! `vyges-opendb-lib` — low-level FFI to OpenROAD's OpenDB (`libodb`).
//!
//! Read + write-path surface over a standalone libodb (no tcl/swig/engines), proven on
//! linux/x86_64, linux/arm64, and macOS/Apple Silicon. Objects are addressed by name so
//! no raw odb pointers cross the FFI boundary. The write primitives are the building
//! blocks for the ECO applier (`InsertECOBuffers`). The safe, ergonomic wrappers live in
//! the sibling crate `vyges-opendb`.

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
    add_obstruction, block_name, bterm_direction, bterm_net, bterm_x, bterm_y, check_3dblox,
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
    num_net_iterms, read_def, set_inst_location, set_inst_orient, total_wire_length, write_db,
    write_def, OdbDb,
};
