// SPDX-License-Identifier: Apache-2.0
#include "shim.h"

#include "lint3d.h"      // 3D structural lint (compiled into libodb; see that header)
#include "odb/defin.h"   // LEF/DEF I/O (libodb v1)
#include "odb/defout.h"
#include "spdlog/sinks/callback_sink.h"   // forward libodb's utl::Logger -> Rust -> vyges-events
#include "vyges-opendb-lib/src/lib.rs.h"  // odb_forward_log (extern "Rust")

#include <cstdint>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

using odb::dbBlock;
using odb::dbBox;
using odb::dbBPin;
using odb::dbBTerm;
using odb::dbInst;
using odb::dbITerm;
using odb::dbMaster;
using odb::dbNet;
using odb::dbObstruction;
using odb::dbTech;
using odb::dbTechLayer;

static std::string s(rust::Str v) { return std::string(v.data(), v.size()); }

static dbBlock* block_of(const OdbDb& h) {
  odb::dbChip* chip = h.db->getChip();
  return chip ? chip->getBlock() : nullptr;
}
static dbBlock* require_block(const OdbDb& h) {
  dbBlock* b = block_of(h);
  if (!b) throw std::runtime_error("vyges-opendb: no top block");
  return b;
}
static dbInst* require_inst(const OdbDb& h, rust::Str inst) {
  dbInst* i = require_block(h)->findInst(s(inst).c_str());
  if (!i) throw std::runtime_error("vyges-opendb: inst not found: " + s(inst));
  return i;
}
static dbITerm* require_iterm(const OdbDb& h, rust::Str inst, rust::Str pin) {
  dbITerm* t = require_inst(h, inst)->findITerm(s(pin).c_str());
  if (!t) throw std::runtime_error("vyges-opendb: pin not found: " + s(inst) + "/" + s(pin));
  return t;
}

// Route every libodb log message (already formatted by utl::Logger as "[INFO ODB-0127] …") to the
// Rust forwarder, which hands it to the installed events sink. Added alongside utl's default stdout
// sink (so nothing that already worked breaks); the forwarder is a no-op until the engine installs
// a sink, so this is free when unused.
static void install_log_forwarding(utl::Logger& logger) {
  logger.addSink(std::make_shared<spdlog::sinks::callback_sink_mt>(
      [](const spdlog::details::log_msg& m) {
        odb_forward_log(static_cast<int32_t>(m.level),
                        rust::Str(m.payload.data(), m.payload.size()));
      }));
}

// ---- open / write ------------------------------------------------------------
std::unique_ptr<OdbDb> open_db(rust::Str path) {
  auto h = std::make_unique<OdbDb>();
  install_log_forwarding(h->logger);
  std::string p = s(path);
  std::ifstream in(p, std::ios::binary);
  if (!in) throw std::runtime_error("vyges-opendb: cannot open " + p);
  h->db->read(in);
  return h;
}
void write_db(const OdbDb& h, rust::Str path) {
  std::string p = s(path);
  std::ofstream out(p, std::ios::binary);
  if (!out) throw std::runtime_error("vyges-opendb: cannot write " + p);
  h.db->write(out);
}

// ---- read / inspect ----------------------------------------------------------
rust::String block_name(const OdbDb& h) {
  dbBlock* b = block_of(h);
  return rust::String(b ? b->getName() : std::string());
}
std::size_t num_insts(const OdbDb& h)  { dbBlock* b = block_of(h); return b ? b->getInsts().size()  : 0; }
std::size_t num_nets(const OdbDb& h)   { dbBlock* b = block_of(h); return b ? b->getNets().size()   : 0; }
std::size_t num_bterms(const OdbDb& h) { dbBlock* b = block_of(h); return b ? b->getBTerms().size() : 0; }

rust::String nth_inst_name(const OdbDb& h, std::size_t i) {
  dbBlock* b = block_of(h);
  if (!b) return rust::String();
  std::size_t k = 0;
  for (dbInst* inst : b->getInsts()) {
    if (k++ == i) return rust::String(inst->getName());
  }
  return rust::String();
}
rust::String first_master_name(const OdbDb& h) {
  for (odb::dbLib* lib : h.db->getLibs())
    for (dbMaster* m : lib->getMasters())
      return rust::String(m->getName());
  return rust::String();
}
rust::String find_master(const OdbDb& h, rust::Str substr) {
  std::string want = s(substr);
  for (odb::dbLib* lib : h.db->getLibs())
    for (dbMaster* m : lib->getMasters()) {
      std::string n = m->getName();
      if (n.find(want) != std::string::npos) return rust::String(n);
    }
  return rust::String();
}
// Inspect functions are total (never throw): return "" on a missing block/inst/pin.
rust::String input_pin(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  if (!i) return rust::String();
  for (dbITerm* t : i->getITerms())
    if (t->isInputSignal()) return rust::String(t->getMTerm()->getName());
  return rust::String();
}
rust::String output_pin(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  if (!i) return rust::String();
  for (dbITerm* t : i->getITerms())
    if (t->isOutputSignal()) return rust::String(t->getMTerm()->getName());
  return rust::String();
}
rust::String inst_master(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  dbMaster* m = i ? i->getMaster() : nullptr;
  return rust::String(m ? m->getName() : std::string());
}
std::size_t num_iterms(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  return i ? i->getITerms().size() : 0;
}
rust::String nth_iterm_name(const OdbDb& h, rust::Str inst, std::size_t idx) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  if (!i) return rust::String();
  std::size_t k = 0;
  for (dbITerm* t : i->getITerms())
    if (k++ == idx) return rust::String(t->getMTerm()->getName());
  return rust::String();
}
rust::String net_of(const OdbDb& h, rust::Str inst, rust::Str pin) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  dbITerm* t = i ? i->findITerm(s(pin).c_str()) : nullptr;
  dbNet* n = t ? t->getNet() : nullptr;
  return rust::String(n ? n->getName() : std::string());
}
int32_t inst_x(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  if (!i) return 0;
  int x = 0, y = 0;
  i->getLocation(x, y);
  return x;
}
rust::String nth_bterm_name(const OdbDb& h, std::size_t i) {
  dbBlock* b = block_of(h);
  if (!b) return rust::String();
  std::size_t k = 0;
  for (dbBTerm* bt : b->getBTerms()) {
    if (k++ == i) return rust::String(bt->getName());
  }
  return rust::String();
}
rust::String bterm_net(const OdbDb& h, rust::Str bterm) {
  dbBlock* b = block_of(h);
  dbBTerm* bt = b ? b->findBTerm(s(bterm).c_str()) : nullptr;
  dbNet* n = bt ? bt->getNet() : nullptr;
  return rust::String(n ? n->getName() : std::string());
}
int32_t bterm_x(const OdbDb& h, rust::Str bterm) {
  dbBlock* b = block_of(h);
  dbBTerm* bt = b ? b->findBTerm(s(bterm).c_str()) : nullptr;
  int x = 0, y = 0;
  if (bt) bt->getFirstPinLocation(x, y);
  return x;
}
int32_t bterm_y(const OdbDb& h, rust::Str bterm) {
  dbBlock* b = block_of(h);
  dbBTerm* bt = b ? b->findBTerm(s(bterm).c_str()) : nullptr;
  int x = 0, y = 0;
  if (bt) bt->getFirstPinLocation(x, y);
  return y;
}
int32_t inst_y(const OdbDb& h, rust::Str inst) {
  dbBlock* b = block_of(h);
  dbInst* i = b ? b->findInst(s(inst).c_str()) : nullptr;
  if (!i) return 0;
  int x = 0, y = 0;
  i->getLocation(x, y);
  return y;
}

// ---- write / ECO primitives --------------------------------------------------
void create_net(const OdbDb& h, rust::Str name) {
  dbBlock* b = require_block(h);
  if (b->findNet(s(name).c_str())) throw std::runtime_error("vyges-opendb: net exists: " + s(name));
  if (!dbNet::create(b, s(name).c_str())) throw std::runtime_error("vyges-opendb: create_net failed: " + s(name));
}
void create_inst(const OdbDb& h, rust::Str master, rust::Str name) {
  dbBlock* b = require_block(h);
  dbMaster* m = h.db->findMaster(s(master).c_str());
  if (!m) throw std::runtime_error("vyges-opendb: master not found: " + s(master));
  if (!dbInst::create(b, m, s(name).c_str())) throw std::runtime_error("vyges-opendb: create_inst failed: " + s(name));
}
void set_inst_location(const OdbDb& h, rust::Str inst, int32_t x, int32_t y) {
  dbInst* i = require_inst(h, inst);
  i->setLocation(x, y);
  i->setPlacementStatus(odb::dbPlacementStatus::PLACED);
}
void set_inst_orient(const OdbDb& h, rust::Str inst, rust::Str orient) {
  // dbOrientType parses "R0"/"R90"/"R180"/"R270"/"MX"/"MY"/"MXR90"/"MYR90".
  require_inst(h, inst)->setOrient(odb::dbOrientType(s(orient).c_str()));
}
void add_obstruction(const OdbDb& h, rust::Str layer, int32_t x1, int32_t y1, int32_t x2,
                     int32_t y2) {
  dbBlock* b = require_block(h);
  dbTech* tech = b->getTech();
  dbTechLayer* l = tech ? tech->findLayer(s(layer).c_str()) : nullptr;
  if (!l) throw std::runtime_error("vyges-opendb: tech layer not found: " + s(layer));
  dbObstruction::create(b, l, x1, y1, x2, y2);
}
std::size_t num_obstructions(const OdbDb& h) {
  dbBlock* b = block_of(h);
  return b ? b->getObstructions().size() : 0;
}
std::size_t clear_obstructions(const OdbDb& h) {
  dbBlock* b = block_of(h);
  if (!b) return 0;
  std::vector<dbObstruction*> obs(b->getObstructions().begin(), b->getObstructions().end());
  for (dbObstruction* o : obs) dbObstruction::destroy(o);
  return obs.size();
}
rust::String bterm_direction(const OdbDb& h, rust::Str bterm) {
  dbBlock* b = block_of(h);
  dbBTerm* bt = b ? b->findBTerm(s(bterm).c_str()) : nullptr;
  if (!bt) return rust::String();
  return rust::String(std::string(bt->getIoType().getString()));  // INPUT/OUTPUT/INOUT/…
}
std::uint64_t total_wire_length(const OdbDb& h) {
  dbBlock* b = block_of(h);
  if (!b) return 0;
  std::uint64_t total = 0;
  for (dbNet* n : b->getNets()) {
    odb::dbWire* w = n->getWire();
    if (w) total += w->getLength();
  }
  return total;
}
// ---- net traversal + connectivity graph (instrumentation core) ---------------
static dbNet* find_net(const OdbDb& h, rust::Str net) {
  dbBlock* b = block_of(h);
  return b ? b->findNet(s(net).c_str()) : nullptr;
}
rust::String nth_net_name(const OdbDb& h, std::size_t i) {
  dbBlock* b = block_of(h);
  if (!b) return rust::String();
  std::size_t k = 0;
  for (dbNet* n : b->getNets()) {
    if (k++ == i) return rust::String(n->getName());
  }
  return rust::String();
}
rust::String net_sigtype(const OdbDb& h, rust::Str net) {
  dbNet* n = find_net(h, net);
  return rust::String(n ? std::string(n->getSigType().getString()) : std::string());  // SIGNAL/POWER/…
}
bool net_is_special(const OdbDb& h, rust::Str net) {
  dbNet* n = find_net(h, net);
  return n ? n->isSpecial() : false;
}
std::size_t num_net_iterms(const OdbDb& h, rust::Str net) {
  dbNet* n = find_net(h, net);
  return n ? n->getITerms().size() : 0;
}
rust::String nth_net_iterm(const OdbDb& h, rust::Str net, std::size_t i) {
  dbNet* n = find_net(h, net);
  if (!n) return rust::String();
  std::size_t k = 0;
  for (dbITerm* t : n->getITerms()) {
    if (k++ == i) return rust::String(t->getInst()->getName() + "/" + t->getMTerm()->getName());
  }
  return rust::String();
}
std::size_t num_net_bterms(const OdbDb& h, rust::Str net) {
  dbNet* n = find_net(h, net);
  return n ? n->getBTerms().size() : 0;
}
rust::String nth_net_bterm(const OdbDb& h, rust::Str net, std::size_t i) {
  dbNet* n = find_net(h, net);
  if (!n) return rust::String();
  std::size_t k = 0;
  for (dbBTerm* bt : n->getBTerms()) {
    if (k++ == i) return rust::String(bt->getName());
  }
  return rust::String();
}
void write_def(const OdbDb& h, rust::Str path) {
  dbBlock* b = require_block(h);
  // the OdbDb owns its logger; DefOut wants a non-const Logger* (logically mutable, like h.db).
  odb::DefOut writer(const_cast<utl::Logger*>(&h.logger));
  if (!writer.writeBlock(b, s(path).c_str())) {
    throw std::runtime_error("vyges-opendb: DEF write failed: " + s(path));
  }
}
// Read a DEF into the db. mode: "default" (design from scratch), "floorplan" (update existing
// COMPONENTS/PINS/DIEAREA/TRACKS/ROWS/NETS — this is Odb.ApplyDEFTemplate), "incremental"
// (update COMPONENTS/PINS). Non-default modes require an existing design (chip + libs).
void read_def(const OdbDb& h, rust::Str def_path, rust::Str mode) {
  odb::dbDatabase* db = h.db;
  std::string ms = s(mode);
  odb::defin::MODE m = odb::defin::DEFAULT;
  if (ms == "floorplan") {
    m = odb::defin::FLOORPLAN;
  } else if (ms == "incremental") {
    m = odb::defin::INCREMENTAL;
  }
  odb::dbChip* chip = db->getChip();
  if (!chip) {
    if (m != odb::defin::DEFAULT) {
      throw std::runtime_error("vyges-opendb: no existing design for a floorplan/incremental DEF update");
    }
    chip = odb::dbChip::create(db, db->getTech());  // fresh chip (DEFAULT import into a tech-only db)
  }
  std::vector<odb::dbLib*> libs;
  for (odb::dbLib* lib : db->getLibs()) {
    libs.push_back(lib);
  }
  odb::defin reader(db, const_cast<utl::Logger*>(&h.logger), m);
  reader.readChip(libs, s(def_path).c_str(), chip);
}
void place_bterm(const OdbDb& h, rust::Str bterm, rust::Str layer, int32_t x1, int32_t y1,
                 int32_t x2, int32_t y2) {
  dbBlock* b = require_block(h);
  dbBTerm* bt = b->findBTerm(s(bterm).c_str());
  if (!bt) throw std::runtime_error("vyges-opendb: bterm not found: " + s(bterm));
  dbTech* tech = b->getTech();
  dbTechLayer* l = tech ? tech->findLayer(s(layer).c_str()) : nullptr;
  if (!l) throw std::runtime_error("vyges-opendb: tech layer not found: " + s(layer));
  dbBPin* bpin = dbBPin::create(bt);
  dbBox::create(bpin, l, x1, y1, x2, y2);
  bpin->setPlacementStatus(odb::dbPlacementStatus::PLACED);
}
void connect(const OdbDb& h, rust::Str inst, rust::Str pin, rust::Str net) {
  dbNet* n = require_block(h)->findNet(s(net).c_str());
  if (!n) throw std::runtime_error("vyges-opendb: net not found: " + s(net));
  require_iterm(h, inst, pin)->connect(n);
}
void disconnect(const OdbDb& h, rust::Str inst, rust::Str pin) {
  require_iterm(h, inst, pin)->disconnect();
}
std::size_t check_3dblox(const OdbDb& h) {
  if (!h.db->getChip()) throw std::runtime_error("vyges-opendb: no top chip to check");
  // Delegates to vyges::check_3dblox, which is compiled into libodb — see src/lint3d.h for why
  // odb::Checker is not constructed here.
  return vyges::check_3dblox(h.db, const_cast<utl::Logger*>(&h.logger));
}
// ---- 3D / chiplet construction ---------------------------------------------------------------
// odb's dbChip* creation statics. Unlike the read surface these cannot be generated: the
// signatures are heterogeneous (dbChipConn takes two std::vector<dbChipInst*> paths), so they
// follow the hand-written pattern of create_inst/create_net above. Objects are addressed by
// name in and out, so no odb pointer crosses the boundary.

static odb::dbChip::ChipType chip_type_of(rust::Str t) {
  const std::string v = s(t);
  if (v == "DIE") return odb::dbChip::ChipType::DIE;
  if (v == "RDL") return odb::dbChip::ChipType::RDL;
  if (v == "IP") return odb::dbChip::ChipType::IP;
  if (v == "SUBSTRATE") return odb::dbChip::ChipType::SUBSTRATE;
  if (v == "HIER") return odb::dbChip::ChipType::HIER;
  throw std::runtime_error("vyges-opendb: unknown chip type: " + v
                           + " (DIE|RDL|IP|SUBSTRATE|HIER)");
}

static odb::dbChipRegion::Side region_side_of(rust::Str t) {
  const std::string v = s(t);
  if (v == "FRONT") return odb::dbChipRegion::Side::FRONT;
  if (v == "BACK") return odb::dbChipRegion::Side::BACK;
  if (v == "INTERNAL") return odb::dbChipRegion::Side::INTERNAL;
  if (v == "INTERNAL_EXT") return odb::dbChipRegion::Side::INTERNAL_EXT;
  throw std::runtime_error("vyges-opendb: unknown region side: " + v
                           + " (FRONT|BACK|INTERNAL|INTERNAL_EXT)");
}

// Local lookups rather than the generated resolvers: this file is hand-written and should not
// depend on generator output, and each is one call on the public API.
static odb::dbChipRegion* find_region(const OdbDb& h, rust::Str chip, rust::Str region);
static odb::dbChipInst* find_chip_inst(const OdbDb& h, rust::Str chip, rust::Str inst);

static odb::dbChip* require_chip(const OdbDb& h, rust::Str name) {
  odb::dbChip* c = h.db->findChip(s(name).c_str());
  if (!c) throw std::runtime_error("vyges-opendb: chip not found: " + s(name));
  return c;
}

static odb::dbChipRegion* find_region(const OdbDb& h, rust::Str chip, rust::Str region) {
  return require_chip(h, chip)->findChipRegion(s(region));
}
static odb::dbChipInst* find_chip_inst(const OdbDb& h, rust::Str chip, rust::Str inst) {
  return require_chip(h, chip)->findChipInst(s(inst));
}

void chip_create(const OdbDb& h, rust::Str name, rust::Str tech, rust::Str chip_type) {
  // Per-chip dbTech is the load-bearing fact of the 3D model — it is what lets dies from
  // different processes coexist — so the tech is selectable by name. Empty means the database's
  // default, which is the single-process case.
  odb::dbTech* t = nullptr;
  if (tech.empty()) {
    // A chip may legitimately have no tech — upstream creates the design top that way
    // (3dblox.cpp createDesignTopChiplet), and a geometry-only read has no LEF to give it one.
    t = h.db->getTech();
  } else {
    t = h.db->findTech(s(tech).c_str());
    if (!t) throw std::runtime_error("vyges-opendb: tech not found: " + s(tech));
  }
  if (!odb::dbChip::create(h.db, t, s(name), chip_type_of(chip_type)))
    throw std::runtime_error("vyges-opendb: chip_create failed (duplicate name?): " + s(name));
}

void chip_block_create(const OdbDb& h, rust::Str chip, rust::Str name) {
  // A chip carries its own dbBlock — the die's design. The TOP chip needs one for the
  // block-level accessors to resolve through it, and a die needs one to hold the instances its
  // bumps wrap. Separate from chip_create because not every chip needs one.
  if (!odb::dbBlock::create(require_chip(h, chip), s(name).c_str()))
    throw std::runtime_error("vyges-opendb: chip_block_create failed: " + s(name));
}

void chip_inst_create(const OdbDb& h, rust::Str parent_chip, rust::Str master_chip,
                      rust::Str name) {
  // ORDER MATTERS: dbChipInst::create derives the region and bump instances from the MASTER
  // chip's regions and bumps as they exist right now. Regions added to the master afterwards
  // are silently not instantiated for this inst.
  odb::dbChip* parent = require_chip(h, parent_chip);
  odb::dbChip* master = require_chip(h, master_chip);
  if (!odb::dbChipInst::create(parent, master, s(name)))
    throw std::runtime_error("vyges-opendb: chip_inst_create failed: " + s(name));
}

void chip_region_create(const OdbDb& h, rust::Str chip, rust::Str name, rust::Str side,
                        rust::Str layer) {
  odb::dbChip* c = require_chip(h, chip);
  odb::dbTech* t = c->getTech();
  odb::dbTechLayer* l = nullptr;
  if (!layer.empty()) {
    if (!t) throw std::runtime_error("vyges-opendb: chip has no tech to resolve a layer against");
    l = t->findLayer(s(layer).c_str());
    if (!l) throw std::runtime_error("vyges-opendb: tech layer not found: " + s(layer));
  }
  if (!odb::dbChipRegion::create(c, s(name), region_side_of(side), l))
    throw std::runtime_error("vyges-opendb: chip_region_create failed: " + s(name));
}

void chip_region_set_box(const OdbDb& h, rust::Str chip, rust::Str region, int32_t x1, int32_t y1,
                         int32_t x2, int32_t y2) {
  // setBox takes a Rect, which the generated setter surface cannot marshal; without it a region
  // has no footprint and the bump-alignment and connection-region checks have nothing to test.
  odb::dbChipRegion* r = find_region(h, chip, region);
  if (!r) throw std::runtime_error("vyges-opendb: chip region not found: " + s(region));
  r->setBox(odb::Rect(x1, y1, x2, y2));
}

void chip_bump_create(const OdbDb& h, rust::Str chip, rust::Str region, rust::Str inst) {
  // A bump wraps a placed dbInst living in the OWNING CHIP's block, not the top block.
  odb::dbChipRegion* r = find_region(h, chip, region);
  if (!r) throw std::runtime_error("vyges-opendb: chip region not found: " + s(region));
  odb::dbBlock* b = require_chip(h, chip)->getBlock();
  if (!b) throw std::runtime_error("vyges-opendb: chip has no block to hold bump instances: "
                                   + s(chip));
  odb::dbInst* i = b->findInst(s(inst).c_str());
  if (!i) throw std::runtime_error("vyges-opendb: instance not found in chip block: " + s(inst));
  if (!odb::dbChipBump::create(r, i))
    throw std::runtime_error("vyges-opendb: chip_bump_create failed: " + s(inst));
}

void chip_conn_create(const OdbDb& h, rust::Str name, rust::Str parent_chip, rust::Str top_inst,
                      rust::Str top_region, rust::Str bottom_inst, rust::Str bottom_region,
                      int32_t thickness) {
  // odb takes a PATH of chip insts on each side, to name a region inside a nested assembly.
  // This binds the direct case — one hop per side — which is what a bond between two chips in
  // the same parent needs. Deeper paths are expressible upstream and not yet here; that wants a
  // list-valued parameter, and no caller has needed one.
  odb::dbChip* parent = require_chip(h, parent_chip);
  odb::dbChipInst* ti = find_chip_inst(h, parent_chip, top_inst);
  odb::dbChipInst* bi = find_chip_inst(h, parent_chip, bottom_inst);
  if (!ti) throw std::runtime_error("vyges-opendb: chip inst not found: " + s(top_inst));
  if (!bi) throw std::runtime_error("vyges-opendb: chip inst not found: " + s(bottom_inst));
  odb::dbChipRegionInst* tr = ti->findChipRegionInst(s(top_region));
  odb::dbChipRegionInst* br = bi->findChipRegionInst(s(bottom_region));
  if (!tr) throw std::runtime_error("vyges-opendb: region inst not found on " + s(top_inst)
                                    + ": " + s(top_region));
  if (!br) throw std::runtime_error("vyges-opendb: region inst not found on " + s(bottom_inst)
                                    + ": " + s(bottom_region));
  odb::dbChipConn* c = odb::dbChipConn::create(s(name), parent, {ti}, tr, {bi}, br);
  if (!c) throw std::runtime_error("vyges-opendb: chip_conn_create failed: " + s(name));
  c->setThickness(thickness);
}

void chip_net_create(const OdbDb& h, rust::Str chip, rust::Str name) {
  if (!odb::dbChipNet::create(require_chip(h, chip), s(name)))
    throw std::runtime_error("vyges-opendb: chip_net_create failed: " + s(name));
}

void chip_path_create(const OdbDb& h, rust::Str chip, rust::Str name) {
  if (!odb::dbChipPath::create(require_chip(h, chip), s(name).c_str()))
    throw std::runtime_error("vyges-opendb: chip_path_create failed: " + s(name));
}

std::unique_ptr<OdbDb> new_db() { return std::make_unique<OdbDb>(); }

void tech_create(const OdbDb& h, rust::Str name) {
  // odb refuses to create a DIE chip without a technology, which is why a .3dbv points each
  // chiplet at its own APR_tech_file. A geometry-only read has no LEF to give it one, so it
  // needs a placeholder: no layers, no rules, only the precision the coordinates are in.
  odb::dbTech* t = odb::dbTech::create(h.db, s(name).c_str());
  if (!t) throw std::runtime_error("vyges-opendb: tech_create failed (one already exists?): "
                                   + s(name));
  // Precision is a database-level property (dbTech only exposes a getter), so the caller sets
  // it via set_dbu_per_micron BEFORE creating the tech.
}

int32_t dbu_per_micron(const OdbDb& h) { return h.db->getDbuPerMicron(); }

void set_dbu_per_micron(const OdbDb& h, int32_t dbu) {
  // A 3Dblox header declares the precision its coordinates are written at, and odb validates
  // the database's dbu against it. Coordinates in those files are microns, so this is what
  // makes the conversion to DBU well defined rather than assumed.
  h.db->setDbuPerMicron(dbu);
}

void chip_net_add_bump(const OdbDb& h, rust::Str chip, rust::Str net, rust::Str chip_inst,
                       rust::Str region, std::size_t bump_index) {
  // Associates a bump INSTANCE with a logical 3D net. The logical-connectivity check compares
  // the nets of physically aligned bump pairs, so without this the check has nothing to
  // disagree about and silently passes on any design.
  odb::dbChip* c = require_chip(h, chip);
  odb::dbChipNet* n = nullptr;
  for (odb::dbChipNet* cand : c->getChipNets()) {
    if (cand->getName() == s(net)) { n = cand; break; }
  }
  if (!n) throw std::runtime_error("vyges-opendb: chip net not found: " + s(net));
  odb::dbChipInst* ci = find_chip_inst(h, chip, chip_inst);
  if (!ci) throw std::runtime_error("vyges-opendb: chip inst not found: " + s(chip_inst));
  odb::dbChipRegionInst* ri = ci->findChipRegionInst(s(region));
  if (!ri) throw std::runtime_error("vyges-opendb: region inst not found: " + s(region));
  std::size_t i = 0;
  for (odb::dbChipBumpInst* b : ri->getChipBumpInsts()) {
    if (i++ == bump_index) { n->addBumpInst(b, {ci}); return; }
  }
  throw std::runtime_error("vyges-opendb: bump index out of range on " + s(chip_inst) + "/"
                           + s(region));
}

void alignment_marker_rule_create(const OdbDb& h, rust::Str master_a, rust::Str master_b,
                                  int32_t tolerance) {
  // The alignment-marker check returns immediately when no rules exist, so a design without
  // one is not "clean" so much as unexamined.
  odb::dbMaster* a = h.db->findMaster(s(master_a).c_str());
  odb::dbMaster* b = h.db->findMaster(s(master_b).c_str());
  if (!a) throw std::runtime_error("vyges-opendb: master not found: " + s(master_a));
  if (!b) throw std::runtime_error("vyges-opendb: master not found: " + s(master_b));
  odb::dbAlignmentMarkerRule* r = odb::dbAlignmentMarkerRule::create(a, b);
  if (!r) throw std::runtime_error("vyges-opendb: alignment_marker_rule_create failed");
  r->setTolerance(tolerance);
}

void set_top_chip(const OdbDb& h, rust::Str chip) {
  // Roots the assembly. dbUnfoldedBuilder::build() starts from dbDatabase::getChip() and walks
  // its chip insts, so with the top chip left pointing elsewhere every unfolded table reads
  // empty and nothing says why.
  h.db->setTopChip(require_chip(h, chip));
}

void construct_unfolded_model(const OdbDb& h) {
  if (!h.db->getChip()) throw std::runtime_error("vyges-opendb: no top chip to unfold");
  // The unfolded tables are derived, not stored: _dbDatabase::operator>> calls this on read.
  // A caller that moves a dbChipInst must call it again, or every unfolded query -- and the
  // 3D linter, which reads through them -- answers from the placement before the move.
  h.db->constructUnfoldedModel();
}
void log_capture_begin(const OdbDb& h) {
  // libodb's default sink writes to STDOUT, which corrupts any caller whose stdout is
  // machine-readable (our JSON subcommands). Capture instead, so the caller decides where the
  // human-readable text goes. utl::Logger's redirect detaches every sink for the duration --
  // including our events forwarder -- and restores them on end, so diagnostics emitted while
  // captured reach the events trail only via whatever the caller does with the returned text.
  const_cast<utl::Logger&>(h.logger).redirectStringBegin();
}
rust::String log_capture_end(const OdbDb& h) {
  return rust::String(const_cast<utl::Logger&>(h.logger).redirectStringEnd());
}
void eco_begin(const OdbDb& h) { odb::dbDatabase::beginEco(require_block(h)); }
void eco_end(const OdbDb& h) { odb::dbDatabase::endEco(require_block(h)); }
void eco_commit(const OdbDb& h) { odb::dbDatabase::commitEco(require_block(h)); }
void eco_undo(const OdbDb& h) { odb::dbDatabase::undoEco(require_block(h)); }
bool eco_empty(const OdbDb& h) { return odb::dbDatabase::ecoEmpty(require_block(h)); }
bool swap_master(const OdbDb& h, rust::Str inst, rust::Str master) {
  dbInst* i = require_inst(h, inst);
  std::string want = s(master);
  dbMaster* m = nullptr;
  for (odb::dbLib* lib : h.db->getLibs()) {
    if ((m = lib->findMaster(want.c_str()))) break;
  }
  if (!m) throw std::runtime_error("vyges-opendb: master not found: " + want);
  // odb throws (utl::Logger::error) on a dont_touch instance; let that propagate as a Result.
  return i->swapMaster(m);
}
