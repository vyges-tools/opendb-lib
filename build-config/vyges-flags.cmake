# SPDX-License-Identifier: Apache-2.0
# Flags shared by every CMake-built native library: the CMake counterpart of
# bazel/common.bazelrc. Include it BEFORE project() (the macOS deployment target must be known
# when the compiler is probed) and before any FetchContent, so fetched dependencies inherit it:
#
#   cmake_minimum_required(VERSION 3.20)
#   include(${CMAKE_CURRENT_LIST_DIR}/build-config/cmake/vyges-flags.cmake)
#   project(... LANGUAGES CXX)
#
# scripts/check.py accepts either that include (of a copy identical to this file) or the same
# settings written inline. Values shared with versions.toml must agree: `scripts/check.py --self`.

# One C++ standard for the library and its fetched dependencies (abseil changes its ABI with it).
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Position-independent objects, so the static archive can also go into a Rust cdylib.
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

# Release unless the caller chose: CMake's Release flags are -O3 -DNDEBUG for Clang and GCC,
# matching the reference's -O3. (The Rust `cmake` crate must also ask for Release explicitly:
# left alone it picks RelWithDebInfo, -O2, whenever Cargo debug info is on.)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

# No floating-point contraction on any C or C++ source, ours or fetched. A contracted a*b+c
# rounds once instead of twice; Clang fuses it by default wherever the target has FMA, which
# aarch64 has in its baseline (Linux and macOS) and x86-64 has under an FMA -march. Checked
# 2026-11-16: -ffp-contract=off gives two roundings on every one of those targets; rustc never
# contracts. add_compile_options (not a C++-only variable) so C sources are covered too.
add_compile_options(-ffp-contract=off)

# Oldest macOS the archive runs on: the macOS release we build and test on; matches
# bazel/common.bazelrc --macos_minimum_os.
if(APPLE AND NOT CMAKE_OSX_DEPLOYMENT_TARGET)
  set(CMAKE_OSX_DEPLOYMENT_TARGET 15.0)
endif()
