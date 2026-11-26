//! Read only named semantic documents for the installed mixed-change classifier.
#![allow(unexpected_cfgs)]

pub mod policy {
    use vstd::prelude::*;
    verus! {
    #[derive(Clone, Copy)]
    pub enum Side { Base, Candidate }
    pub enum Source { Invalid, Config, Judges, Projects, Filemap, Workflow, Read(&'static str) }

    /// Only a named path with a blob at this endpoint can supply a document.
    pub fn path_available(side: Side, named: bool, old_blob: bool, new_blob: bool) -> (ok: bool)
        ensures ok == (named && match side { Side::Base => old_blob, Side::Candidate => new_blob }),
    {
        named && match side { Side::Base => old_blob, Side::Candidate => new_blob }
    }

    /// A registration document is already decoded for this endpoint. Later
    /// registry roles retain the original precedence when they share a path.
    pub fn select_source(side: Side, present: bool, repeated: bool, config: bool,
        judges: bool, projects: bool, filemap: bool, workflow: bool) -> (source: Source)
        ensures match source {
            Source::Invalid => !present || repeated,
            Source::Workflow => present && !repeated && workflow,
            Source::Filemap => present && !repeated && !workflow && filemap,
            Source::Projects => present && !repeated && !workflow && !filemap && projects,
            Source::Judges => present && !repeated && !workflow && !filemap && !projects && judges,
            Source::Config => present && !repeated && !workflow && !filemap && !projects && !judges && config,
            Source::Read(endpoint) => present && !repeated && !workflow && !filemap && !projects && !judges && !config
                && endpoint@ == match side { Side::Base => "base"@, Side::Candidate => "candidate"@ },
        },
    {
        if !present || repeated { Source::Invalid }
        else if workflow { Source::Workflow }
        else if filemap { Source::Filemap }
        else if projects { Source::Projects }
        else if judges { Source::Judges }
        else if config { Source::Config }
        else { Source::Read(match side { Side::Base => "base", Side::Candidate => "candidate" }) }
    }
    }
}

#[cfg(not(verus_keep_ghost))]
pub(crate) use native::for_endpoints;

#[cfg(not(verus_keep_ghost))]
mod native {
    use super::policy::{Side, Source, path_available, select_source};
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
        side: Side,
        registration: &Endpoint,
        paths: &[String],
        present: &BTreeSet<&str>,
        reader: &mut Reader,
    ) -> Result<BTreeMap<String, Value>, String> {
        let mut docs = BTreeMap::new();
        for path in paths {
            let registries = &registration.config()["registries"];
            let source = select_source(
                side,
                present.contains(path.as_str()),
                docs.contains_key(path),
                registration.effective_config_path() == path,
                registries["judges"] == *path,
                registries["projects"] == *path,
                registries["filemap"] == *path,
                registries["workflow"] == *path,
            );
            let doc = match source {
                Source::Invalid => {
                    return Err(format!("invalid document-paths-v1 reply for {path}"));
                }
                Source::Config => registration.config().clone(),
                Source::Judges => registration.judges().clone(),
                Source::Projects => registration.projects().clone(),
                Source::Filemap => registration.filemap().clone(),
                Source::Workflow => registration.workflow().clone(),
                Source::Read(endpoint) => reader.document(endpoint, path)?,
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
        let read = |side, registration, requested, reader: &mut Reader| {
            let available = delta
                .iter()
                .filter(|d| {
                    path_available(
                        side,
                        named.contains(d.path.as_str()),
                        d.old_blob.is_some(),
                        d.new_blob.is_some(),
                    )
                })
                .map(|d| d.path.as_str())
                .collect();
            documents(side, registration, requested, &available, reader)
        };
        let before = read(Side::Base, base, &selected.base, reader)?;
        let after = read(Side::Candidate, candidate, &selected.candidate, reader)?;
        let paths = delta.iter().map(|d| d.path.as_str()).collect();
        Ok(invoke(
            "classify-v1",
            json!({"base":endpoint(base,&paths),"candidate":endpoint(candidate,&paths),"delta":delta,"before":before,"after":after}),
        ))
    }
}
