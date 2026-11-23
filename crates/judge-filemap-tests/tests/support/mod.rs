use chrono_judge_filemap::Delta;
use chrono_judge_filemap::registration::Views;
use serde_json::{Value, json};
use std::fs;
use std::sync::LazyLock;

pub const FM: &str = ".chrono-harness/FILEMAP.json";
pub const CONFIG: &str = ".chrono-harness/config.json";
pub const PROJECTS: &str = ".chrono-harness/projects.json";
pub const JUDGES: &str = ".chrono-harness/judges.json";

pub fn edge(from: &str, kind: &str, to: &str) -> Value {
    json!({"from":from,"kind":kind,"to":to})
}

pub fn standard() -> Value {
    serde_json::from_str(include_str!("../fixtures/registration-standard.json")).unwrap()
}

pub fn views(base: &str, candidate: &str) -> Views {
    #[derive(serde::Deserialize)]
    struct Variant {
        from: String,
        patch: json_patch::Patch,
    }
    static VARIANTS: LazyLock<std::collections::BTreeMap<String, Variant>> = LazyLock::new(|| {
        serde_json::from_str(include_str!("../fixtures/registration-variants.json")).unwrap()
    });
    fn endpoint(name: &str) -> Value {
        if name == "standard" {
            return standard();
        }
        let variant = &VARIANTS[name];
        let mut result = endpoint(&variant.from);
        json_patch::patch(&mut result, &variant.patch).unwrap();
        result
    }
    serde_json::from_value(
        json!({"base":endpoint(base),"candidate":endpoint(candidate),"repaired":[]}),
    )
    .unwrap()
}
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
pub fn fixture_bin() -> std::path::PathBuf {
    let search_path = std::env::var_os("PATH").expect("test process PATH");
    let fixture_dir = std::env::split_paths(&search_path)
        .next()
        .expect("fixture directory");
    let selected = fixture_dir
        .join("git")
        .canonicalize()
        .expect("run tests with ./check.sh");
    assert_eq!(
        selected,
        std::path::Path::new(env!("CHRONO_TEST_GIT"))
            .canonicalize()
            .unwrap(),
        "test Git fixture must be first on PATH"
    );
    fixture_dir
}
pub fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new(fixture_bin().join("git"));
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
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
pub fn init_endpoints(root: &Path) {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let bin = fixture_bin();
        for (mock, command) in [
            (
                env!("CARGO_BIN_EXE_chrono-test-filemap-runner"),
                "chrono-harness",
            ),
            (
                env!("CARGO_BIN_EXE_chrono-test-mixed"),
                "chrono-judge-mixed",
            ),
            (
                env!("CARGO_BIN_EXE_chrono-test-registration"),
                "chrono-judge-registration",
            ),
        ] {
            std::os::unix::fs::symlink(mock, bin.join(command)).unwrap();
        }
    });
    git(root, &["init", "-q"]);
    let endpoint = standard();
    for role in ["config", "filemap", "projects", "judges", "workflow"] {
        let path = endpoint["original_schemas"][role][0].as_str().unwrap();
        let value = &endpoint[role];
        let dest = root.join(path);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::write(dest, serde_json::to_vec(&value).unwrap()).unwrap();
    }
}

// Read the actual change set FILEMAP sent to its classifier for this pair of commits.
pub fn observed_delta(root: &Path, base: &str, candidate: &str) -> Result<Vec<Delta>, String> {
    let query =
        serde_json::to_vec(&json!({"config":CONFIG,"base":base,"candidate":candidate})).unwrap();
    let capture = root.join(".git/classifier-delta.json");
    if capture.exists() {
        fs::remove_file(&capture).unwrap();
    }
    chrono_judge_filemap::endpoint_tests(root, query.as_slice())?;
    Ok(serde_json::from_slice(&fs::read(capture).unwrap()).unwrap())
}
