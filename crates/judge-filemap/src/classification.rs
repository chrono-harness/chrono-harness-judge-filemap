//! Read only named semantic documents for the installed mixed-change classifier.
use chrono_harness::{
    CommandSpec, facts, file_identity, resolve_program, run_process_observed, wire::Delta,
};
use chrono_judge_registration::Registrations;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Paths {
    base: Vec<String>,
    candidate: Vec<String>,
}

fn endpoint(r: &Registrations, delta: &BTreeSet<&str>) -> Value {
    let mut surfaces = BTreeMap::new();
    for file in r.filemap()["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        if delta.contains(path) {
            surfaces.entry(path).or_insert(&file["surface"]);
        }
    }
    json!({"surfaces":surfaces,"workflow":r.config()["registries"]["workflow"],"semantic_fields":r.config()["semantic_fields"]})
}

// Registration resolves selectors and historical conversions before classification.
// External documents must be read with this invocation's Git reader.
fn documents(
    registration: &Registrations,
    paths: &[String],
    present: &BTreeSet<&str>,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut docs = BTreeMap::new();
    for path in paths {
        if !present.contains(path.as_str()) || docs.contains_key(path) {
            return Err(format!("invalid document-paths-v1 reply for {path}"));
        }
        let mut value =
            (registration.effective_config_path() == path).then(|| registration.config());
        for (role, document) in [
            ("judges", registration.judges()),
            ("projects", registration.projects()),
            ("filemap", registration.filemap()),
            ("workflow", registration.workflow()),
        ] {
            if registration.config()["registries"][role] == *path {
                value = Some(document);
            }
        }
        let doc = match value {
            Some(value) => value.clone(),
            None => chrono_harness::decode(&read(path)?)
                .map_err(|e| format!("E_SEMANTIC_INPUT: {path}: {e}"))?,
        };
        docs.insert(path.clone(), doc);
    }
    Ok(docs)
}

pub(crate) fn for_endpoints(
    root: &Path,
    commits: [&str; 2],
    endpoints: [&Registrations; 2],
    delta: &[Delta],
    reader: &facts::Reader,
    search_path: Option<&str>,
) -> Result<Result<Value, String>, String> {
    let [base, candidate] = endpoints;
    let path = resolve_program(root, "chrono-judge-mixed", search_path)?;
    let digest = file_identity(&path)?.0;
    let policy = &candidate.config()["protocol"];
    let mut spec = CommandSpec {
        program: path
            .to_str()
            .ok_or("mixed executable path is not UTF-8")?
            .into(),
        args: vec![],
        env: BTreeMap::new(),
        timeout_seconds: policy["timeout_seconds"]
            .as_u64()
            .ok_or("invalid protocol timeout")?,
        output_limit_bytes: policy["stdout_limit_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("invalid protocol output bound")?,
    };
    let mut invoke = |operation: &str, request: Value| -> Result<Value, String> {
        spec.args = vec![operation.into()];
        let input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let result = run_process_observed(root, &spec, &input, &digest)?;
        if let Some(failure) = result.failure {
            return Err(failure);
        }
        if result.exit_code != 0 {
            return Err(if result.stderr.trim_end().is_empty() {
                format!("{operation} exited {}", result.exit_code)
            } else {
                result.stderr.trim_end().into()
            });
        }
        chrono_harness::decode(&result.stdout_bytes)
    };
    let plan = json!({
        "base": base.config()["semantic_fields"],
        "candidate": candidate.config()["semantic_fields"],
        "delta": delta,
    });
    let selected: Paths = serde_json::from_value(invoke("document-paths-v1", plan)?)
        .map_err(|e| format!("invalid document paths: {e}"))?;
    let named: BTreeSet<_> = [base, candidate]
        .iter()
        .flat_map(|endpoint| endpoint.config()["semantic_fields"].as_array().unwrap())
        .filter(|rule| !rule["pointers"].as_array().unwrap().is_empty())
        .map(|rule| rule["path"].as_str().unwrap())
        .collect();
    let old_present = delta
        .iter()
        .filter(|d| d.old_blob.is_some() && named.contains(d.path.as_str()))
        .map(|d| d.path.as_str())
        .collect();
    let new_present = delta
        .iter()
        .filter(|d| d.new_blob.is_some() && named.contains(d.path.as_str()))
        .map(|d| d.path.as_str())
        .collect();
    let before = documents(base, &selected.base, &old_present, |p| {
        reader.blob(root, commits[0], p)
    })?;
    let after = documents(candidate, &selected.candidate, &new_present, |p| {
        reader.blob(root, commits[1], p)
    })?;
    let paths = delta.iter().map(|d| d.path.as_str()).collect();
    Ok(invoke(
        "classify-v1",
        json!({"base":endpoint(base,&paths),"candidate":endpoint(candidate,&paths),"delta":delta,"before":before,"after":after}),
    ))
}
