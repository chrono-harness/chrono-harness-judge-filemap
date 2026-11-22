//! Process replacement for the FILEMAP endpoint test's two classifier calls.
mod classification;
use serde_json::{Value, json};
use std::io::Write;

fn run() -> Result<Value, String> {
    let wire: Value = serde_json::from_reader(std::io::stdin().lock()).unwrap();
    let delta = wire["delta"].as_array().unwrap();
    let paths: Vec<_> = delta.iter().map(|d| d["path"].as_str().unwrap()).collect();
    assert!(paths.contains(&"src.bin"));
    match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [cmd] if cmd == "document-paths-v1" => {
            assert!(wire["base"].is_array() && wire["candidate"].is_array());
            Ok(json!({"base":[],"candidate":[]}))
        }
        [cmd] if cmd == "classify-v1" => {
            assert_eq!(wire["base"]["surfaces"]["src.bin"], "product");
            assert!(wire["candidate"]["workflow"].is_string());
            assert_eq!(wire["before"], json!({}));
            assert_eq!(wire["after"], json!({}));
            classification::reply(&paths)
        }
        _ => panic!("unexpected mixed command"),
    }
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(v) => {
            let mut out = std::io::stdout().lock();
            serde_json::to_writer(&mut out, &v).unwrap();
            out.flush().unwrap();
            0.into()
        }
        Err(e) => {
            eprintln!("{e}");
            2.into()
        }
    }
}
