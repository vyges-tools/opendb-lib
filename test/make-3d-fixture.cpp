// SPDX-License-Identifier: Apache-2.0
// Fixture generator: read a flat .odb and bolt on a small, well-known 3D / CHIPLET hierarchy,
// then write it out. Produces the 3D fixture the Rust tests read (chiplet3d.odb) so the
// dbChip / dbChipInst accessors can be validated against *populated* data with exact values,
// not just discovery / graceful-empty.
//
// Same rationale as make-hier-fixture.cpp: our safe Rust API deliberately does not expose
// structural *creation*, and we cannot read a .3dbv/.3dbx yet (the 3Dblox parsers need
// yaml-cpp, and 3dblox.cpp's netlist path needs OpenSTA — neither is in this build). This tiny
// libodb program is the reproducible way to synthesize the fixture.
//
// Build with -DVYGES_ODB_MK3DFIXTURE=ON, then:
//     odb_mk3dfixture <in-flat.odb> <out-chiplet3d.odb>
//
// Writing and re-reading this also independently checks the persistency claim for the 3D
// schema: if write_db/read_db did not round-trip dbChip*, the Rust tests would read nothing.
//
// What it creates (names and values the tests assert on):
//
//   stack : dbChip HIER, no tech
//     |- u_top  : dbChipInst -> top_die  (DIE)       loc (1000, 2000, 3000)  orient MZ_R90
//     |- u_base : dbChipInst -> base_die (SUBSTRATE) loc (0, 0, 0)           orient R0
//     |- bond0  : dbChipConn  u_top/front <-> u_base/back, thickness 25
//     |- vdd_3d : dbChipNet
//     |- path0  : dbChipPath
//
//   top_die  : width 50000, height 40000, thickness 700,  tsv true
//              block "top_die_blk" with inst "bump_pad0"
//              region "front" (FRONT), box (0,0)-(50000,40000), one dbChipBump on bump_pad0
//   base_die : width 60000, height 50000, thickness 1500, tsv false
//              region "back" (BACK), box (0,0)-(60000,50000)
//
// The two die chips deliberately carry DIFFERENT ChipTypes, and the two regions different
// Sides: odb ships no getString() for dbChip::ChipType or dbChipRegion::Side, so the generator
// emits those mappings, and a fixture where every value was the same could not tell a working
// mapping from one that returns a constant.
//
// dbChipRegionInst / dbChipBumpInst are NOT created here — dbChipInst::create derives them from
// the master chip's regions and bumps. The UNFOLDED classes are not written either: they are
// rebuilt by constructUnfoldedModel(), which _dbDatabase::operator>> runs on read whenever the
// database holds more than one chip.
//
// THE STACK IS DELIBERATELY DEFECTIVE, and check_3dblox is expected to report exactly two
// violations on it. A linter fixture that lints clean proves almost nothing — "found 0" is also
// what a broken checker returns — so this one carries a realistic modelling error:
//
//   u_top is mirrored in Z (MZ_R90), so top_die's FRONT region ends up facing BOTTOM. It is
//   bonded to base_die's BACK region, which also faces BOTTOM. Two surfaces pointing the same
//   way cannot mate, so:
//     1. "Connection regions"  -> bond0 is invalid (both faces BOTTOM)
//     2. "Floating chips"      -> u_base is an isolated set, BECAUSE its only bond is invalid
//
//   The second follows from the first, which is what makes it a good test: the checker is
//   reasoning over the assembled geometry, not counting objects.
//
// Do not "fix" the orientation to make it lint clean — the read tests assert MZ_R90 and the
// lint tests assert these two findings. A clean-path check is covered separately by running
// the linter over the flat 2D fixtures, which correctly report 0.
#include <fstream>
#include <iostream>

#include "odb/db.h"
#include "odb/dbTypes.h"
#include "odb/geom.h"
#include "utl/Logger.h"

using namespace odb;

int main(int argc, char** argv)
{
  if (argc < 3) {
    std::cerr << "usage: odb_mk3dfixture <in-flat.odb> <out-chiplet3d.odb>\n";
    return 2;
  }
  utl::Logger logger;
  dbDatabase* db = dbDatabase::create();
  db->setLogger(&logger);

  std::ifstream in(argv[1], std::ios::binary);
  if (!in) {
    std::cerr << "cannot open " << argv[1] << "\n";
    return 1;
  }
  db->read(in);

  // Reuse the flat design's tech for the die chips — per-chip dbTech is the load-bearing fact
  // of the 3D model, and a DIE with no tech is not representative. The HIER chip gets none,
  // matching what the 3Dblox reader does for a design top (3dblox.cpp createDesignTopChiplet).
  dbTech* tech = db->getTech();
  if (!tech) {
    std::cerr << "no tech in " << argv[1] << "\n";
    return 1;
  }

  // The design top. 3Dblox builds this with no tech, but we give it one so it can carry a block:
  // db->setTopChip(stack) below is what roots the unfolded model, and dbDatabase::getChip() is
  // also what our block-level accessors resolve through, so the top chip must have a block.
  dbChip* stack = dbChip::create(db, tech, "stack", dbChip::ChipType::HIER);
  dbBlock::create(stack, "stack_blk");

  dbChip* top_die = dbChip::create(db, tech, "top_die", dbChip::ChipType::DIE);
  top_die->setWidth(50000);
  top_die->setHeight(40000);
  top_die->setThickness(700);
  top_die->setTsv(true);

  dbChip* base_die
      = dbChip::create(db, tech, "base_die", dbChip::ChipType::SUBSTRATE);
  base_die->setWidth(60000);
  base_die->setHeight(50000);
  base_die->setThickness(1500);
  base_die->setTsv(false);

  // --- bonding surfaces, and something real for a bump to sit on ------------------------------
  // A dbChipBump wraps a placed dbInst, so top_die needs its own block with an instance in it.
  // Per-chip blocks are exactly the point of the 3D model: each die carries its own design.
  dbTechLayer* layer = nullptr;
  for (dbTechLayer* l : tech->getLayers()) {
    layer = l;
    break;
  }
  dbMaster* master = nullptr;
  for (dbLib* lib : db->getLibs()) {
    for (dbMaster* m : lib->getMasters()) {
      master = m;
      break;
    }
    if (master) {
      break;
    }
  }
  if (!layer || !master) {
    std::cerr << "need at least one tech layer and one master in " << argv[1] << "\n";
    return 1;
  }

  dbBlock* top_blk = dbBlock::create(top_die, "top_die_blk");
  dbInst* pad = dbInst::create(top_blk, master, "bump_pad0");

  // ORDER MATTERS (2): regions and their bumps must exist on the MASTER chip before any
  // dbChipInst of it is created. dbChipInst::create walks the master's regions and bumps to
  // build the matching dbChipRegionInst / dbChipBumpInst; regions added afterwards are simply
  // not instantiated, silently, for that inst.
  dbChipRegion* top_front = dbChipRegion::create(
      top_die, "front", dbChipRegion::Side::FRONT, layer);
  top_front->setBox(Rect(0, 0, 50000, 40000));
  dbChipBump::create(top_front, pad);

  dbChipRegion* base_back = dbChipRegion::create(
      base_die, "back", dbChipRegion::Side::BACK, layer);
  base_back->setBox(Rect(0, 0, 60000, 50000));

  // ORDER MATTERS (1): setOrient BEFORE setLoc. dbChipInst::setLoc does not store the point it
  // is given — it orients the master chip's cuboid, then stores the delta that puts that
  // cuboid's lower-left-lower corner at the requested point. getLoc() is getCuboid().lll(),
  // which re-applies the *current* orientation. So setting the location first and rotating
  // afterwards silently moves the chip: the stored delta was computed against the old
  // orientation.
  dbChipInst* u_top = dbChipInst::create(stack, top_die, "u_top");
  // MZ_R90: mirrored in Z and rotated 90. Exercises both halves of dbOrientType3D, which a
  // plain R0 would not — R0 with no mirror is also the default-constructed value.
  u_top->setOrient(dbOrientType3D(dbOrientType::R90, true));
  u_top->setLoc(Point3D(1000, 2000, 3000));

  dbChipInst* u_base = dbChipInst::create(stack, base_die, "u_base");
  u_base->setOrient(dbOrientType3D(dbOrientType::R0, false));
  u_base->setLoc(Point3D(0, 0, 0));

  // --- bonding connection, 3D net and path ----------------------------------------------------
  // The region insts were created for us by dbChipInst::create above, so look them up by the
  // master region's name rather than trying to construct them.
  dbChipRegionInst* top_ri = u_top->findChipRegionInst("front");
  dbChipRegionInst* base_ri = u_base->findChipRegionInst("back");
  if (!top_ri || !base_ri) {
    std::cerr << "region insts were not created — regions must precede dbChipInst::create\n";
    return 1;
  }

  dbChipConn* bond = dbChipConn::create(
      "bond0", stack, {u_top}, top_ri, {u_base}, base_ri);
  bond->setThickness(25);

  dbChipNet::create(stack, "vdd_3d");
  dbChipPath::create(stack, "path0");

  // Root the stack. This is REQUIRED for the unfolded model: dbUnfoldedBuilder::build() starts
  // from dbDatabase::getChip() and walks its chip insts, so with the top chip left pointing at
  // the flat design we read in, the unfolded tables come back empty.
  //
  // The UNFOLDED model itself is not written and does not need to be — _dbDatabase's operator>>
  // calls constructUnfoldedModel() on read whenever the database holds more than one chip, so
  // it is rebuilt on load. That is why the unfolded accessors have data to answer from despite
  // being derived rather than serialised.
  //
  // Consequence: on THIS fixture the block-level accessors resolve through stack's own
  // (near-empty) block, not the counter design's. That is correct for a 3D database and is why
  // the flat 2D tests keep using counter.odb / hier.odb.
  db->setTopChip(stack);

  std::ofstream out(argv[2], std::ios::binary);
  if (!out) {
    std::cerr << "cannot write " << argv[2] << "\n";
    return 1;
  }
  db->write(out);
  std::cout << "wrote " << argv[2] << ": 3 chips (stack/top_die/base_die), 2 chip insts\n";
  return 0;
}
