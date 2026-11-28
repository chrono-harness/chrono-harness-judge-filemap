//! Stable registration record identities and set normalization, owned by FILEMAP impact.
#[cfg(verus_keep_ghost)]
use super::graph_key::equal_text;
#[cfg(not(verus_keep_ghost))]
use crate::verified_graph::equal_text;
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

#[derive(Clone, Copy)]
pub enum InventoryRule {
    Rows {
        identity: &'static str,
        target_kind: Option<&'static str>,
        test_aliases: bool,
        artifact_owners: bool,
    },
    ExecutionUnits,
    Scheduling,
    Plans,
    ProjectEdges,
    CostModels,
    Owners,
    Environment,
    Field,
}

pub const INVENTORY_RULES: [(&'static str, &'static str, InventoryRule); 15] = [
    ("filemap", "files", InventoryRule::Rows { identity: "path", target_kind: Some("file"), test_aliases: true, artifact_owners: false }),
    ("projects", "projects", InventoryRule::Rows { identity: "id", target_kind: Some("project"), test_aliases: true, artifact_owners: false }),
    ("projects", "scripts", InventoryRule::Rows { identity: "id", target_kind: Some("script"), test_aliases: true, artifact_owners: false }),
    ("judges", "judges", InventoryRule::Rows { identity: "id", target_kind: Some("judge"), test_aliases: true, artifact_owners: false }),
    ("filemap", "test_costs", InventoryRule::Rows { identity: "test", target_kind: Some("test"), test_aliases: false, artifact_owners: false }),
    ("config", "tools", InventoryRule::Rows { identity: "id", target_kind: Some("tool"), test_aliases: true, artifact_owners: false }),
    ("workflow", "stability", InventoryRule::Rows { identity: "id", target_kind: None, test_aliases: false, artifact_owners: false }),
    ("config", "artifacts", InventoryRule::Rows { identity: "path", target_kind: None, test_aliases: false, artifact_owners: true }),
    ("config", "execution_units", InventoryRule::ExecutionUnits),
    ("filemap", "execution_scheduling", InventoryRule::Scheduling),
    ("filemap", "execution_plans", InventoryRule::Plans),
    ("filemap", "project_edges", InventoryRule::ProjectEdges),
    ("filemap", "cost_models", InventoryRule::CostModels),
    ("projects", "owners", InventoryRule::Owners),
    ("config", "environment", InventoryRule::Environment),
];

pub fn inventory_rule(scope: &str, field: &str) -> (rule: InventoryRule)
    ensures
        exists|i: int| 0 <= i < INVENTORY_RULES@.len()
            && (#[trigger] INVENTORY_RULES@[i]).0@ == scope@
            && INVENTORY_RULES@[i].1@ == field@
            && INVENTORY_RULES@[i].2 == rule
            && (forall|before: int| 0 <= before < i
                ==> (#[trigger] INVENTORY_RULES@[before]).0@ != scope@
                    || INVENTORY_RULES@[before].1@ != field@)
        || rule == InventoryRule::Field
            && (forall|i: int| 0 <= i < INVENTORY_RULES@.len()
                ==> (#[trigger] INVENTORY_RULES@[i]).0@ != scope@ || INVENTORY_RULES@[i].1@ != field@),
{
    let rules = INVENTORY_RULES;
    for i in 0..rules.len()
        invariant rules@ == INVENTORY_RULES@,
            forall|j: int| 0 <= j < i
                ==> (#[trigger] rules@[j]).0@ != scope@ || rules@[j].1@ != field@,
    {
        let (registered_scope, registered_field, rule) = rules[i];
        if equal_text(scope, registered_scope) && equal_text(field, registered_field) {
            assert(INVENTORY_RULES@[i as int].2 == rule);
            return rule;
        }
    }
    InventoryRule::Field
}

/// JSON string observations use exact text equality. Absent and non-string fields do not match.
pub fn matches_declaration(observed: Option<&str>, declared: &str) -> (matches: bool)
    ensures matches == (observed matches Some(value) && value@ == declared@),
{
    match observed {
        Some(value) => equal_text(value, declared),
        None => false,
    }
}

/// Every matching row is retained in input order, including repeated targets.
pub fn matching_targets<T: Copy>(rows: &[(T, Option<&str>)], declared: &str)
    -> (targets: Vec<T>)
    ensures targets@ == rows@.filter(|row: (T, Option<&str>)|
        row.1 matches Some(value) && value@ == declared@).map_values(|row: (T, Option<&str>)| row.0),
{
    let mut targets = Vec::new();
    for i in 0..rows.len()
        invariant targets@ == rows@.take(i as int).filter(|row: (T, Option<&str>)|
            row.1 matches Some(value) && value@ == declared@).map_values(|row: (T, Option<&str>)| row.0),
    {
        let row = rows[i];
        if matches_declaration(row.1, declared) {
            targets.push(row.0);
        }
        proof {
            let prefix = rows@.take(i as int);
            let matching = |row: (T, Option<&str>)| row.1 matches Some(value) && value@ == declared@;
            assert(rows@.take(i as int + 1) =~= prefix.push(row));
            prefix.lemma_filter_push(row, matching);
        }
    }
    assert(rows@.take(rows.len() as int) =~= rows@);
    targets
}

pub fn declares_name(names: &[&str], declared: &str) -> (found: bool)
    ensures found == (exists|i: int| 0 <= i < names.len() && (#[trigger] names@[i])@ == declared@),
{
    for i in 0..names.len()
        invariant forall|j: int| 0 <= j < i ==> (#[trigger] names@[j])@ != declared@,
    {
        if equal_text(names[i], declared) {
            assert(names@[i as int]@ == declared@);
            return true;
        }
    }
    false
}

/// A claim affects a plan when an operation in that plan is named by the claim's conflict set.
pub fn plan_has_claim(operations: &[&str], affected: &[&str]) -> (found: bool)
    ensures found == (exists|i: int, j: int| 0 <= i < operations.len() && 0 <= j < affected.len()
        && (#[trigger] operations@[i])@ == (#[trigger] affected@[j])@),
{
    for i in 0..operations.len()
        invariant forall|k: int, j: int| 0 <= k < i && 0 <= j < affected.len()
            ==> (#[trigger] operations@[k])@ != (#[trigger] affected@[j])@,
    {
        if declares_name(affected, operations[i]) {
            assert(exists|j: int| 0 <= j < affected.len() && (#[trigger] affected@[j])@ == operations@[i as int]@);
            return true;
        }
    }
    false
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
fn declaration_rows<'a>(rows: &'a Value, field: &str) -> Vec<(&'a Value, Option<&'a str>)> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| (row, row[field].as_str()))
        .collect()
}

#[cfg(not(verus_keep_ghost))]
fn declared_names(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect()
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
            match inventory_rule(scope, key) {
                InventoryRule::Rows {
                    identity,
                    target_kind,
                    test_aliases,
                    artifact_owners,
                } => {
                    for (index, row) in v.as_array().unwrap().iter().enumerate() {
                        let id = row[identity].as_str().unwrap();
                        let target = match target_kind {
                            Some(kind) => format!("{kind}:{id}"),
                            None => format!("file:{path}"),
                        };
                        let mut targets = BTreeSet::from([target.clone()]);
                        // Artifact ownership is an explicit declaration. Changing/removing
                        // it affects that owner at each endpoint, without deriving any
                        // source directory, language or dependency from the output path.
                        if artifact_owners {
                            let owner = row["owner"].as_str().unwrap();
                            for prefix in ["project", "script"] {
                                let node = format!("{prefix}:{owner}");
                                if nodes.contains_key(&node) {
                                    targets.insert(node);
                                }
                            }
                            if let Some(policy) = r.filemap().get("execution_scheduling") {
                                for (operation, claims) in policy["claims"].as_object().unwrap() {
                                    if declares_name(&declared_names(&claims["outputs"]), id) {
                                        for (test, plan) in
                                            r.filemap()["execution_plans"].as_object().unwrap()
                                        {
                                            if declares_name(
                                                &declared_names(&plan["operations"]),
                                                operation,
                                            ) {
                                                targets.extend(plan_targets(
                                                    test,
                                                    Some(plan),
                                                    nodes,
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if test_aliases {
                            if let Some(tests) = r.active_test_aliases.get(&target) {
                                targets.extend(tests.iter().cloned());
                            }
                        }
                        add(key, id, format!("/{key}/{index}"), row, targets);
                    }
                }
                InventoryRule::ExecutionUnits => {
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
                    for (operation, owners) in
                        v["shared_operations"].as_object().into_iter().flatten()
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
                }
                InventoryRule::Scheduling => {
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
                        let affected: Vec<_> = r.claim_conflicts[operation]
                            .iter()
                            .map(String::as_str)
                            .collect();
                        let targets = r.filemap()["execution_plans"]
                            .as_object()
                            .unwrap()
                            .iter()
                            .filter(|(_, p)| {
                                plan_has_claim(&declared_names(&p["operations"]), &affected)
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
                }
                InventoryRule::Plans => {
                    for (test, plan) in v.as_object().unwrap() {
                        let targets = plan_targets(test, Some(plan), nodes);
                        add(key, test, format!("/{key}/{}", escape(test)), plan, targets);
                    }
                }
                InventoryRule::ProjectEdges => {
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
                }
                InventoryRule::CostModels => {
                    for (id, cost) in v.as_object().unwrap() {
                        let files = declaration_rows(&r.filemap()["files"], "cost");
                        let mut targets: BTreeSet<_> = matching_targets(&files, id)
                            .into_iter()
                            .map(|f| format!("file:{}", f["path"].as_str().unwrap()))
                            .collect();
                        let tests = declaration_rows(&r.filemap()["test_costs"], "cost");
                        targets.extend(
                            matching_targets(&tests, id)
                                .into_iter()
                                .map(|t| format!("test:{}", t["test"].as_str().unwrap())),
                        );
                        add(key, id, format!("/{key}/{}", escape(id)), cost, targets);
                    }
                }
                InventoryRule::Owners => {
                    for (index, owner) in v.as_array().unwrap().iter().enumerate() {
                        let id = owner.as_str().unwrap();
                        let files = declaration_rows(&r.filemap()["files"], "owner");
                        let targets = matching_targets(&files, id)
                            .into_iter()
                            .map(|f| format!("file:{}", f["path"].as_str().unwrap()))
                            .collect();
                        add(key, id, format!("/{key}/{index}"), owner, targets);
                    }
                }
                InventoryRule::Environment => {
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
                    let inherited_names = declared_names(&v["inherit"]);
                    let names: Vec<_> = inherited_names
                        .iter()
                        .copied()
                        .chain(v["values"].as_object().unwrap().keys().map(String::as_str))
                        .collect();
                    for name in crate::verified_graph::name_inventory(&names) {
                        let inherited = declares_name(&inherited_names, name);
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
                }
                InventoryRule::Field => {
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
