//! Read the interpreted declarations that the installed registration judge owns.
use crate::NodeDefinition;
use chrono_harness::{CommandSpec, file_identity, resolve_program, run_process_observed};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    File,
    Project,
    Script,
    Input,
    Test,
    Judge,
    Tool,
    Environment,
}

#[derive(Deserialize)]
pub struct NodeView {
    pub kind: NodeKind,
    pub definitions: Vec<NodeDefinition>,
}

#[derive(Deserialize)]
pub struct Endpoint {
    pub config: Value,
    pub effective_config_path: String,
    pub projects: Value,
    pub filemap: Value,
    pub judges: Value,
    pub workflow: Value,
    pub original_schemas: BTreeMap<String, (String, u64)>,
    pub nodes: BTreeMap<String, NodeView>,
    pub active_test_aliases: BTreeMap<String, BTreeSet<String>>,
    pub claim_conflicts: BTreeMap<String, BTreeSet<String>>,
}

impl Endpoint {
    pub(crate) fn config(&self) -> &Value {
        &self.config
    }
    pub(crate) fn projects(&self) -> &Value {
        &self.projects
    }
    pub(crate) fn filemap(&self) -> &Value {
        &self.filemap
    }
    pub(crate) fn judges(&self) -> &Value {
        &self.judges
    }
    pub(crate) fn workflow(&self) -> &Value {
        &self.workflow
    }
    pub(crate) fn effective_config_path(&self) -> &str {
        &self.effective_config_path
    }
    pub(crate) fn original_schemas(&self) -> &BTreeMap<String, (String, u64)> {
        &self.original_schemas
    }
}

#[derive(Deserialize)]
pub struct Views {
    pub base: Endpoint,
    pub candidate: Endpoint,
    pub repaired: BTreeSet<String>,
    pub environments: Option<[BTreeMap<String, String>; 2]>,
    pub registration_view: Option<Value>,
    pub environment_error: Option<String>,
}

pub(crate) struct Reply {
    pub views: Result<Views, String>,
    pub git_facts: Option<Value>,
    pub position: usize,
}

pub(crate) fn read(
    root: &Path,
    config: &str,
    reader: &chrono_harness::facts::Reader,
    operation: Value,
    commits: [&str; 2],
    request_id: Option<&str>,
    search_path: Option<&str>,
) -> Result<Reply, String> {
    let originals = |documents: chrono_harness::facts::RegistrySnapshot| {
        documents
            .bytes
            .into_iter()
            .map(|(name, bytes)| {
                String::from_utf8(bytes)
                    .map(|value| (name, value))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<BTreeMap<_, _>, String>>()
    };
    let base = reader.registry_snapshot(root, commits[0], config)?;
    let candidate = reader.registry_snapshot(root, commits[1], config)?;
    let limits = candidate.values[&candidate.effective_path]["protocol"].clone();
    let originals = [originals(base)?, originals(candidate)?];
    let scheduling = operation["scheduling"] == true;
    let query =
        serde_json::to_vec(&serde_json::json!({"operation":operation,"originals":originals}))
            .map_err(|e| e.to_string())?;
    let position = reader.observation()["processes"]
        .as_array()
        .map_or(0, Vec::len);
    let path = resolve_program(root, "chrono-judge-registration", search_path)?;
    let digest = file_identity(&path)?.0;
    let env = search_path
        .map(str::to_owned)
        .or_else(|| std::env::var("PATH").ok())
        .map(|path| BTreeMap::from([("PATH".into(), path)]))
        .unwrap_or_default();
    let command = CommandSpec {
        program: path
            .to_str()
            .ok_or("registration executable path is not UTF-8")?
            .into(),
        args: vec!["impact-views-v1".into()],
        env,
        timeout_seconds: limits["timeout_seconds"]
            .as_u64()
            .ok_or("invalid registration timeout")?,
        output_limit_bytes: serde_json::from_value(limits["stdout_limit_bytes"].clone())
            .map_err(|e| e.to_string())?,
    };
    let result = run_process_observed(root, &command, &query, &digest)?;
    if let Some(error) = result.failure {
        return Err(error);
    }
    if result.exit_code != 0 {
        return Err(if result.stderr.is_empty() {
            format!("impact-views-v1 exited {}", result.exit_code)
        } else {
            result
                .stderr
                .strip_suffix('\n')
                .unwrap_or(&result.stderr)
                .to_owned()
        });
    }
    let mut value: Value = chrono_harness::decode(&result.stdout_bytes)?;
    if value["commits"] != serde_json::json!(commits)
        || value.get("request_id") != request_id.map(serde_json::Value::from).as_ref()
    {
        return Err("E_PROTOCOL: registration impact query identity".into());
    }
    let git_facts = value
        .as_object_mut()
        .and_then(|object| object.remove("git_facts"));
    let views = match value.get("error") {
        Some(error) => Err(error
            .as_str()
            .ok_or("invalid registration error")?
            .to_owned()),
        None if scheduling && git_facts.is_none() => {
            Err("missing registration scheduling Git facts".into())
        }
        None if request_id.is_some()
            && value.get("environments").is_some() == value.get("environment_error").is_some() =>
        {
            Err("missing or conflicting registration environment result".into())
        }
        None => serde_json::from_value(value)
            .map_err(|e| format!("invalid registration impact view: {e}")),
    };
    Ok(Reply {
        views,
        git_facts,
        position,
    })
}

/// Insert the query's processes at the point it ran. Keep a single binding and original process order.
pub(crate) fn with_registration_facts(
    mut local: Value,
    at: usize,
    other: Option<Value>,
) -> Result<Value, String> {
    let Some(mut other) = other else {
        return Ok(local);
    };
    if local.is_null() && other.is_null() {
        return Ok(Value::Null);
    }
    let left = local
        .as_object_mut()
        .ok_or("E_GIT_FACTS: unbound local reader")?;
    let right = other
        .as_object_mut()
        .ok_or("E_GIT_FACTS: unbound registration reader")?;
    let mut own = left
        .remove("processes")
        .ok_or("E_GIT_FACTS: missing local processes")?;
    let extra = right
        .remove("processes")
        .ok_or("E_GIT_FACTS: missing registration processes")?;
    if left != right {
        return Err("E_GIT_FACTS: registration and FILEMAP bindings differ".into());
    }
    let processes = own
        .as_array_mut()
        .ok_or("E_GIT_FACTS: invalid local processes")?;
    let Value::Array(extra) = extra else {
        return Err("E_GIT_FACTS: invalid registration processes".into());
    };
    if at > processes.len() {
        return Err("E_GIT_FACTS: missing reader prefix".into());
    }
    processes.splice(at..at, extra);
    left.insert("processes".into(), own);
    Ok(local)
}
