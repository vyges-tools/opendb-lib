// SPDX-License-Identifier: Apache-2.0
// Implementation of the 3D-lint entry point. Compiled into libodb by CMakeLists (NOT by the
// cxx build), because it is the one file of ours that needs odb's internal 3dblox headers.
#include "lint3d.h"

#include "checker.h"  // odb/src/3dblox — internal; see lint3d.h for why this lives here
#include "odb/db.h"
#include "utl/Logger.h"

namespace vyges {

std::size_t check_3dblox(odb::dbDatabase* db, utl::Logger* logger)
{
  if (db == nullptr || logger == nullptr) {
    return 0;
  }
  odb::dbChip* chip = db->getChip();
  if (chip == nullptr) {
    return 0;
  }

  // The checker reads the UNFOLDED model. dbDatabase::operator>> builds it on read for a
  // multi-chip database, but rebuild here so the result reflects any edits made since the
  // load rather than a stale flattening.
  db->constructUnfoldedModel();

  odb::Checker checker(logger, db);
  checker.check();

  // Everything is filed under a "3DBlox" category on the top chip, one sub-category per check.
  // getAllMarkers() walks that tree, so this is the total across every check.
  odb::dbMarkerCategory* cat = chip->findMarkerCategory("3DBlox");
  return cat != nullptr ? cat->getAllMarkers().size() : 0;
}

}  // namespace vyges
