//! Registration process replacement for the two immutable endpoints in the FILEMAP CLI test.
use serde_json::{Value, json};
use std::io::Write;

fn main() {
    assert_eq!(
        std::env::args().skip(1).collect::<Vec<_>>(),
        ["impact-views-v1"]
    );
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
