// SPDX-License-Identifier: Apache-2.0
// Fixture generator for the timing-driven ECO loop: emits ONE design in the TWO forms the loop
// needs, from a single definition.
//
//   <out>.odb  — the design database the applier edits (vyges-opendb)
//   <out>.v    — the gate-level netlist the planner times (vyges-sta-si, via loom)
//
// Why both come out of one program: the planner and the applier are deliberately joined by a
// file rather than a library call, which means they must agree on instance names, pin names and
// cell names. Hand-maintaining a .v beside a .odb is a drift waiting to happen — the plan would
// name instances the database does not have, and the failure would look like a bug in the
// applier. Generating both from the same statements makes that impossible by construction.
//
// The matching Liberty (.lib) is hand-written and lives with the timing tests: it carries delay
// tables, not structure, so it cannot drift from the netlist in the way names can.
//
// Build with -DVYGES_ODB_MKECOFIXTURE=ON, then:
//     odb_mkecofixture <out-prefix>
//
// The design is a two-flop ring — the smallest thing with a real register-to-register hold path:
//
//     r1.Q -> g1(INV) -> r2.D          r2.Q -> r1.D  (and out to port y)
//
// Both flop D pins are launched by a flop Q, so hold is checkable and repairable at both. A BUF
// master exists but is unused: the repair inserts it, and a master the design never instantiates
// still has to be in the library for the applier to find.
#include <fstream>
#include <iostream>
#include <string>
#include <vector>

#include "odb/db.h"
#include "utl/Logger.h"

using namespace odb;

namespace {

// One instance of the shared design. The generator walks this list twice — once to build the
// database, once to write the Verilog — so the two can never disagree.
struct InstDef
{
  std::string name;
  std::string master;
  // (pin, net) in the order the master declares them
  std::vector<std::pair<std::string, std::string>> conns;
};

dbMaster* make_master(dbLib* lib,
                      const char* name,
                      const std::vector<std::pair<const char*, dbIoType>>& pins,
                      bool sequential)
{
  dbMaster* m = dbMaster::create(lib, name);
  m->setType(dbMasterType::CORE);
  // A nominal footprint. Real geometry comes from a LEF; the loop never reads it, but a master
  // with zero size confuses anything that later tries to place the cell.
  m->setWidth(1000);
  m->setHeight(2720);
  for (const auto& [pin, io] : pins) {
    dbMTerm::create(m, pin, io, dbSigType::SIGNAL);
  }
  if (sequential) {
    m->setSequential(true);
  }
  m->setFrozen();
  return m;
}

}  // namespace

int main(int argc, char** argv)
{
  if (argc < 2) {
    std::cerr << "usage: odb_mkecofixture <out-prefix>\n";
    return 2;
  }
  const std::string prefix = argv[1];

  utl::Logger logger;
  dbDatabase* db = dbDatabase::create();
  db->setLogger(&logger);

  dbTech* tech = dbTech::create(db, "eco_demo_tech");
  tech->setLefUnits(1000);  // dbTech::create takes no DBU; set it explicitly
  dbLib* lib = dbLib::create(db, "eco_demo_lib", tech);

  make_master(lib, "INV", {{"A", dbIoType::INPUT}, {"Y", dbIoType::OUTPUT}}, false);
  // Unused by the design on purpose — this is what a hold repair inserts, and the applier can
  // only insert a master the library actually holds.
  make_master(lib, "BUF", {{"A", dbIoType::INPUT}, {"Y", dbIoType::OUTPUT}}, false);
  make_master(lib,
              "DFF",
              {{"CK", dbIoType::INPUT}, {"D", dbIoType::INPUT}, {"Q", dbIoType::OUTPUT}},
              true);

  dbChip* chip = dbChip::create(db, tech, "eco_demo");
  dbBlock* block = dbBlock::create(chip, "eco_demo");
  block->setDefUnits(1000);

  // ---- the shared design, declared once ----
  const std::vector<std::string> inputs = {"clk"};
  const std::vector<std::string> outputs = {"y"};
  const std::vector<InstDef> insts = {
      {"r1", "DFF", {{"CK", "clk"}, {"D", "y"}, {"Q", "q1"}}},
      {"g1", "INV", {{"A", "q1"}, {"Y", "n1"}}},
      {"r2", "DFF", {{"CK", "clk"}, {"D", "n1"}, {"Q", "y"}}},
  };

  // ---- form 1: the database ----
  for (const std::string& p : inputs) {
    dbNet* net = dbNet::create(block, p.c_str());
    dbBTerm* bt = dbBTerm::create(net, p.c_str());
    bt->setIoType(dbIoType::INPUT);
    bt->setSigType(p == "clk" ? dbSigType::CLOCK : dbSigType::SIGNAL);
  }
  for (const std::string& p : outputs) {
    dbNet* net = dbNet::create(block, p.c_str());
    dbBTerm* bt = dbBTerm::create(net, p.c_str());
    bt->setIoType(dbIoType::OUTPUT);
  }

  int x = 0;
  for (const InstDef& def : insts) {
    dbMaster* master = lib->findMaster(def.master.c_str());
    if (master == nullptr) {
      std::cerr << "no master " << def.master << "\n";
      return 1;
    }
    dbInst* inst = dbInst::create(block, master, def.name.c_str());
    // Spread them out so locations are distinct — an inserted cell inherits its target's
    // position, and coincident instances make that impossible to eyeball.
    inst->setLocation(x, 0);
    inst->setPlacementStatus(dbPlacementStatus::PLACED);
    x += 5000;

    for (const auto& [pin, net_name] : def.conns) {
      dbNet* net = block->findNet(net_name.c_str());
      if (net == nullptr) {
        net = dbNet::create(block, net_name.c_str());
      }
      dbITerm* it = inst->findITerm(pin.c_str());
      if (it == nullptr) {
        std::cerr << "no pin " << pin << " on " << def.name << "\n";
        return 1;
      }
      it->connect(net);
    }
  }

  const std::string odb = prefix + ".odb";
  {
    std::ofstream out(odb, std::ios::binary);
    if (!out) {
      std::cerr << "cannot write " << odb << "\n";
      return 1;
    }
    db->write(out);
  }

  // ---- form 2: the Verilog netlist, from the SAME statements ----
  const std::string v = prefix + ".v";
  {
    std::ofstream out(v);
    if (!out) {
      std::cerr << "cannot write " << v << "\n";
      return 1;
    }
    out << "// GENERATED by odb_mkecofixture — do not edit.\n"
        << "// The matching " << odb.substr(odb.find_last_of('/') + 1)
        << " is emitted from the same statements, so instance,\n"
        << "// pin and cell names are identical on both sides of the ECO loop by construction.\n"
        << "//\n"
        << "// Two-flop ring: r1.Q -> g1(INV) -> r2.D, and r2.Q (= port y) -> r1.D. Both flop D\n"
        << "// pins are launched by a flop Q, so hold is register-to-register and repairable.\n"
        << "module eco_demo ( clk, y );\n"
        << "  input  clk;\n"
        << "  output y;\n"
        << "  wire q1, n1;\n";
    for (const InstDef& def : insts) {
      out << "  " << def.master << " " << def.name << " (";
      for (std::size_t i = 0; i < def.conns.size(); ++i) {
        out << (i ? ", " : " ") << "." << def.conns[i].first << "(" << def.conns[i].second << ")";
      }
      out << " );\n";
    }
    out << "endmodule\n";
  }

  std::cout << "wrote " << odb << " and " << v << ": " << insts.size() << " instances, masters "
            << "INV/BUF/DFF\n";
  return 0;
}
