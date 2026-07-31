# SPDX-License-Identifier: Apache-2.0
# vyges-opendb-lib — standalone libodb (OpenDB core + LEF/DEF I/O) built from a pinned, sparse
# OpenROAD subtree. No SWIG, no Tcl, no OpenROAD engines. C++20. BISON at build time (LEF/DEF).
#
# The source layout is pinned (see openroad-pin.yaml); scripts/fetch-odb-src.sh
# sparse-checks-out src/odb + src/utl + cmake into vendor/OpenROAD.
cmake_minimum_required(VERSION 3.20)
project(vyges_odb LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)              # odb headers use operator<=> and <numbers>
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON) # so the .a can go into a Rust cdylib later
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(OPENROAD_SRC "${CMAKE_SOURCE_DIR}/vendor/OpenROAD"
    CACHE PATH "Root of the sparse OpenROAD subtree (src/odb, src/utl, cmake)")
if(NOT EXISTS "${OPENROAD_SRC}/src/odb/include/odb/db.h")
  message(FATAL_ERROR
    "OpenROAD odb source not found at ${OPENROAD_SRC}.\n"
    "Run: scripts/fetch-odb-src.sh   (sparse-checkout of the pinned SHA)")
endif()

# ---- external deps -----------------------------------------------------------
# Boost (headers) + zlib come from the system: stable, ABI-safe, always present.
# fmt / spdlog / abseil are built FROM SOURCE at PINNED versions and static-linked, so the
# binary does not depend on the host's (often older, incompatible) system versions. This is
# what makes libodb portable — the version floor is glibc, not the distro's C++ libs.
# (Versions mirrored in openroad-pin.yaml for the provenance manifest.)
find_package(Boost REQUIRED COMPONENTS iostreams)  # iostreams (compiled) for gzipped LEF/DEF
find_package(ZLIB  REQUIRED)

include(FetchContent)
set(BUILD_SHARED_LIBS OFF CACHE BOOL "" FORCE)      # build the fetched deps static
set(ABSL_PROPAGATE_CXX_STD ON  CACHE BOOL "" FORCE)
set(ABSL_ENABLE_INSTALL    OFF CACHE BOOL "" FORCE)
set(FMT_INSTALL            OFF CACHE BOOL "" FORCE)
set(SPDLOG_INSTALL         OFF CACHE BOOL "" FORCE)
set(SPDLOG_FMT_EXTERNAL    ON  CACHE BOOL "" FORCE)  # use our pinned fmt, not spdlog's bundled copy

FetchContent_Declare(fmt    GIT_REPOSITORY https://github.com/fmtlib/fmt.git        GIT_TAG 11.0.2     GIT_SHALLOW TRUE)
FetchContent_Declare(absl   GIT_REPOSITORY https://github.com/abseil/abseil-cpp.git GIT_TAG 20250127.0 GIT_SHALLOW TRUE)
FetchContent_Declare(spdlog GIT_REPOSITORY https://github.com/gabime/spdlog.git     GIT_TAG v1.15.3    GIT_SHALLOW TRUE)
FetchContent_MakeAvailable(fmt absl)   # fmt before spdlog (SPDLOG_FMT_EXTERNAL)
FetchContent_MakeAvailable(spdlog)

# ---- sources (pinned layout) -------------------------------------------------
# v1 scope: the db core + utl (logger) + LEF/DEF I/O (lefin/lefout/defin/defout and the si2
# LEF/DEF parser libs). We do NOT use OpenROAD's per-sub-lib CMakeLists via add_subdirectory —
# they call OpenROAD-only macros (swig_lib, messages) and assume the OpenROAD project layout — so
# we glob the sources into one static lib ourselves, as we already do for db+utl.
# Still deliberately excluded: Tcl, SWIG, or-tools, engines, GDS/CDL/3dblox (add later if needed).
set(ODB "${OPENROAD_SRC}/src/odb/src")

# The si2 LEF/DEF parsers are BISON-generated (their lexers are hand-written — no flex).
find_package(BISON REQUIRED)
bison_target(lef_parser "${ODB}/lef/lef/lef.y" "${CMAKE_CURRENT_BINARY_DIR}/lef_parser.cpp"
             COMPILE_FLAGS "--name-prefix=lefyy")
bison_target(def_parser "${ODB}/def/def/def.y" "${CMAKE_CURRENT_BINARY_DIR}/def_parser.cpp"
             COMPILE_FLAGS "--name-prefix=defyy")

file(GLOB DB_SRC  CONFIGURE_DEPENDS "${ODB}/db/*.cpp")
file(GLOB UTL_SRC CONFIGURE_DEPENDS
     "${OPENROAD_SRC}/src/utl/src/*.cpp"
     "${OPENROAD_SRC}/src/utl/src/prometheus/*.cpp")  # Logger.cpp references it unconditionally
# Drop the Tcl-glue files (need tcl.h; unused in a tcl-free libodb).
list(FILTER UTL_SRC EXCLUDE REGEX "/(MakeLogger|decode)\\.cpp$")
# LEF/DEF readers/writers + the parser libs. For lef/def, ONLY the lef/ + lefzlib/ (resp. def/ +
# defzlib/) subdirs are part of the library; the sibling subdirs (clef, lefrw, TEST, doc, …) are
# standalone tools we don't build.
file(GLOB IO_SRC CONFIGURE_DEPENDS
     "${ODB}/defin/*.cpp" "${ODB}/defout/*.cpp"
     "${ODB}/lefin/*.cpp" "${ODB}/lefout/*.cpp"
     "${ODB}/lef/lef/*.cpp" "${ODB}/lef/lefzlib/*.cpp"
     "${ODB}/def/def/*.cpp" "${ODB}/def/defzlib/*.cpp")

# The 3D structural linter (check_3dblox). ONLY checker.cpp from src/3dblox — the rest of that
# directory links yaml-cpp (the .3dbv/.3dbx parsers) and OpenSTA (3dblox.cpp's Verilog/Liberty
# path), neither of which belongs in an engine-free libodb. checker.cpp itself needs nothing
# beyond odb + utl (Logger, spatialIndex, unionFind) + spdlog/fmt + Boost geometry, all already
# linked here, and it reports through ordinary dbMarker objects.
#
# src/lint3d.cpp is OURS, and is the one file of ours compiled here rather than by the cxx
# build: it is the only place that needs odb's internal 3dblox headers, so keeping it inside
# libodb means the shim (and the prebuilt-bundle path, which ships only include/) never has to
# reach into src/3dblox. See src/lint3d.h.
set(LINT_SRC "${ODB}/3dblox/checker.cpp" "${CMAKE_CURRENT_SOURCE_DIR}/src/lint3d.cpp")

add_library(odb STATIC ${DB_SRC} ${UTL_SRC} ${IO_SRC} ${LINT_SRC}
            ${BISON_lef_parser_OUTPUT_SOURCE} ${BISON_def_parser_OUTPUT_SOURCE})
# internal 3dblox headers (checker.h) + our own src/, for LINT_SRC only — PRIVATE, so nothing
# downstream inherits a dependency on odb's internal layout.
target_include_directories(odb PRIVATE "${ODB}/3dblox" "${CMAKE_CURRENT_SOURCE_DIR}/src")
target_include_directories(odb PUBLIC
  "${OPENROAD_SRC}/src/odb/include"
  "${OPENROAD_SRC}/src/utl/include"
  "${ODB}/lef/lef" "${ODB}/lef/lefzlib"
  "${ODB}/def/def" "${ODB}/def/defzlib"
  "${CMAKE_CURRENT_BINARY_DIR}")            # generated lef_parser.hpp / def_parser.hpp
# Link fmt/spdlog/abseil (our pinned, source-built) BEFORE Boost so their headers win: a system
# Boost dir (e.g. Homebrew's /opt/homebrew/include) also bundles a *different* fmt/abseil, and if
# it came first the odb objects would reference those symbols and fail to link our pins.
target_link_libraries(odb PUBLIC
  fmt::fmt spdlog::spdlog
  absl::synchronization absl::strings absl::hash absl::flat_hash_map absl::base
  Boost::headers Boost::iostreams ZLIB::ZLIB)
# The si2 parser C code trips -Wclass-memaccess on GCC (not an error — no -Werror); silence it.
target_compile_options(odb PRIVATE $<$<CXX_COMPILER_ID:GNU>:-Wno-class-memaccess>)

# ---- OPT-IN: 3Dblox file I/O (.3dbv / .3dbx) ---------------------------------
# OFF by default, and it must stay that way: this is the ONE part of the 3D story that breaks
# the engine-free guarantee. The parsers/writers need yaml-cpp, and 3dblox.cpp needs OpenSTA —
# not as a timer, but as a Verilog parser + netlist container + Liberty reader
# (sta::ConcreteNetwork, sta::VerilogReader, readLibertyFile). That is a LINK-time dependency
# even when the ThreeDBlox ctor is handed a null Sta*, so it cannot be stubbed away cheaply.
#
# Everything else 3D — the dbChip* schema, the unfolded model, and check_3dblox — needs neither
# and is built unconditionally above.
option(VYGES_ODB_3DBLOX_IO "Build the 3Dblox .3dbv/.3dbx reader/writer (needs yaml-cpp + OpenSTA)" OFF)
if(VYGES_ODB_3DBLOX_IO)
  find_package(yaml-cpp REQUIRED)
  find_path(OPENSTA_INCLUDE_DIR sta/Sta.hh HINTS ${OPENSTA_ROOT}/include /usr/local/include)
  find_library(OPENSTA_LIBRARY NAMES OpenSTA HINTS ${OPENSTA_ROOT}/lib /usr/local/lib)
  if(NOT OPENSTA_INCLUDE_DIR OR NOT OPENSTA_LIBRARY)
    message(FATAL_ERROR
      "VYGES_ODB_3DBLOX_IO=ON needs OpenSTA (sta/Sta.hh + libOpenSTA). "
      "Set -DOPENSTA_ROOT=<prefix>, or leave the option OFF — the rest of the 3D surface "
      "(dbChip* schema, unfolded model, check_3dblox) does not need it.")
  endif()
  # checker.cpp is already in LINT_SRC above; adding it again would duplicate symbols.
  file(GLOB BLOX_SRC CONFIGURE_DEPENDS "${ODB}/3dblox/*.cpp")
  list(REMOVE_ITEM BLOX_SRC "${ODB}/3dblox/checker.cpp")
  target_sources(odb PRIVATE ${BLOX_SRC})
  # yaml-cpp exports different target names across versions/distros: modern releases give
  # yaml-cpp::yaml-cpp, older ones a bare `yaml-cpp` that may carry no INTERFACE include dir at
  # all (which fails at "yaml-cpp/yaml.h: No such file", not at link). Prefer the namespaced
  # target, and fall back to the discovered include dir so either shape works.
  if(TARGET yaml-cpp::yaml-cpp)
    set(YAMLCPP_TGT yaml-cpp::yaml-cpp)
  else()
    set(YAMLCPP_TGT yaml-cpp)
    find_path(YAMLCPP_INCLUDE_DIR yaml-cpp/yaml.h HINTS ${YAML_CPP_INCLUDE_DIR})
    if(YAMLCPP_INCLUDE_DIR)
      target_include_directories(odb PRIVATE "${YAMLCPP_INCLUDE_DIR}")
    endif()
  endif()
  target_include_directories(odb PRIVATE "${OPENSTA_INCLUDE_DIR}")
  target_link_libraries(odb PUBLIC ${YAMLCPP_TGT} "${OPENSTA_LIBRARY}")
  target_compile_definitions(odb PUBLIC VYGES_ODB_3DBLOX_IO=1)
  message(STATUS "3Dblox I/O: ON (OpenSTA=${OPENSTA_LIBRARY})")

  # Round-trip smoke: .odb -> .3dbv/.3dbx -> fresh .odb. Writing first is what gives us a real
  # 3Dblox input to read, since we have no third-party one.
  add_executable(odb_blox_roundtrip test/blox-roundtrip.cpp)
  target_include_directories(odb_blox_roundtrip PRIVATE "${ODB}/3dblox" "${OPENSTA_INCLUDE_DIR}")
  target_link_libraries(odb_blox_roundtrip PRIVATE odb)
endif()

# ---- smoke test: round-trip a real .odb --------------------------------------
option(VYGES_ODB_SMOKE "Build the .odb round-trip smoke test" OFF)
if(VYGES_ODB_SMOKE)
  add_executable(odb_smoke test/probe.cpp)
  target_link_libraries(odb_smoke PRIVATE odb)
endif()

# ---- fixture generator: synthesize a hierarchical + marker .odb for the Rust tests ----------
option(VYGES_ODB_MKFIXTURE "Build the hierarchical-fixture generator (odb_mkfixture)" OFF)
if(VYGES_ODB_MKFIXTURE)
  add_executable(odb_mkfixture test/make-hier-fixture.cpp)
  target_link_libraries(odb_mkfixture PRIVATE odb)
endif()

# ---- fixture generator: synthesize a 3D / chiplet .odb for the Rust tests -------------------
option(VYGES_ODB_MK3DFIXTURE "Build the 3D-chiplet-fixture generator (odb_mk3dfixture)" OFF)
if(VYGES_ODB_MK3DFIXTURE)
  add_executable(odb_mk3dfixture test/make-3d-fixture.cpp)
  target_link_libraries(odb_mk3dfixture PRIVATE odb)
endif()

# ---- conformance gate: OpenROAD's OWN odb C++ GTests, run against OUR libodb --
# Proves our standalone/no-Tcl build is behaviorally OpenROAD, and catches drift on a pin bump.
# Only the tests needing nothing beyond libodb build here: OpenROAD's tst/ framework pulls STA +
# Tcl + Bazel-runfiles, so the tst/helper-based, gdsin, 3dblox, and STA tests are out of scope.
# Coverage note (2026-07-24): only TestGeom builds standalone. The other 25 odb gtests pull
# OpenROAD's `tst::Fixture` framework (tst/fixture.h, {sky130,nangate45}_fixture.h,
# IntegratedFixture.h), which #includes db_sta/dbSta + sta/Liberty + tcl.h + Bazel runfiles — none
# of which exist in a Tcl/SWIG/engine-free libodb. Expanding this gate toward parity = writing a
# minimal standalone `tst` shim (db + Logger + LEF load via our lefin, no STA/Tcl); see
# docs (opendb-conformance-gate.md) in vyges-tools-internal.
option(VYGES_ODB_GTEST "Build OpenROAD's standalone odb C++ GTests against our libodb" OFF)
if(VYGES_ODB_GTEST)
  FetchContent_Declare(googletest GIT_REPOSITORY https://github.com/google/googletest.git
                       GIT_TAG v1.15.2 GIT_SHALLOW TRUE)
  set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
  FetchContent_MakeAvailable(googletest)
  set(ODB_TEST "${OPENROAD_SRC}/src/odb/test/cpp")
  add_executable(odb_gtest "${ODB_TEST}/TestGeom.cpp")   # Point/Rect/Oct/Polygon geometry
  target_link_libraries(odb_gtest PRIVATE odb GTest::gtest GTest::gtest_main GTest::gmock)
endif()
