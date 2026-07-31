// SPDX-License-Identifier: Apache-2.0
// 3Dblox I/O round-trip: read a chiplet .odb, write it out as .3dbv + .3dbx, read those back
// into a FRESH database, and check the assembly survived.
//
// Built only with -DVYGES_ODB_3DBLOX_IO=ON, which pulls in yaml-cpp (the parsers/writers) and
// OpenSTA (3dblox.cpp's Verilog/Liberty front end). Everything else 3D — the dbChip* schema,
// the unfolded model, check_3dblox — needs neither, so it is NOT part of the default build.
//
//     odb_blox_roundtrip <in-chiplet.odb> <out-prefix>
//
// Writing before reading is deliberate: it is what gives us a real .3dbv/.3dbx to read at all.
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <string>

#include "3dblox.h"
#include "odb/db.h"
#include "utl/Logger.h"

using namespace odb;

namespace {

int fail(const std::string& msg)
{
  std::cerr << "FAIL: " << msg << "\n";
  return 1;
}

// Report what a database holds, so the two sides can be compared field by field rather than
// on a single count that could match by accident.
struct Summary
{
  std::size_t chips = 0, insts = 0, conns = 0, regions = 0;
  std::string top;
};

Summary summarize(dbDatabase* db)
{
  Summary s;
  s.chips = db->getChips().size();
  s.insts = db->getChipInsts().size();
  s.conns = db->getChipConns().size();
  for (dbChip* c : db->getChips()) {
    s.regions += c->getChipRegions().size();
  }
  dbChip* top = db->getChip();
  s.top = top != nullptr ? top->getName() : "";
  return s;
}

void print(const char* label, const Summary& s)
{
  std::cout << label << ": top=" << s.top << " chips=" << s.chips
            << " chip_insts=" << s.insts << " conns=" << s.conns
            << " regions=" << s.regions << "\n";
}

}  // namespace

int main(int argc, char** argv)
{
  if (argc < 3) {
    std::cerr << "usage: odb_blox_roundtrip <in-chiplet.odb> <out-prefix>\n";
    return 2;
  }
  const std::string in_odb = argv[1];
  const std::string prefix = argv[2];
  const std::string dbv = prefix + ".3dbv";
  const std::string dbx = prefix + ".3dbx";

  utl::Logger logger;

  // ---- source database ----
  dbDatabase* src = dbDatabase::create();
  src->setLogger(&logger);
  {
    std::ifstream in(in_odb, std::ios::binary);
    if (!in) {
      return fail("cannot open " + in_odb);
    }
    src->read(in);
  }
  dbChip* top = src->getChip();
  if (top == nullptr) {
    return fail("no top chip in " + in_odb);
  }
  const Summary before = summarize(src);
  print("wrote  ", before);

  // ---- write 3Dblox ----
  // No Sta* — the writer path does not need one; only the Verilog/Liberty READ path does.
  ThreeDBlox writer(&logger, src, nullptr);
  writer.writeDbv(dbv, top);
  writer.writeDbx(dbx, top);

  for (const std::string& f : {dbv, dbx}) {
    std::ifstream check(f);
    if (!check) {
      return fail("writer produced no " + f);
    }
  }

  // ---- read it back into a FRESH database ----
  dbDatabase* dst = dbDatabase::create();
  dst->setLogger(&logger);
  ThreeDBlox reader(&logger, dst, nullptr);
  reader.readDbv(dbv);
  reader.readDbx(dbx);

  const Summary after = summarize(dst);
  print("read   ", after);

  // The .3dbv/.3dbx pair carries the ASSEMBLY (chiplet definitions, the stack, bonding), not
  // the per-die layout — so compare the assembly, not instance counts inside the blocks.
  int rc = 0;
  if (after.chips != before.chips) {
    rc = fail("chip count changed: " + std::to_string(before.chips) + " -> "
              + std::to_string(after.chips));
  }
  if (after.insts != before.insts) {
    rc = fail("chip-inst count changed: " + std::to_string(before.insts) + " -> "
              + std::to_string(after.insts));
  }
  if (after.regions != before.regions) {
    rc = fail("region count changed: " + std::to_string(before.regions) + " -> "
              + std::to_string(after.regions));
  }
  if (rc == 0) {
    std::cout << "3Dblox round-trip OK\n";
  }
  return rc;
}
