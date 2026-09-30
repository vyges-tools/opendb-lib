// SPDX-License-Identifier: Apache-2.0
//! `build.rs`'s vendor-commit check must answer for the vendor tree itself, never for a repository
//! that encloses it (see `build_support/tree_sha.rs`).
#[path = "../build_support/tree_sha.rs"]
mod tree_sha;
use std::path::{Path, PathBuf};
use std::process::Command;
use tree_sha::tree_sha;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vyges-tree-sha-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A one-commit repository at `dir`; returns its HEAD.
///
/// ⚠️ The message is the path: two empty commits made in the same second with the same message
/// and identity hash to the SAME commit, and the nested case could not tell them apart.
fn init_repo(dir: &Path) -> String {
    let git = |args: &[&str]| {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(&["init", "-q"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", dir.to_str().unwrap()]);
    git(&["rev-parse", "HEAD"])
}

#[test]
fn a_checkout_root_reports_its_own_head() {
    let d = scratch("root");
    let head = init_repo(&d);
    assert_eq!(tree_sha(&d), Some(head));
    std::fs::remove_dir_all(&d).unwrap();
}

/// The 2026-09-29 case: `vendor/OpenROAD` without a `.git`, inside the opendb-lib checkout.
#[test]
fn a_tree_without_its_own_git_does_not_report_the_enclosing_repo() {
    let d = scratch("nested");
    let outer = init_repo(&d);
    let vendor = d.join("vendor/OpenROAD");
    std::fs::create_dir_all(&vendor).unwrap();
    assert_ne!(tree_sha(&vendor), Some(outer));
    assert_eq!(tree_sha(&vendor), None);
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn a_nested_checkout_reports_its_own_head_not_the_outer_one() {
    let d = scratch("both");
    let outer = init_repo(&d);
    let vendor = d.join("vendor/OpenROAD");
    std::fs::create_dir_all(&vendor).unwrap();
    let inner = init_repo(&vendor);
    assert_ne!(inner, outer);
    assert_eq!(tree_sha(&vendor), Some(inner));
    std::fs::remove_dir_all(&d).unwrap();
}
