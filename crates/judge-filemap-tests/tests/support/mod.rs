use chrono_harness::facts;
use chrono_harness::wire::Delta;
pub use chrono_registration_fixture::*;
use std::{path::Path, process::Command};
pub fn delta(path: &str) -> Delta {
    Delta {
        kind: "M".into(),
        path: path.into(),
        old_blob: Some("a".repeat(40)),
        new_blob: Some("b".repeat(40)),
        old_mode: Some("100644".into()),
        new_mode: Some("100644".into()),
    }
}
pub fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new(env!("CHRONO_TEST_GIT"));
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    chrono_harness::process_fds::forward_command(&mut command, &[]).unwrap();
    let o = command.output().unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8(o.stdout).unwrap().trim().into()
}
pub fn commit(root: &Path) -> String {
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--no-gpg-sign",
            "-qm",
            "fixture",
            "--allow-empty",
        ],
    );
    git(root, &["rev-parse", "HEAD"])
}
pub fn git_delta(root: &Path, b: &str, c: &str) -> Vec<Delta> {
    facts::delta(
        &facts::tree(root, b).unwrap(),
        &facts::tree(root, c).unwrap(),
    )
}
