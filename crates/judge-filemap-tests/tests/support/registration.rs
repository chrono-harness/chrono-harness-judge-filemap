//! Registration process replacement for the two immutable endpoints in the FILEMAP CLI test.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::{prelude::*, string::StringSliceAdditionalSpecFns};

#[cfg(verus_keep_ghost)]
use crate::policy::equal_text;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_filemap::verified_graph::equal_text;

verus! {
pub enum RequestKind { Cost, Impact, Invalid }

pub fn request_kind(count: usize, operation: &str) -> (kind: RequestKind)
    ensures match kind {
        RequestKind::Cost => count == 1 && operation@ == "filemap-views"@,
        RequestKind::Impact => count == 1 && operation@ == "impact-views-v1"@,
        RequestKind::Invalid => count != 1 || (operation@ != "filemap-views"@ && operation@ != "impact-views-v1"@),
    },
{
    if count != 1 { RequestKind::Invalid }
    else if equal_text(operation, "filemap-views") { RequestKind::Cost }
    else if equal_text(operation, "impact-views-v1") { RequestKind::Impact }
    else { RequestKind::Invalid }
}
}

#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::io::Write;

#[cfg(not(verus_keep_ghost))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let operation = args.first().map(String::as_str).unwrap_or_default();
    match request_kind(args.len(), operation) {
        RequestKind::Cost => cost_reply(),
        RequestKind::Impact => impact_reply(),
        RequestKind::Invalid => panic!("unexpected registration arguments: {args:?}"),
    }
}

#[cfg(not(verus_keep_ghost))]
fn cost_reply() {
    let input: Value = serde_json::from_reader(std::io::stdin().lock()).unwrap();
    let expected: Value = serde_json::from_slice(&std::fs::read("cost-request").unwrap()).unwrap();
    assert_eq!(input, expected);
    std::io::stdout()
        .write_all(&std::fs::read("cost-reply").unwrap())
        .unwrap();
}

#[cfg(not(verus_keep_ghost))]
fn impact_reply() {
    let input: Value = serde_json::from_reader(std::io::stdin().lock()).unwrap();
    let op = &input["operation"];
    assert_eq!(op["mode"], "endpoints");
    assert_eq!(op["scheduling"], false);
    let endpoint: Value =
        serde_json::from_str(include_str!("../fixtures/registration-standard.json")).unwrap();
    for documents in input["originals"].as_array().unwrap() {
        for role in ["config", "projects", "filemap", "judges", "workflow"] {
            let path = endpoint["original_schemas"][role][0].as_str().unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(documents[path].as_str().unwrap()).unwrap(),
                endpoint[role]
            );
        }
    }
    let mut reply = json!({"commits":[op["base"],op["candidate"]],"base":endpoint,"candidate":endpoint,"repaired":[]});
    let mode = std::fs::read_to_string("registration-reply").unwrap_or_default();
    match mode.as_str() {
        "" => {}
        "wrong-identity" => reply["commits"][0] = json!("0".repeat(40)),
        "incomplete" => {
            reply["candidate"].as_object_mut().unwrap().remove("nodes");
        }
        "duplicate" => {
            print!("{{\"error\":\"one\",\"error\":\"two\"}}");
            return;
        }
        "error" => {
            eprintln!("registry rejected");
            std::process::exit(2);
        }
        _ => panic!("unexpected mock reply"),
    }
    let mut out = std::io::stdout().lock();
    serde_json::to_writer(&mut out, &reply).unwrap();
    out.flush().unwrap();
}
