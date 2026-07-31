// SPDX-License-Identifier: Apache-2.0
// Vyges-owned entry point for odb's 3D structural linter (check_3dblox).
//
// Why this exists rather than including odb/src/3dblox/checker.h from the cxx shim: checker.h
// is an INTERNAL header, not part of odb's public include/ tree. The prebuilt-bundle path
// (VYGES_ODB_PREBUILT_DIR) ships only include/{odb,utl}, so a shim that reached into
// src/3dblox would compile from source and fail against a bundle. Constructing odb::Checker
// also puts its object layout in our stack frame, which would silently break if upstream
// added a member.
//
// So lint3d.cpp is compiled INTO libodb by our CMakeLists, where odb's internal headers are
// available, and everything outside sees only this pointer-free declaration.
#pragma once
#include <cstddef>

namespace odb {
class dbDatabase;
}
namespace utl {
class Logger;
}

namespace vyges {

// Run odb's 3D checker over the chiplet assembly and return the total number of violation
// markers it filed. 0 means clean. Returns 0 if the database has no top chip.
//
// The logger is passed in because dbDatabase exposes setLogger but no getter — the caller owns
// it (in our case OdbDb does). The checker logs each violation through it as well as filing a
// marker, so this is also what routes 3D lint output into the events trail.
//
// Violations are recorded as ordinary dbMarker objects under a "3DBlox" category on the top
// chip, one sub-category per check, so they are read back through the normal marker accessors.
// This annotates the in-memory database; it does not modify the design.
std::size_t check_3dblox(odb::dbDatabase* db, utl::Logger* logger);

}  // namespace vyges
