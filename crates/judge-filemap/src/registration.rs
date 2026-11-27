//! Read the interpreted declarations that the installed registration judge owns.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// A provider error takes precedence over missing scheduling or environment evidence.
pub enum ReplyFields { ProviderError, MissingFacts, EnvironmentConflict, Views }

pub fn reply_fields(provider_error: bool, scheduling: bool, facts: bool,
    bound: bool, environments: bool, environment_error: bool) -> (field: ReplyFields)
    ensures field == if provider_error { ReplyFields::ProviderError }
        else if scheduling && !facts { ReplyFields::MissingFacts }
        else if bound && environments == environment_error { ReplyFields::EnvironmentConflict }
        else { ReplyFields::Views },
{
    if provider_error { ReplyFields::ProviderError }
    else if scheduling && !facts { ReplyFields::MissingFacts }
    else if bound && environments == environment_error { ReplyFields::EnvironmentConflict }
    else { ReplyFields::Views }
}
}
#[cfg(not(verus_keep_ghost))]
use crate::NodeDefinition;
#[cfg(not(verus_keep_ghost))]
use crate::runtime::{Program, Reader};
#[cfg(not(verus_keep_ghost))]
use crate::verified_graph::registration_environment;
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::Value;
#[cfg(not(verus_keep_ghost))]
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
pub struct NodeView {
    pub kind: NodeKind,
    pub definitions: Vec<NodeDefinition>,
}

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
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

#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
pub struct Views {
    pub base: Endpoint,
    pub candidate: Endpoint,
    pub repaired: BTreeSet<String>,
    pub environments: Option<[BTreeMap<String, String>; 2]>,
    pub registration_view: Option<Value>,
    pub environment_error: Option<String>,
}

#[cfg(not(verus_keep_ghost))]
pub(crate) struct Reply {
    pub views: Result<Views, String>,
    pub git_facts: Option<Value>,
    pub position: usize,
}

#[cfg(not(verus_keep_ghost))]
pub(crate) fn read(
    root: &Path,
    reader: &mut Reader,
    operation: Value,
    commits: [&str; 2],
    request_id: Option<&str>,
    search_path: Option<&str>,
) -> Result<Reply, String> {
    let base = reader.snapshot("base")?;
    let candidate = reader.snapshot("candidate")?;
    let limits = candidate.values[&candidate.effective_path]["protocol"].clone();
    let originals = [base.originals, candidate.originals];
    let scheduling = operation["scheduling"] == true;
    let query =
        serde_json::to_vec(&serde_json::json!({"operation":operation,"originals":originals}))
            .map_err(|e| e.to_string())?;
    let (position, env) = {
        let facts = reader.observation()?;
        let position = facts["processes"].as_array().map_or(0, Vec::len);
        let inherited = (scheduling && reader.bound).then(|| &facts["environment"]["inherited"]);
        let path = search_path
            .map(str::to_owned)
            .or_else(|| std::env::var("PATH").ok());
        (
            position,
            registration_environment::child_environment(path, inherited)?,
        )
    };
    let program = Program::resolve(root, "chrono-judge-registration", search_path)?;
    let output = program.run(root, "impact-views-v1", &env, &limits, &query)?;
    let mut value: Value = crate::strict_json::parse(&output)?;
    if value["commits"] != serde_json::json!(commits)
        || !crate::request::request_id_matches(
            request_id,
            value.get("request_id").map(Value::as_str),
        )
    {
        return Err("E_PROTOCOL: registration impact query identity".into());
    }
    let git_facts = value
        .as_object_mut()
        .and_then(|object| object.remove("git_facts"));
    let views = match reply_fields(
        value.get("error").is_some(),
        scheduling,
        git_facts.is_some(),
        request_id.is_some(),
        value.get("environments").is_some(),
        value.get("environment_error").is_some(),
    ) {
        ReplyFields::ProviderError => Err(value["error"]
            .as_str()
            .ok_or("invalid registration error")?
            .to_owned()),
        ReplyFields::MissingFacts => Err("missing registration scheduling Git facts".into()),
        ReplyFields::EnvironmentConflict => {
            Err("missing or conflicting registration environment result".into())
        }
        ReplyFields::Views => serde_json::from_value(value)
            .map_err(|e| format!("invalid registration impact view: {e}")),
    };
    Ok(Reply {
        views,
        git_facts,
        position,
    })
}

#[cfg(not(verus_keep_ghost))]
type Facts =
    crate::verified_graph::registration_trace::Reader<Value, serde_json::Map<String, Value>>;

#[cfg(not(verus_keep_ghost))]
fn reader(value: Value) -> Facts {
    match value {
        Value::Null => Facts::Null,
        Value::Object(object) => Facts::Object(object),
        other => Facts::Other(other),
    }
}

#[cfg(not(verus_keep_ghost))]
fn processes(value: Value) -> Option<Vec<Value>> {
    match value {
        Value::Array(rows) => Some(rows),
        _ => None,
    }
}

#[cfg(not(verus_keep_ghost))]
impl crate::verified_graph::registration_trace::MergeError {
    fn diagnostic(self) -> String {
        use crate::verified_graph::registration_trace::MergeError::*;
        let reason = match self {
            UnboundLocal => "unbound local reader",
            UnboundRegistration => "unbound registration reader",
            MissingLocal => "missing local processes",
            MissingRegistration => "missing registration processes",
            DifferentBinding => "registration and FILEMAP bindings differ",
            InvalidLocal => "invalid local processes",
            InvalidRegistration => "invalid registration processes",
            MissingPrefix => "missing reader prefix",
        };
        format!("E_GIT_FACTS: {reason}")
    }
}

/// Insert the query's processes at the point it ran. Keep a single binding and original process order.
#[cfg(not(verus_keep_ghost))]
pub(crate) fn with_registration_facts(
    local: Value,
    at: usize,
    other: Option<Value>,
) -> Result<Value, String> {
    use crate::verified_graph::registration_trace::{Selection, bind_traces, select_readers};
    match select_readers(reader(local), other.map(reader)).map_err(|e| e.diagnostic())? {
        Selection::Original(Facts::Null) => Ok(Value::Null),
        Selection::Original(Facts::Other(value)) => Ok(value),
        Selection::Original(Facts::Object(map)) => Ok(Value::Object(map)),
        Selection::Bind(mut left, mut right) => {
            let local = left.remove("processes").map(processes);
            let registration = right.remove("processes").map(processes);
            let same_binding = local.is_some() && registration.is_some() && left == right;
            let merged =
                bind_traces(same_binding, local, at, registration).map_err(|e| e.diagnostic())?;
            left.insert("processes".into(), Value::Array(merged));
            Ok(Value::Object(left))
        }
    }
}
