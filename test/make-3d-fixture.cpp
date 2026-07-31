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
//
//   top_die  : width 50000, height 40000, thickness 700,  tsv true
//   base_die : width 60000, height 50000, thickness 1500, tsv false
//
// The two die chips deliberately carry DIFFERENT ChipTypes: odb ships no getString() for
// dbChip::ChipType, so the generator emits the mapping, and a fixture where every chip was a
// DIE could not tell a working mapping from one that returns a constant.
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

  dbChip* stack = dbChip::create(db, nullptr, "stack", dbChip::ChipType::HIER);

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

  // ORDER MATTERS: setOrient BEFORE setLoc. dbChipInst::setLoc does not store the point it is
  // given — it orients the master chip's cuboid, then stores the delta that puts that cuboid's
  // lower-left-lower corner at the requested point. getLoc() is getCuboid().lll(), which
  // re-applies the *current* orientation. So setting the location first and rotating afterwards
  // silently moves the chip: the stored delta was computed against the old orientation.
  dbChipInst* u_top = dbChipInst::create(stack, top_die, "u_top");
  // MZ_R90: mirrored in Z and rotated 90. Exercises both halves of dbOrientType3D, which a
  // plain R0 would not — R0 with no mirror is also the default-constructed value.
  u_top->setOrient(dbOrientType3D(dbOrientType::R90, true));
  u_top->setLoc(Point3D(1000, 2000, 3000));

  dbChipInst* u_base = dbChipInst::create(stack, base_die, "u_base");
  u_base->setOrient(dbOrientType3D(dbOrientType::R0, false));
  u_base->setLoc(Point3D(0, 0, 0));

  // NOTE: deliberately NOT calling db->setTopChip(stack). gen_block() resolves the top block
  // via db->getChip()->getBlock(), so repointing the top chip at a block-less HIER chip would
  // strand every existing 2D accessor on this fixture.

  std::ofstream out(argv[2], std::ios::binary);
  if (!out) {
    std::cerr << "cannot write " << argv[2] << "\n";
    return 1;
  }
  db->write(out);
  std::cout << "wrote " << argv[2] << ": 3 chips (stack/top_die/base_die), 2 chip insts\n";
  return 0;
}
