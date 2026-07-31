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
