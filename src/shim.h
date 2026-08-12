// SPDX-License-Identifier: Apache-2.0
// Thin cxx-friendly shim over OpenDB. Opaque handle owns a dbDatabase + its Logger.
// Objects are addressed by name (cxx-friendly): no raw odb pointers cross the boundary.
#pragma once
#include <cstddef>
#include <cstdint>
#include <memory>

#include "rust/cxx.h"
#include "odb/db.h"
#include "utl/Logger.h"

// Complete definition here (not a forward decl) so the generated cxx bridge —
// which instantiates std::unique_ptr<OdbDb> — sees a complete type.
struct OdbDb {
  utl::Logger logger;
  odb::dbDatabase* db;
  OdbDb() : db(odb::dbDatabase::create()) { db->setLogger(&logger); }
};

// ---- open / read / write -----------------------------------------------------
std::unique_ptr<OdbDb> open_db(rust::Str path);   // throws -> Rust Result
void write_db(const OdbDb& db, rust::Str path);   // throws -> Rust Result
void write_def(const OdbDb& db, rust::Str path);  // export the block to a DEF file (libodb v1; throws)
void read_def(const OdbDb& db, rust::Str def_path, rust::Str mode);  // import DEF / ApplyDEFTemplate (throws)

// ---- read / inspect ----------------------------------------------------------
rust::String block_name(const OdbDb& db);
std::size_t num_insts(const OdbDb& db);
std::size_t num_nets(const OdbDb& db);
std::size_t num_bterms(const OdbDb& db);
rust::String nth_inst_name(const OdbDb& db, std::size_t i);      // "" if out of range
rust::String first_master_name(const OdbDb& db);                 // any master, "" if none
rust::String find_master(const OdbDb& db, rust::Str substr);     // first master whose name contains substr
rust::String input_pin(const OdbDb& db, rust::Str inst);         // first input-signal pin name
rust::String output_pin(const OdbDb& db, rust::Str inst);        // first output-signal pin name
rust::String inst_master(const OdbDb& db, rust::Str inst);       // instance's master cell name ("" if none)
std::size_t num_iterms(const OdbDb& db, rust::Str inst);         // number of instance pins (iterms)
rust::String nth_iterm_name(const OdbDb& db, rust::Str inst, std::size_t i);  // i-th pin name ("" out of range)
rust::String net_of(const OdbDb& db, rust::Str inst, rust::Str pin);  // net on a pin, "" if none
int32_t inst_x(const OdbDb& db, rust::Str inst);   // instance origin x in DBU (0 if not found)
int32_t inst_y(const OdbDb& db, rust::Str inst);   // instance origin y in DBU (0 if not found)
rust::String nth_bterm_name(const OdbDb& db, std::size_t i);         // block port (bterm), "" if out of range
rust::String bterm_net(const OdbDb& db, rust::Str bterm);            // net on a port, "" if none
int32_t bterm_x(const OdbDb& db, rust::Str bterm);                   // port first-pin x in DBU (0 if none)
int32_t bterm_y(const OdbDb& db, rust::Str bterm);                   // port first-pin y in DBU (0 if none)

// ---- write / ECO primitives (the InsertECOBuffers building blocks) -----------
void create_net(const OdbDb& db, rust::Str name);                       // throws on dup/failure
void create_inst(const OdbDb& db, rust::Str master, rust::Str name);    // throws if master missing
void set_inst_location(const OdbDb& db, rust::Str inst, int32_t x, int32_t y);  // + PLACED
void set_inst_orient(const OdbDb& db, rust::Str inst, rust::Str orient);        // R0/R90/MX/…
void add_obstruction(const OdbDb& db, rust::Str layer, int32_t x1, int32_t y1, int32_t x2, int32_t y2);  // routing/PDN obstruction rect on a layer (throws if layer missing)
std::size_t num_obstructions(const OdbDb& db);
std::size_t clear_obstructions(const OdbDb& db);   // destroy all obstructions, returns the count removed
rust::String bterm_direction(const OdbDb& db, rust::Str bterm);   // port direction: INPUT/OUTPUT/INOUT/…
uint64_t total_wire_length(const OdbDb& db);                      // sum of routed wire length over nets (DBU)

// ---- net traversal + connectivity graph (instrumentation core) ---------------
rust::String nth_net_name(const OdbDb& db, std::size_t i);        // i-th net name ("" out of range)
rust::String net_sigtype(const OdbDb& db, rust::Str net);         // SIGNAL/POWER/GROUND/CLOCK/… ("" if not found)
bool net_is_special(const OdbDb& db, rust::Str net);              // special (power/routing) net?
std::size_t num_net_iterms(const OdbDb& db, rust::Str net);       // instance pins on the net
rust::String nth_net_iterm(const OdbDb& db, rust::Str net, std::size_t i);  // i-th as "inst/pin" ("" out of range)
std::size_t num_net_bterms(const OdbDb& db, rust::Str net);       // block ports on the net
rust::String nth_net_bterm(const OdbDb& db, rust::Str net, std::size_t i);  // i-th port name ("" out of range)

// ---- 3D structural lint (check_3dblox) ---------------------------------------
// Runs odb's own 3D checker over the chiplet assembly: logical connectivity, floating chips,
// overlapping dies, unused internal_ext regions, connection-region overlap and mating-surface
// gap vs connection thickness, bump physical alignment, and alignment markers.
//
// Results are reported the way odb reports every other violation — as dbMarker objects, under a
// "3DBlox" category on the top chip with one sub-category per check — so they are read back
// through the ordinary marker accessors. Returns the total marker count (0 == clean).
//
// This ANNOTATES the in-memory database (it creates the marker categories); it does not modify
// the design. Nothing is persisted unless the caller writes the database out.
std::size_t check_3dblox(const OdbDb& db);

// Rebuild the derived 3D tables (dbUnfoldedChipInst / dbUnfoldedChipRegionInst /
// dbUnfoldedChipBumpInst) from the folded chip hierarchy.
//
// They are DERIVED and never serialised: _dbDatabase::operator>> calls this on read. Nothing
// rebuilds them when a dbChipInst moves or is reoriented, so every unfolded query answers from
// the placement before the move until this is called. No error, no warning, just stale geometry.
//
// MEASURED, because the obvious assumption is wrong: check_3dblox above rebuilds the model
// itself, so the LINTER is not affected. This is for callers that read the unfolded accessors
// (surface_z, effective_side, bump global positions) after a move.
// ---- 3D / chiplet construction -----------------------------------------------
// odb's dbChip* creation statics. Not generated: the signatures are heterogeneous (dbChipConn
// takes two std::vector<dbChipInst*> paths), so these follow the hand-written create_inst /
// create_net pattern. Everything is addressed by name; no odb pointer crosses the boundary.
// All throw on failure -> Rust Result.
//
// Without these the 3D surface is read-only in practice: a design could be inspected and its
// chips moved, but not brought into existence at all.
void chip_create(const OdbDb& db, rust::Str name, rust::Str tech, rust::Str chip_type);  // tech "" = default
void chip_block_create(const OdbDb& db, rust::Str chip, rust::Str name);
void chip_inst_create(const OdbDb& db, rust::Str parent_chip, rust::Str master_chip, rust::Str name);
void chip_region_create(const OdbDb& db, rust::Str chip, rust::Str name, rust::Str side, rust::Str layer);
void chip_region_set_box(const OdbDb& db, rust::Str chip, rust::Str region, int32_t x1, int32_t y1, int32_t x2, int32_t y2);
void chip_bump_create(const OdbDb& db, rust::Str chip, rust::Str region, rust::Str inst);
void chip_conn_create(const OdbDb& db, rust::Str name, rust::Str parent_chip,
                      rust::Str top_inst, rust::Str top_region,
                      rust::Str bottom_inst, rust::Str bottom_region, int32_t thickness);
void chip_net_create(const OdbDb& db, rust::Str chip, rust::Str name);
void chip_path_create(const OdbDb& db, rust::Str chip, rust::Str name);
std::unique_ptr<OdbDb> new_db();   // an empty database, for building rather than reading
void tech_from_lef(const OdbDb& db, rust::Str name, rust::Str lef_path);
void lib_from_lef(const OdbDb& db, rust::Str lib_name, rust::Str tech_name, rust::Str lef_path);
void bump_master_create(const OdbDb& db, rust::Str name, int32_t width, int32_t height);
void tech_create(const OdbDb& db, rust::Str name);
int32_t dbu_per_micron(const OdbDb& db);
void set_dbu_per_micron(const OdbDb& db, int32_t dbu);
void chip_net_add_bump(const OdbDb& db, rust::Str chip, rust::Str net, rust::Str chip_inst, rust::Str region, std::size_t bump_index);
void alignment_marker_rule_create(const OdbDb& db, rust::Str master_a, rust::Str master_b, int32_t tolerance);
void set_top_chip(const OdbDb& db, rust::Str chip);

void construct_unfolded_model(const OdbDb& db);

// ---- resize / Vt-swap --------------------------------------------------------
// Replace an instance's library cell in place — the setup-repair move (upsize a critical
// driver), and equally the Vt-swap and downsize moves.
//
// Returns false when odb refuses the swap because the instance is bound to a block hierarchy.
// THROWS when the instance or master is unknown, and when the instance is marked don't-touch
// (odb raises there rather than returning false, and a don't-touch instance being silently
// resized is exactly the failure that flag exists to prevent).
//
// odb DOES check pin compatibility: the new master must have the same number of MTerms with
// exactly the same names (sorted pairwise strcmp), else it warns and returns false. So a swap
// cannot silently strand connections.
//
// It does NOT check LOGICAL equivalence — same pins does not mean same function. Picking a
// replacement that actually computes the same thing is the caller's problem.
//
// The swap is journaled (dbJournal kSwapObject), so it rolls back with the ECO journal below.
bool swap_master(const OdbDb& db, rust::Str inst, rust::Str master);

// ---- ECO journal: speculative edits with a real undo -------------------------
// odb records block edits (create/delete object, connect/disconnect, swap, field updates) into
// a journal, so a batch of ECO changes can be rolled back. This is what lets a timing-driven
// optimizer TRY a fix, re-time, and put the design back if it did not help.
//
// Sequence: eco_begin -> ...edits... -> eco_commit | eco_undo. Journals nest (odb keeps a
// stack), and undo/commit work whether or not eco_end was called first.
//
// Upstream's own note is worth heeding: the mechanism was built for replicating deltas from a
// "remote" database onto an unchanged "master". We use it for local speculate/rollback, which
// is within its semantics, but only edits the journal actually records can be undone.
void eco_begin(const OdbDb& db);   // start recording (throws if there is no top block)
void eco_end(const OdbDb& db);     // stop recording, push onto the ECO stack
void eco_commit(const OdbDb& db);  // keep the recorded changes
void eco_undo(const OdbDb& db);    // roll the recorded changes back
bool eco_empty(const OdbDb& db);   // true when the current ECO recorded nothing

// Capture libodb diagnostics instead of letting them hit stdout; end returns the captured text.
void log_capture_begin(const OdbDb& db);
rust::String log_capture_end(const OdbDb& db);

void place_bterm(const OdbDb& db, rust::Str bterm, rust::Str layer, int32_t x1, int32_t y1, int32_t x2, int32_t y2);  // place a port pin box on a layer
void connect(const OdbDb& db, rust::Str inst, rust::Str pin, rust::Str net);    // iterm -> net
void disconnect(const OdbDb& db, rust::Str inst, rust::Str pin);               // iterm -> (none)

// ---- antenna inputs (odb substrate) ------------------------------------------
// The numerator and denominator of the antenna ratio, read off the ROUTED database — the
// substrate OpenROAD's `ant` uses, and the one where RepairAntennas can still act. (The
// GDS-substrate antenna check in vyges-drc is the same ratio computed post-stream.)
// Consumed by vyges-ant; see vyges-tools-internal/docs/loom/flow-ir-crate-roadmap.md.
//
// Metal is grouped by routing layer because the charge model is per-layer and cumulative:
// a net legal on its own worst layer can still violate the cumulative ratio. Callers that
// only sum are computing something else and should say so.
//
// v0 bound, stated rather than discovered later: shapes are accumulated as raw rectangles,
// so overlapping metal on one layer is DOUBLE-COUNTED. Router output on a single layer
// rarely self-overlaps except at junctions, and the error is conservative (over-reports
// area, hence over-reports the ratio, hence never hides a violation) — but it is a real
// difference from a union-area computation, and correlation against `check_antennas` will
// show it. Fix by unioning per layer if it proves material.
std::size_t num_net_wire_layers(const OdbDb& db, rust::Str net);                 // routing layers this net has metal on
rust::String nth_net_wire_layer(const OdbDb& db, rust::Str net, std::size_t i);  // i-th such layer name ("" out of range)
int64_t net_wire_area_on_layer(const OdbDb& db, rust::Str net, rust::Str layer);       // metal area on that layer (DBU^2)
int64_t net_wire_perimeter_on_layer(const OdbDb& db, rust::Str net, rust::Str layer);  // metal perimeter (DBU); side area = perimeter * layer_thickness
int32_t layer_thickness(const OdbDb& db, rust::Str layer);                       // DBU, 0 if the LEF does not state one
double mterm_antenna_gate_area(const OdbDb& db, rust::Str master, rust::Str term);  // pin-model gate area (denominator); 0.0 if the pin has no model
double mterm_antenna_diff_area(const OdbDb& db, rust::Str master, rust::Str term);  // pin-model diffusion area; indexes the diff-ratio PWL below

// ---- antenna diff-ratio PWL --------------------------------------------------
// LEF states antenna limits two ways: plain ratios (ANTENNAAREARATIO), reachable through the
// generated `layerantenna_get_*` accessors, and DIFFUSION-DEPENDENT ratios
// (ANTENNADIFFAREARATIO), where the limit is a piecewise-linear function of the diffusion area
// connected to the net. sky130 states ONLY the latter — `dbTechLayerAntennaRule::isValid()` is
// false on every sky130 routing layer, and that predicate is exactly "does any plain ratio
// exceed zero" — so without these accessors there is nothing to check on sky130 at all.
//
// They return odb's `pwl_pair` (two parallel `std::vector<double>&`), which the schema
// generator skips because it is not a cxx-friendly scalar. Hence a hand shim.
//
// odb's own convention for the vectors, which callers must honour:
//   indices.size() == 0  -> the ratio is UNSET
//   indices.size() == 1  -> a single constant ratio, NOT a piecewise-linear curve
//
// `which` selects the curve: "par" | "car" | "psr" | "csr" | "area_diff_reduce" |
// "gate_plus_diff". An unrecognised value throws rather than returning zero, so a typo
// surfaces as an error instead of a silently empty rule that reads like "no limit".
// ---- routed-wire shape graph -------------------------------------------------
// The per-layer area accessors above answer "how much metal does this NET have on this layer",
// which is the wrong question for an antenna check: the charge a given gate collects comes only
// from the metal reachable FROM THAT GATE over layers at or below the one being deposited. Two
// gates on one net can sit on different branches and see very different metal until a higher
// layer joins them. Answering per gate needs the shapes plus their connectivity, not a sum.
//
// Returned flat so one call yields a whole net — a per-shape accessor would re-walk the wire on
// every query, which is quadratic on the nets that matter most (the big ones).
//
// Layout: 8 entries per shape, in wire order.
//   [0] layer number (odb dbTechLayer::getNumber), -1 if none
//   [1..4] x0, y0, x1, y1 in DBU
//   [5] 1 when the shape is a via (cut geometry), else 0
//   [6] via bottom layer number, -1 when not a via
//   [7] via top layer number, -1 when not a via
// Vias carry their two layer numbers because they are what makes the graph three-dimensional:
// without them every layer would look like a separate net.
rust::Vec<std::int64_t> net_wire_shapes(const OdbDb& db, rust::Str net);
rust::String layer_name_by_number(const OdbDb& db, std::int64_t number);  // "" if unknown

// Every routed box of a net, with vias DECOMPOSED onto the layers they occupy.
//
// `net_wire_shapes` above reports a via as one bounding box tagged with the pair it joins. That
// loses the thing that matters for an antenna check: a via is not a box in mid-air, it is a cut
// plus an enclosure on the layer below and another on the layer above. `dbShape::getViaBoxes`
// gives all three, and OpenROAD's wiresToPolygonSetMap files each on its OWN layer.
//
// This is load-bearing, not cosmetic. Standard-cell pins sit on the lowest routing layer (li1 on
// sky130) and a net routed on met1 and above has no li1 WIRE at all — its only li1 metal is the
// enclosure of the li1->met1 via. Without decomposition there is no geometry on the layer where
// pins live, so nothing can attach to them and nothing can be cut by them. Measured: a net whose
// terminals all reported "attached to nothing" until vias were decomposed.
//
// Layout: 7 entries per box — [layer number, x0, y0, x1, y1, is_routing_layer, came_from_via].
// `is_routing_layer` separates metal (ratios apply) from cut layers (their own ratios apply);
// `came_from_via` is kept for diagnosis, since via enclosures behave like wire but arrive
// differently.
rust::Vec<std::int64_t> net_wire_boxes(const OdbDb& db, rust::Str net);

// Where an instance pin sits, for anchoring a gate to the shape graph. Returns false when odb
// cannot place the pin, which the caller must treat as "cannot attribute", never as (0,0).
bool iterm_avg_xy(const OdbDb& db, rust::Str inst, rust::Str pin, std::int32_t& x, std::int32_t& y);

// The pin's actual metal, in placed coordinates: every ROUTING-layer box of every dbMPin of the
// terminal, with the instance's transform applied.
//
// A terminal's average point says where a pin roughly is; these say what it physically touches,
// which is what decides the conductor it joins. Matching a pin to routing by proximity instead
// merges conductors that are electrically separate — measured on a real block, it put a gate and
// its protection diode on one conductor and suppressed a real violation.
//
// Non-routing (cut) boxes are skipped: a pin joins the wire through metal, and OpenROAD's
// AntennaChecker::saveGates filters the same way.
//
// Layout: 5 entries per box — [layer number, x0, y0, x1, y1], DBU.
rust::Vec<std::int64_t> iterm_pin_boxes(const OdbDb& db, rust::Str inst, rust::Str pin);

std::size_t layerantenna_num_diff_pwl(const OdbDb& db, rust::Str layer, rust::Str which);
double layerantenna_diff_pwl_index(const OdbDb& db, rust::Str layer, rust::Str which, std::size_t i);  // diffusion area at point i
double layerantenna_diff_pwl_ratio(const OdbDb& db, rust::Str layer, rust::Str which, std::size_t i);  // ratio limit at point i

// ---- floorplan writes (vyges-ifp) ------------------------------------------------------
// Hand-written, not generated: the generator emits scalar setters, while `setDieArea` takes a
// `Rect`, `setCoreArea` a `Polygon`, and `dbRow::create` is a static factory. Same reason the
// dbChip*::create surface is hand-written.
void block_set_die_area(const OdbDb& db, int32_t x1, int32_t y1, int32_t x2, int32_t y2);
void block_set_core_area(const OdbDb& db, int32_t x1, int32_t y1, int32_t x2, int32_t y2);
// Replace the core area with what the ROWS actually cover — upstream's
// `setCoreArea(computeCoreArea())`. Polygon-to-polygon, so nothing is lost to a bbox.
void block_set_core_area_from_rows(const OdbDb& db);
// Bounding box of `computeCoreArea()` WITHOUT storing it: [x_min, y_min, x_max, y_max], DBU.
// Empty when there are no rows.
rust::Vec<std::int32_t> block_compute_core_area(const OdbDb& db);

// Manufacturing grid in DBU; 0 when the tech does not declare one (the "no snap" case).
int32_t tech_manufacturing_grid(const OdbDb& db);

// Site width/height and hybrid-ness are already generated (`site_get_width` / `_height` /
// `site_is_hybrid`) — by name, exactly this shape. Do not add them here.

// `orient` is a dbOrientType spelling (R0, MX, MY, R180, ...); `direction` is HORIZONTAL or
// VERTICAL. Throws when the site is not found in any library.
void row_create(const OdbDb& db, rust::Str name, rust::Str site, int32_t x, int32_t y,
                rust::Str orient, rust::Str direction, int32_t num_sites, int32_t spacing);
// Site enumeration — `ifp` validates `-site`/`-additional_sites` against what the libraries
// actually define, and an engine cannot ask by name for a name it does not know.
std::size_t num_sites(const OdbDb& db);
rust::String nth_site_name(const OdbDb& db, std::size_t i);
std::size_t num_rows(const OdbDb& db);
std::size_t clear_rows(const OdbDb& db);  // upstream clears rows before rebuilding them

// A hybrid site's ROW PATTERN — the repeating sequence of (site, orientation) that tiles the
// core, where a plain site would repeat one height. Whether a pattern exists is already
// generated (`site_has_row_pattern`), but its CONTENTS cannot be: getRowPattern() returns a
// vector of structs, which the schema generator has no shape for. Hence these three, indexed
// like `nth_site_name` rather than returned as one vector, so the bridge stays scalar.
// A site with no pattern has length 0; an out-of-range index throws rather than returning "".
std::size_t site_row_pattern_len(const OdbDb& db, rust::Str site);
rust::String site_row_pattern_site(const OdbDb& db, rust::Str site, std::size_t i);
rust::String site_row_pattern_orient(const OdbDb& db, rust::Str site, std::size_t i);

// Row cutting around macros — `odb::cutRows` from odb/util.h, NOT reimplemented. Cutting rows to
// clear placed macros is odb's own algorithm on odb's own data; the engine's job is only to
// decide WHICH instances are blockages and to report the ones it skipped. `blockage_insts` names
// those instances; each is resolved to its bounding box here because a dbBox* cannot cross the
// bridge. An unknown instance name throws rather than silently cutting around nothing.
void block_cut_rows(const OdbDb& db, int32_t min_row_width,
                    rust::Slice<const rust::String> blockage_insts,
                    int32_t halo_x, int32_t halo_y);

// True when the technology has a single-site-width master — the condition under which upstream
// allows tapcells to leave one-site gaps.
bool has_one_site_master(const OdbDb& db);

// Row enumeration. `row_get_*` accessors are generated and address a row BY NAME, but nothing
// enumerates the names — and a floorplan's rows are not named predictably enough to guess.
rust::String nth_row_name(const OdbDb& db, std::size_t i);

// A site's class (CORE, PAD, ...). Rows on PAD sites sit outside the core and are not part of
// the region standard cells occupy, so `tap` has to be able to tell them apart.
rust::String site_get_class(const OdbDb& db, rust::Str site);

// Create a PHYSICAL-ONLY instance — a cell that exists in the layout but not the netlist, which
// is what every tap, endcap and filler is. `create_inst` makes an ordinary netlist instance;
// using it for physical cells would put them in the hierarchy, where nothing should see them.
void create_physical_inst(const OdbDb& db, rust::Str master, rust::Str name);

// An instance's bounding box in placed coordinates: [x_min, y_min, x_max, y_max], empty when the
// instance is unknown. Not derivable from origin + master size, because orientation decides which
// way the master extends from its origin.
rust::Vec<int32_t> inst_bbox(const OdbDb& db, rust::Str inst);

// Master enumeration + LEF master type. `find_master` matches a name substring, which cannot
// answer "which cell is the bottom-left endcap?" -- that is a question about the master's TYPE
// (ENDCAP_LEF58_LEFTBOTTOMCORNER and friends), and nothing enumerated masters at all.
// The type STRING is returned so the matching stays in the engine, where the policy belongs.
std::size_t num_masters(const OdbDb& db);
rust::String nth_master_name(const OdbDb& db, std::size_t i);
rust::String master_get_type(const OdbDb& db, rust::Str master);

// Row geometry BY INDEX. ⚠️ ROW NAMES ARE NOT UNIQUE. `cut_rows` splits a row into pieces that
// can collide with another family's names, and a design read from DEF may simply repeat them --
// 699 rows over 692 distinct names in one upstream test case. Every generated `row_get_*`
// accessor resolves by name and therefore returns the FIRST match, silently reading one row's
// geometry for another and losing the rest. Anything that walks all rows must use these.
rust::Vec<int32_t> nth_row_bbox(const OdbDb& db, std::size_t i);   // [x_min,y_min,x_max,y_max]
rust::String nth_row_site(const OdbDb& db, std::size_t i);
rust::String nth_row_orient(const OdbDb& db, std::size_t i);

// Destroy one instance by name. The primitive, not a bulk "delete everything matching" -- which
// cells to remove is engine policy, and a shim that took a pattern would put that policy here.
// Throws when the instance does not exist, so a typo cannot read as "nothing to remove".
void destroy_inst(const OdbDb& db, rust::Str inst);

// ---- density fill (vyges-fin) -------------------------------------------------
// Every shape of every PLACED instance, flattened to 5 i64 each
// (layer_number, x_min, y_min, x_max, y_max), in placed coordinates.
//
// This is what stops fill landing on top of the design: a standard cell's pins and internal metal
// are non-fill area, and nothing else in the bridge exposes them. Flat because one call per shape
// would cross the boundary millions of times on a real block.
rust::Vec<int64_t> inst_shapes(const OdbDb& db);

// Routing/PDN obstruction rectangles, 5 i64 each (layer_number, x_min, y_min, x_max, y_max).
// `num_obstructions` counts them; this is the geometry.
rust::Vec<int64_t> obstruction_boxes(const OdbDb& db);

// Create a fill rectangle. `mask` 0 means "no mask" (single-mask layers).
void fill_create(const OdbDb& db, bool needs_opc, uint32_t mask, rust::Str layer,
                 int32_t x1, int32_t y1, int32_t x2, int32_t y2);
std::size_t num_fills(const OdbDb& db);
std::size_t clear_fills(const OdbDb& db);   // returns the count removed

// Layer enumeration + routing direction. `layer_get_*` accessors are generated and address a
// layer by name, but nothing enumerated them, and the DIRECTION (which decides how fill shapes
// are oriented and where line-end spacing applies) has no generated accessor at all.
std::size_t num_layers(const OdbDb& db);
rust::String nth_layer_name(const OdbDb& db, std::size_t i);
rust::String layer_direction(const OdbDb& db, rust::Str layer);   // HORIZONTAL/VERTICAL/NONE

// Every SPECIAL-wire box, vias decomposed onto the layers they occupy: 5 i64 each
// (layer_number, x_min, y_min, x_max, y_max).
//
// Special wires are the power grid — rails and straps — and they are a separate collection from
// routed signal wires. Density fill that misses them fills straight over the PDN.
rust::Vec<int64_t> swire_boxes(const OdbDb& db);

// Routing track coordinates for a layer — the positions pins and wires may legally sit on.
//
// These are the foundation of pin placement: every legal pin slot is a track. `getGridX` fills a
// std::vector by reference, which the schema generator cannot express, so nothing reached them.
// An empty result means the layer has no track grid, which is an answer, not an error.
rust::Vec<int32_t> track_grid_x(const OdbDb& db, rust::Str layer);
rust::Vec<int32_t> track_grid_y(const OdbDb& db, rust::Str layer);

// Track PATTERNS for a layer: 3 i32 each (origin, line_count, step), one triple per pattern.
//
// The expanded coordinates above answer "where are the tracks"; pin placement needs the pattern
// itself, because its slot arithmetic indexes tracks by number from the pattern's origin and a
// layer may carry several patterns with different pitches. `getGridPatternX` returns its three
// values through out-parameters, so the schema generator cannot express it.
rust::Vec<int32_t> track_patterns_x(const OdbDb& db, rust::Str layer);
rust::Vec<int32_t> track_patterns_y(const OdbDb& db, rust::Str layer);

// A block port's CONSTRAINT REGION: 4 i32 (x0, y0, x1, y1), or empty when the port has none.
//
// This is how `set_io_pin_constraint -region` reaches an engine: the constraint is stored on the
// port in the database, not passed as a command argument, so it survives a write/read cycle and
// has to be read back rather than re-supplied.
//
// A **degenerate** rectangle — zero width or zero height — is an interval on one die edge, which
// is the common case. A rectangle with real area is a top-layer region instead. The distinction is
// the rectangle's own shape, so callers must check it rather than assume.
//
// `getConstraintRegion` returns `std::optional<Rect>`, which the schema generator has no shape for.
rust::Vec<int32_t> bterm_constraint_region(const OdbDb& db, rust::Str bterm);

// PIN GROUPS declared on the block: `set_io_pin_constraint -group` records a set of ports that
// must be placed on ADJACENT slots, optionally in the order given.
//
// Three accessors rather than one, because a group is a list of names plus a flag and
// `getBTermGroups()` returns a `std::vector` of a struct holding both — a shape the schema
// generator has nothing to say about.
std::size_t num_bterm_groups(const OdbDb& db);
rust::Vec<rust::String> nth_bterm_group(const OdbDb& db, std::size_t i);
bool nth_bterm_group_ordered(const OdbDb& db, std::size_t i);

// Regions where NO pin may be placed: `exclude_io_pin_region -region edge:lo-hi`, stored on the
// block. Four i32 per rectangle (x0, y0, x1, y1); a degenerate rectangle is an interval on an edge,
// exactly as a constraint region is.
//
// `getBlockedRegionsForPins()` returns `const std::vector<Rect>&` — a by-reference container, so
// the schema generator leaves it unimplemented.
rust::Vec<int32_t> blocked_regions_for_pins(const OdbDb& db);

// Every metal shape of every port whose placement is already FIXED: 5 i64 each
// (layer number, x0, y0, x1, y1).
//
// A fixed port is not ours to move, and the slots its metal covers are not ours to use either —
// `place_pin` puts a port at an explicit location, and a later pin landing on the same slot would
// short to it. Resolve layer numbers with `layer_name_by_number`.
rust::Vec<int64_t> fixed_bterm_shapes(const OdbDb& db);
