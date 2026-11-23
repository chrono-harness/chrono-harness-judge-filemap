//! Read only named semantic documents for the installed mixed-change classifier.
use crate::registration::Endpoint;
use crate::{
    Delta,
    runtime::{Program, Reader},
};
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

fn endpoint(r: &Endpoint, delta: &BTreeSet<&str>) -> Value {
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
    registration: &Endpoint,
    paths: &[String],
    present: &BTreeSet<&str>,
    mut read: impl FnMut(&str) -> Result<Value, String>,
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
            None => read(path)?,
        };
        docs.insert(path.clone(), doc);
    }
    Ok(docs)
}

pub(crate) fn for_endpoints(
    root: &Path,
    endpoints: [&Endpoint; 2],
    delta: &[Delta],
    reader: &mut Reader,
    search_path: Option<&str>,
) -> Result<Result<Value, String>, String> {
    let [base, candidate] = endpoints;
    let program = Program::resolve(root, "chrono-judge-mixed", search_path)?;
    let invoke = |operation: &str, request: Value| -> Result<Value, String> {
        let input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let output = program.run(
            root,
            operation,
            &BTreeMap::new(),
            &candidate.config()["protocol"],
            &input,
        )?;
        crate::strict_json::parse(&output)
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
        reader.document("base", p)
    })?;
    let after = documents(candidate, &selected.candidate, &new_present, |p| {
        reader.document("candidate", p)
    })?;
    let paths = delta.iter().map(|d| d.path.as_str()).collect();
    Ok(invoke(
        "classify-v1",
        json!({"base":endpoint(base,&paths),"candidate":endpoint(candidate,&paths),"delta":delta,"before":before,"after":after}),
    ))
}
