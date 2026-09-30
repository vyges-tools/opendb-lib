// SPDX-License-Identifier: Apache-2.0
//! The commit an OpenROAD tree is checked out at — shared by `build.rs` and its test.
use std::path::Path;

/// The checked-out SHA of `dir`, or None when `dir` is not itself the root of a git checkout.
///
/// ⛔ **`git -C dir rev-parse HEAD` alone answers for whatever repository ENCLOSES `dir`.** A
/// `vendor/OpenROAD` copied without its `.git` (rsync leaves `.git` out) has none of its own, so git
/// walks up and reports the enclosing checkout's HEAD instead. On 2026-09-29 the box's opendb-lib
/// copy refused its build with "vendor/OpenROAD is at 3587d5b" — an opendb-lib commit, not an
/// OpenROAD one. The top level must be `dir` itself before its HEAD is believed.
pub fn tree_sha(dir: &Path) -> Option<String> {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let top = std::fs::canonicalize(git(&["rev-parse", "--show-toplevel"])?).ok()?;
    if top != std::fs::canonicalize(dir).ok()? {
        return None;
    }
    git(&["rev-parse", "HEAD"])
}
