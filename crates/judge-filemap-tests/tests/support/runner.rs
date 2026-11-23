//! Emulate the installed runner at its command boundary. Git and both data commands are test executables.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{self, BufRead, Write},
    process::{Command, Stdio},
};

fn git(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new(env!("CHRONO_TEST_GIT"))
        .arg("-C")
        .arg(".")
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into())
    }
}
fn tree(commit: &str) -> Result<BTreeMap<String, (String, String)>, String> {
    let bytes = git(&["ls-tree", "-rz", "--full-tree", commit])?;
    let mut map = BTreeMap::new();
    for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let (description, path) = std::str::from_utf8(record)
            .unwrap()
            .split_once('\t')
            .unwrap();
        let mut fields = description.split_whitespace();
        let mode = fields.next().unwrap();
        fields.next();
        let blob = fields.next().unwrap();
        map.insert(path.to_owned(), (mode.to_owned(), blob.to_owned()));
    }
    Ok(map)
}
fn operation(header: &Value, q: &Value) -> Result<Value, String> {
    let which = q["endpoint"].as_str().unwrap_or("base");
    let oid = header[which].as_str().unwrap();
    match q["op"].as_str().unwrap() {
        "registry-snapshot" => {
            let ep: Value =
                serde_json::from_str(include_str!("../fixtures/registration-standard.json"))
                    .unwrap();
            let mut values = BTreeMap::new();
            let mut originals = BTreeMap::new();
            for role in ["config", "filemap", "projects", "judges", "workflow"] {
                let path = ep["original_schemas"][role][0].as_str().unwrap();
                let text = String::from_utf8(git(&["show", &format!("{oid}:{path}")])?).unwrap();
                values.insert(path, serde_json::from_str::<Value>(&text).unwrap());
                originals.insert(path, text);
            }
            Ok(
                json!({"originals":originals,"values":values,"effective_path":header["config"],"selection":null}),
            )
        }
        "delta" => {
            let a = tree(header["base"].as_str().unwrap())?;
            let b = tree(header["candidate"].as_str().unwrap())?;
            let mut changes = vec![];
            for path in a
                .keys()
                .chain(b.keys())
                .collect::<std::collections::BTreeSet<_>>()
            {
                let (old, new) = (a.get(path), b.get(path));
                if old == new {
                    continue;
                }
                changes.push(json!({"kind":if old.is_none(){"A"}else if new.is_none(){"D"}else{"M"},"path":path,
                    "old_blob":old.map(|v|&v.1),"new_blob":new.map(|v|&v.1),"old_mode":old.map(|v|&v.0),"new_mode":new.map(|v|&v.0)}));
            }
            Ok(json!(changes))
        }
        "observation" => Ok(Value::Null),
        op => panic!("unexpected fixture operation {op}"),
    }
}
fn main() {
    let mut input = io::stdin().lock();
    let mut first = String::new();
    input.read_line(&mut first).unwrap();
    let header: Value = serde_json::from_str(&first).unwrap();
    match std::env::args().nth(1).unwrap().as_str() {
        "git-facts-v1" => {
            assert_eq!(header["mode"], "config");
            if ["base", "candidate"]
                .iter()
                .any(|end| !header[*end].as_str().is_some_and(|s| s.len() == 40))
            {
                println!("{}", json!({"Err":"invalid endpoint object ID"}));
                return;
            }
            println!("{}", json!({"Ok":{"bound":false}}));
            io::stdout().flush().unwrap();
            for line in input.lines() {
                let q = serde_json::from_str(&line.unwrap()).unwrap();
                println!(
                    "{}",
                    serde_json::to_string(&operation(&header, &q)).unwrap()
                );
                io::stdout().flush().unwrap();
            }
        }
        "run-command-v1" => {
            let path = std::path::Path::new(header["program"].as_str().unwrap());
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(matches!(
                name,
                "chrono-judge-registration" | "chrono-judge-mixed"
            ));
            assert!(
                header["expected_sha256"]
                    .as_str()
                    .is_some_and(|s| s.len() == 64)
            );
            assert!(
                header["timeout_seconds"].as_u64().unwrap() > 0
                    && header["output_limit_bytes"].as_u64().unwrap() > 0
            );
            let mut child = Command::new(path)
                .arg(header["args"][0].as_str().unwrap())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            io::copy(&mut input, &mut child.stdin.take().unwrap()).unwrap();
            let out = child.wait_with_output().unwrap();
            let mut stdout = io::stdout().lock();
            writeln!(stdout,"{}",json!({"exit_code":out.status.code(),"stdout_length":out.stdout.len(),"stderr_length":out.stderr.len()})).unwrap();
            stdout.write_all(&out.stdout).unwrap();
            stdout.write_all(&out.stderr).unwrap();
        }
        op => panic!("unexpected runner command {op}"),
    }
}
