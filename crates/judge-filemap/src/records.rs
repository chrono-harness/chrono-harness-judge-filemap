//! Stable registration record identities and set normalization, owned by FILEMAP impact.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
pub enum FieldDifference { Equal, Members, Leaf }
/// Only differing pairs of objects are descended. Arrays, type changes and additions remain atomic.
pub fn field_difference(equal: bool, both_objects: bool) -> (kind: FieldDifference)
    ensures kind == if equal { FieldDifference::Equal }
        else if both_objects { FieldDifference::Members } else { FieldDifference::Leaf },
{
    if equal { FieldDifference::Equal }
    else if both_objects { FieldDifference::Members }
    else { FieldDifference::Leaf }
}
}
#[cfg(not(verus_keep_ghost))]
use crate::registration::{Endpoint, NodeView};
#[cfg(not(verus_keep_ghost))]
use crate::verified_graph::{PlanAction, PlanOperation, endpoint_role, plan_owner_names};
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct Record {
    pub path: String,
    pub targets: BTreeSet<String>,
    pub value: Value,
}
#[cfg(not(verus_keep_ghost))]
impl crate::verified_graph::RecordTargets for Record {
    fn target_rows(&self) -> Vec<&String> {
        self.targets.iter().collect()
    }
}

/// Keep the input location only while producing findings.
#[cfg(not(verus_keep_ghost))]
pub(crate) struct InputRecord {
    pub record: Record,
    pub pointer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct RecordPair {
    pub base: Option<Record>,
    pub candidate: Option<Record>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct Change {
    pub record: String,
    pub fields: Vec<String>,
}
#[cfg(not(verus_keep_ghost))]
pub fn escape(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
#[cfg(not(verus_keep_ghost))]
fn observed_operation<'a>(value: &'a Value, others: &mut Vec<&'a Value>) -> PlanOperation<'a> {
    if let Some(name) = value.as_str() {
        return PlanOperation::Name(name);
    }
    let index = others
        .iter()
        .position(|item| *item == value)
        .unwrap_or_else(|| {
            others.push(value);
            others.len() - 1
        });
    PlanOperation::Other(index)
}

#[cfg(not(verus_keep_ghost))]
fn plan_targets(
    test: &str,
    plan: Option<&Value>,
    nodes: &BTreeMap<String, NodeView>,
) -> BTreeSet<String> {
    let definitions = nodes
        .get(test)
        .into_iter()
        .flat_map(|node| &node.definitions)
        .map(|definition| definition.identity.as_str())
        .collect();
    let mut others = Vec::new();
    let operations: Vec<_> = plan
        .and_then(|plan| plan["operations"].as_array())
        .into_iter()
        .flatten()
        .map(|value| observed_operation(value, &mut others))
        .collect();
    let mut actions = Vec::new();
    if !operations.is_empty() {
        for (owner, node) in nodes {
            for definition in &node.definitions {
                if let Some(methods) = definition.value["actions"].as_object() {
                    for method in methods.values() {
                        actions.push(PlanAction {
                            owner,
                            role: endpoint_role(node.kind),
                            operation: observed_operation(&method["operation"], &mut others),
                        });
                    }
                }
            }
        }
    }
    plan_owner_names(definitions, &actions, &operations)
        .into_iter()
        .map(str::to_owned)
        .collect()
}
/// Argument vectors, plan sequences and launch priorities preserve semantic order.
#[cfg(not(verus_keep_ghost))]
fn normalize(v: &Value, key: &str) -> Value {
    match v {
        Value::Array(a) => {
            let mut a: Vec<_> = a.iter().map(|v| normalize(v, "")).collect();
            if !crate::verified_graph::ordered_array_field(key) {
                a.sort_by_cached_key(|v| serde_json::to_string(v).unwrap());
            }
            Value::Array(a)
        }
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), normalize(v, k)))
                .collect(),
        ),
        _ => v.clone(),
    }
}
#[cfg(not(verus_keep_ghost))]
pub fn inventory(
    r: &Endpoint,
    nodes: &BTreeMap<String, NodeView>,
    effective_environment: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, InputRecord> {
    let mut out = BTreeMap::new();
    for (scope, value, path) in [
        // The policy record points at the effective target's real path.  The
        // request/registration entry path remains available through the
        // Registrations identity and is never substituted for this source.
        ("config", r.config(), r.effective_config_path()),
        (
            "projects",
            r.projects(),
            r.config()["registries"]["projects"].as_str().unwrap(),
        ),
        (
            "filemap",
            r.filemap(),
            r.config()["registries"]["filemap"].as_str().unwrap(),
        ),
        (
            "judges",
            r.judges(),
            r.config()["registries"]["judges"].as_str().unwrap(),
        ),
        (
            "workflow",
            r.workflow(),
            r.config()["registries"]["workflow"].as_str().unwrap(),
        ),
    ] {
        let converted = value["schema_version"].as_u64() != Some(r.original_schemas()[scope].1);
        let mut add = |collection: &str, key: &str, pointer: String, entry: &Value, targets| {
            out.insert(
                format!("/records/{scope}/{collection}/{}", escape(key)),
                InputRecord {
                    // Converted records may have no field at this location in the original.
                    pointer: if converted {
                        format!("/{scope}")
                    } else {
                        format!("/{scope}{pointer}")
                    },
                    record: Record {
                        path: path.into(),
                        targets,
                        value: normalize(entry, collection),
                    },
                },
            );
        };
        for (key, v) in value.as_object().unwrap() {
            let identity = match (scope, key.as_str()) {
                ("filemap", "files") => Some(("path", "file")),
                ("projects", "projects") => Some(("id", "project")),
                ("projects", "scripts") => Some(("id", "script")),
                ("judges", "judges") => Some(("id", "judge")),
                ("filemap", "test_costs") => Some(("test", "")),
                ("config", "tools") => Some(("id", "tool")),
                ("workflow", "stability") => Some(("id", "")),
                ("config", "artifacts") => Some(("path", "")),
                _ => None,
            };
            if let Some((id, prefix)) = identity {
                for (index, row) in v.as_array().unwrap().iter().enumerate() {
                    let id = row[id].as_str().unwrap();
                    let target = if !prefix.is_empty() {
                        format!("{prefix}:{id}")
                    } else if key == "test_costs" {
                        format!("test:{id}")
                    } else {
                        format!("file:{path}")
                    };
                    let mut targets = BTreeSet::from([target]);
                    // Artifact ownership is an explicit declaration. Changing/removing
                    // it affects that owner at each endpoint, without deriving any
                    // source directory, language or dependency from the output path.
                    if scope == "config" && key == "artifacts" {
                        let owner = row["owner"].as_str().unwrap();
                        for prefix in ["project", "script"] {
                            let node = format!("{prefix}:{owner}");
                            if nodes.contains_key(&node) {
                                targets.insert(node);
                            }
                        }
                        if let Some(policy) = r.filemap().get("execution_scheduling") {
                            for (operation, claims) in policy["claims"].as_object().unwrap() {
                                if claims["outputs"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|p| p == &row["path"])
                                {
                                    for (test, plan) in
                                        r.filemap()["execution_plans"].as_object().unwrap()
                                    {
                                        if plan["operations"]
                                            .as_array()
                                            .unwrap()
                                            .iter()
                                            .any(|o| o == operation)
                                        {
                                            targets.extend(plan_targets(test, Some(plan), nodes));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !prefix.is_empty() {
                        if let Some(tests) = r.active_test_aliases.get(&format!("{prefix}:{id}")) {
                            targets.extend(tests.iter().cloned());
                        }
                    }
                    add(key, id, format!("/{key}/{index}"), row, targets);
                }
            } else if scope == "config" && key == "execution_units" {
                // Ownership and shared-operation changes seed only explicitly
                // named plans at both endpoints.  A test alias alone is not an
                // execution edge, so retain its registered producer identities.
                for (unit, definition) in v["units"].as_object().into_iter().flatten() {
                    let tests: Vec<_> = definition["tests"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    let mut targets = BTreeSet::new();
                    for test in tests {
                        targets.extend(plan_targets(
                            test,
                            r.filemap()["execution_plans"].get(test),
                            nodes,
                        ));
                    }
                    add(
                        "units",
                        unit,
                        format!("/execution_units/units/{}", escape(unit)),
                        definition,
                        targets,
                    );
                }
                for (operation, owners) in v["shared_operations"].as_object().into_iter().flatten()
                {
                    let mut targets = BTreeSet::new();
                    for owner in owners
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                    {
                        for test in v["units"][owner]["tests"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                        {
                            targets.extend(plan_targets(
                                test,
                                r.filemap()["execution_plans"].get(test),
                                nodes,
                            ));
                        }
                    }
                    add(
                        "shared_operations",
                        operation,
                        format!("/execution_units/shared_operations/{}", escape(operation)),
                        owners,
                        targets,
                    );
                }
                for field in ["collection_limits", "report_path"] {
                    add(
                        "execution_units",
                        field,
                        if v.get(field).is_some() {
                            format!("/{key}/{field}")
                        } else {
                            format!("/{key}")
                        },
                        &v[field],
                        BTreeSet::from([format!("file:{path}")]),
                    );
                }
            } else if scope == "filemap" && key == "execution_scheduling" {
                let all_targets = || {
                    r.filemap()["execution_plans"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .flat_map(|(test, plan)| plan_targets(test, Some(plan), nodes))
                        .collect()
                };
                for field in ["max_running", "resources"] {
                    add(
                        key,
                        field,
                        if v.get(field).is_some() {
                            format!("/{key}/{field}")
                        } else {
                            format!("/{key}")
                        },
                        &v[field],
                        all_targets(),
                    );
                }
                if v["priority"]
                    .as_array()
                    .is_some_and(|priority| !priority.is_empty())
                {
                    add(
                        key,
                        "priority",
                        format!("/{key}/priority"),
                        &json!({"priority":v["priority"]}),
                        all_targets(),
                    );
                }
                for (operation, claims) in v["claims"].as_object().unwrap() {
                    let affected = &r.claim_conflicts[operation];
                    let targets = r.filemap()["execution_plans"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .filter(|(_, p)| {
                            p["operations"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .filter_map(Value::as_str)
                                .any(|o| affected.iter().any(|id| id.as_str() == o))
                        })
                        .flat_map(|(test, plan)| plan_targets(test, Some(plan), nodes))
                        .collect();
                    add(
                        "execution_claims",
                        operation,
                        format!("/{key}/claims/{}", escape(operation)),
                        claims,
                        targets,
                    );
                }
            } else if scope == "filemap" && key == "execution_plans" {
                for (test, plan) in v.as_object().unwrap() {
                    let targets = plan_targets(test, Some(plan), nodes);
                    add(key, test, format!("/{key}/{}", escape(test)), plan, targets);
                }
            } else if scope == "filemap" && key == "project_edges" {
                for (index, edge) in v.as_array().unwrap().iter().enumerate() {
                    let id =
                        serde_json::to_string(&json!([edge["from"], edge["kind"], edge["to"]]))
                            .unwrap();
                    add(
                        key,
                        &id,
                        format!("/{key}/{index}"),
                        edge,
                        BTreeSet::from([
                            edge["from"].as_str().unwrap().into(),
                            edge["to"].as_str().unwrap().into(),
                        ]),
                    );
                }
            } else if scope == "filemap" && key == "cost_models" {
                for (id, cost) in v.as_object().unwrap() {
                    let mut targets = BTreeSet::new();
                    for f in r.filemap()["files"].as_array().unwrap() {
                        if f["cost"] == *id {
                            targets.insert(format!("file:{}", f["path"].as_str().unwrap()));
                        }
                    }
                    for t in r.filemap()["test_costs"].as_array().unwrap() {
                        if t["cost"] == *id {
                            targets.insert(format!("test:{}", t["test"].as_str().unwrap()));
                        }
                    }
                    add(key, id, format!("/{key}/{}", escape(id)), cost, targets);
                }
            } else if scope == "projects" && key == "owners" {
                for (index, owner) in v.as_array().unwrap().iter().enumerate() {
                    let id = owner.as_str().unwrap();
                    let targets = r.filemap()["files"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|f| f["owner"] == id)
                        .map(|f| format!("file:{}", f["path"].as_str().unwrap()))
                        .collect();
                    add(key, id, format!("/{key}/{index}"), owner, targets);
                }
            } else if scope == "config" && key == "environment" {
                for (index, input) in v["inputs"].as_array().unwrap().iter().enumerate() {
                    let id = input["id"].as_str().unwrap();
                    add(
                        "inputs",
                        id,
                        format!("/environment/inputs/{index}"),
                        input,
                        BTreeSet::from([format!("input:{id}")]),
                    );
                }
                let keys: BTreeSet<_> = v["inherit"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_str)
                    .chain(v["values"].as_object().unwrap().keys().map(String::as_str))
                    .collect();
                for name in keys {
                    let inherited = v["inherit"].as_array().unwrap().contains(&json!(name));
                    let mut value = json!({"inherit":inherited,"value":v["values"].get(name)});
                    if let Some(effective) = effective_environment {
                        value["effective"] = json!(effective.get(name));
                    }
                    add(
                        "environment",
                        name,
                        format!("/{key}"),
                        &value,
                        BTreeSet::from([format!("environment:{name}")]),
                    );
                }
            } else {
                add(
                    "fields",
                    key,
                    format!("/{}", escape(key)),
                    v,
                    BTreeSet::from([format!("file:{path}")]),
                );
            }
        }
    }
    out
}
#[cfg(not(verus_keep_ghost))]
pub fn changed_fields(a: Option<&Value>, b: Option<&Value>, prefix: &str, out: &mut Vec<String>) {
    let objects = a
        .and_then(Value::as_object)
        .zip(b.and_then(Value::as_object));
    match field_difference(a == b, objects.is_some()) {
        FieldDifference::Equal => {}
        FieldDifference::Leaf => out.push(prefix.into()),
        FieldDifference::Members => {
            let (a, b) = objects.unwrap();
            for k in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                changed_fields(a.get(k), b.get(k), &format!("{prefix}/{}", escape(k)), out);
            }
        }
    }
}
