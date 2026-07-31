# vyges-opendb-lib

Low-level build + (future) FFI bindings for **OpenROAD's OpenDB (`libodb`)** — built
**standalone** from a pinned, sparse OpenROAD subtree, with **no Tcl, no SWIG, and no
OpenROAD engines**. The safe Rust API lives in the sibling crate `vyges-opendb`.

> Part of Vyges Loom. `libodb` is the in-memory design database every OpenROAD engine reads
> and writes; binding it lets Loom do ECO/audit/extraction natively over `.odb` — and it
> carries the LEF/DEF/GDS/CDL I/O with it.

## What this repo produces

A single static `libodb.a` (the OpenDB core + the `utl` logger), buildable on
**linux/x86_64, linux/arm64, and macOS/Apple Silicon**. Verified: it reads a real placed +
routed `.odb`, walks the model, and writes it back — linking none of the engines.

## How it works — pinned + sparse, no full mirror

- **`openroad-pin.yaml`** pins the OpenROAD commit (matches the `vyges-opendb` distribution).
- **`scripts/fetch-odb-src.sh`** does a blobless, cone-sparse checkout of only `src/odb` +
  `src/utl` + `cmake` at that SHA — **~24 MB**, not the ~1.8 GB full tree.
- **`CMakeLists.txt`** compiles the db core + utl into `libodb.a` (C++20), linking Boost
  (headers), zlib, spdlog, fmt, abseil. Tcl/SWIG/or-tools/engines are deliberately excluded.
- **`.github/workflows/build-libodb.yml`** builds + smoke-tests + publishes `libodb.a`
  per-arch, on demand (`workflow_dispatch`).

## Two ways to get libodb

- **Prebuilt (fast, no toolchain-heavy build):** download the per-arch bundle the
  `build-libodb` workflow publishes (`lib/libodb.a` + `include/{odb,utl}` + the OpenROAD
  license), extract it, and set `VYGES_ODB_PREBUILT_DIR=<dir>`. `build.rs` links the archive
  directly — no cmake, no OpenROAD fetch, no `libodb` compile. This is the path for consumers
  (e.g. `vyges-cli`) and CI.
- **From source (default):** `scripts/fetch-odb-src.sh` + CMake, below.

## Build locally

```sh
scripts/fetch-odb-src.sh                 # sparse-checkout the pinned subtree -> vendor/OpenROAD
cmake -S . -B build -DVYGES_ODB_SMOKE=ON
cmake --build build -j
./build/odb_smoke test/fixtures/counter.odb /tmp/rt.odb   # -> block=counter insts=229 ...
```

Deps: a C++20 compiler + `cmake boost zlib abseil spdlog fmt` (apt `lib*-dev`, or
`brew install`).

## Scope

- **v0 (now):** the db core — the in-memory model + `.odb` read/write (dbDatabase, the ECO
  journal, wire codec, RC, connectivity). Enough for the odb applier + audit steps.
- **v1 (next):** add the LEF/DEF/GDS/CDL I/O sub-libs (`defin/lefin/gdsin/...`).
- **bindings (next):** the `cxx` bridge (this crate's `src/`) + the safe `vyges-opendb` API.

## 3D / chiplet (ODB 3D-IC)

The pinned ODB carries the 3D chiplet schema — one design is multiple dies bonded together,
each with its own `dbTech`. These classes are in `db.h` and compile into `libodb.a` like any
other, so they need no special build. Bound so far:

| Folded | Keyed by |
| --- | --- |
| `dbChip` | chip name |
| `dbChipInst` | parent chip, inst |
| `dbChipRegion` | chip, region |
| `dbChipRegionInst` | chip, inst, region |
| `dbChipBump` | chip, region, index |
| `dbChipConn` / `dbChipNet` / `dbChipPath` | chip, name |

| Unfolded (derived) | Keyed by |
| --- | --- |
| `dbUnfoldedChipInst` | slash-joined chip-inst path |
| `dbUnfoldedChipRegionInst` | path, index |
| `dbUnfoldedChipBumpInst` | path, region index, index |
| `dbUnfoldedChipConn` / `dbUnfoldedChipNet` | index |

`dbChipBumpInst` is deliberately **not** bound: every accessor it has returns an unnameable
type, so it would contribute no fields. The same information is reachable through `dbChipBump`
(folded) or `dbUnfoldedChipBumpInst` (absolute positions).

Four things here will trip you up if they are not written down.

### `dbChip::ChipType` — we generate the string mapping, and it is UPPERCASE

odb ships **no `getString()`** for `dbChip::ChipType`, so the mapping to text is the caller's
problem. odb solves it for itself three separate times — in `3dblox/dbvWriter.cpp`, in the
3Dblox parser, and in its own Python SWIG typemap (`swig/python/dbenums.i`) — so generating
one here is the sanctioned pattern, not a workaround. `ENUM_MAPPED` in
`scripts/generate-bindings.py` handles this generically: any `getString()`-less enum is one
table entry plus a generated helper.

**The vocabulary we emit is UPPERCASE — `DIE`, `RDL`, `IP`, `SUBSTRATE`, `HIER`** — matching
both our other enums (`dbSigType` → `"SIGNAL"`) and OpenROAD's own Python bindings.

Note that the **3Dblox file format spells these lowercase** (`die`, `rdl`, `ip`, `substrate`,
`hier`). That is the `.3dbv` writer's representation of the value, not the database API's, and
this is a database binding — so the two disagree on purpose. **Anything that later reads or
writes 3Dblox files will need the lowercase form, in the other direction.** Do not "fix" one
to match the other.

The generated helper is an if-chain rather than a `switch`: an unrecognised value falls through
to `""` instead of tripping `-Wswitch`. An OpenROAD pin bump can add enumerators, and a blank
is safer than silently reporting the wrong type.

### `dbChipInst::setLoc` is orientation-dependent — set the orientation FIRST

`setLoc` **does not store the point you give it.** It orients the master chip's cuboid, then
stores the *delta* that lands that cuboid's lower-left-lower corner on your point. `getLoc()`
is `getCuboid().lll()`, which re-applies the **current** orientation.

So `setLoc` and `getLoc` are not the symmetric pair the names suggest, and:

```cpp
inst->setLoc(Point3D(1000, 2000, 3000));
inst->setOrient(dbOrientType3D(dbOrientType::R90, true));   // WRONG ORDER
// getLoc() now reads (-39000, 2000, 2300) — the chip moved, with no error or warning

inst->setOrient(dbOrientType3D(dbOrientType::R90, true));
inst->setLoc(Point3D(1000, 2000, 3000));                    // correct
// getLoc() reads (1000, 2000, 3000)
```

Because of this coupling, **`dbChipInst` deliberately exposes no setters at all** in the
generated write surface. `setLoc` takes a `Point3D`, which is not a marshallable setter param,
so we could only have emitted `setOrient` — and a caller who re-oriented a placed chip inst
would have silently moved it with no way to put it back. Exposing only the destructive half of
a coupled pair is worse than exposing neither, so `TARGETS` withholds it via `skip_setters`.
`tests/generated_write.rs` in the sibling crate pins that decision so a regeneration cannot
hand the footgun back.

**TODO:** marshal `Point3D` as a setter param, then expose both `setLoc` and `setOrient`
together and document the orient-before-loc ordering at the API surface.

### Regions and bumps must exist before the chip inst that uses them

A second, unrelated ordering trap. `dbChipInst::create` walks the **master chip's** regions and
bumps and derives the matching `dbChipRegionInst` / `dbChipBumpInst` there and then. Regions
added to the master *afterwards* are simply not instantiated for that inst — silently, with no
error. Build a chip's surfaces first, then instantiate it.

### The unfolded model is derived, but it is rebuilt for you on read

`dbUnfolded*` is `constructUnfoldedModel()`'s output — the hierarchy flattened to absolute
positions, which is what linting, the 3D viewer and full-chip analysis consume. It is **never
serialised**. You do not have to call anything, though: `_dbDatabase::operator>>` runs
`constructUnfoldedModel()` on read whenever the database holds more than one chip, so the
unfolded accessors answer straight after a plain open.

The one prerequisite is that the database's **top chip** is the assembly. `dbUnfoldedBuilder`
starts from `dbDatabase::getChip()` and walks its chip insts, so if the top chip is still some
flat design that has no chip insts, every unfolded table comes back empty and nothing tells you
why.

## Notes

- **C++20 required** — odb headers use `operator<=>` and `<numbers>`.
- `utl` bundles a self-contained Prometheus metrics server (Boost.Asio + std only) that
  `Logger.cpp` constructs unconditionally, so it is compiled in; it pulls no external
  prometheus-cpp.
- OpenROAD is BSD-3-Clause; this repo (our CMake, scripts, workflow, bindings) is Apache-2.0.
