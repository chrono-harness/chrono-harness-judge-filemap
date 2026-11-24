//! Lower explicit workflow test requirements to typed edges before execution.
use crate::Delta;
use crate::registration::Endpoint;
use crate::verified_graph::workflow_paths;
use crate::{
    graph::{Edge, EdgeKind},
    records::InputRecord,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn select(
    a: &Endpoint,
    b: &Endpoint,
    delta: &[Delta],
    records: [&BTreeMap<String, InputRecord>; 2],
    changes: Value,
) -> Result<Value, String> {
    let changed_paths: BTreeSet<_> = delta.iter().map(|d| d.path.as_str()).collect();
    let rule_paths: Vec<&str> = serde::Deserialize::deserialize(&changes["rule_paths"])
        .map_err(|e| format!("invalid classify-v1 rule paths: {e}"))?;
    if !workflow_paths::admit_rules(&changed_paths, &rule_paths) {
        return Err("classification returned a rule path outside DELTA".into());
    }
    let mut requirements = vec![];
    let mut triggers = BTreeSet::<&str>::new();
    let mut schema_transitions = vec![];
    for (role, (old_path, old_version)) in a.original_schemas() {
        let (new_path, new_version) = &b.original_schemas()[role];
        let paths = workflow_paths::schema_paths(
            old_path.as_str(),
            new_path.as_str(),
            *old_version,
            *new_version,
        );
        if paths.is_empty() {
            continue;
        }
        triggers.extend(paths.iter().copied());
        schema_transitions.push(json!({"registry":role,"before":{"path":old_path,"version":old_version},"after":{"path":new_path,"version":new_version}}));
        for (endpoint, r) in [("base", a), ("candidate", b)] {
            for (i, m) in r.workflow()["migrations"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                if workflow_paths::migration_applies(
                    *old_version,
                    *new_version,
                    m["from_version"].as_u64(),
                    m["to_version"].as_u64(),
                    m.get("test").is_some(),
                ) {
                    requirements.push(requirement(
                        endpoint,
                        r.config()["registries"]["workflow"].as_str().unwrap(),
                        &format!("/migrations/{i}/test"),
                        "schema-migration",
                        &paths,
                        &json!([m["test"]]),
                    ));
                }
            }
        }
    }
    for (endpoint, r) in [("base", a), ("candidate", b)] {
        let path = r.config()["registries"]["workflow"].as_str().unwrap();
        for (i, row) in r.workflow()["stability"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let record = format!(
                "/records/workflow/stability/{}",
                crate::records::escape(row["id"].as_str().unwrap())
            );
            let changed = records[0]
                .get(&record)
                .map(|r| (&r.record.path, &r.record.value))
                != records[1]
                    .get(&record)
                    .map(|r| (&r.record.path, &r.record.value));
            let declared: Vec<&str> = serde::Deserialize::deserialize(&row["paths"]).unwrap();
            let paths = workflow_paths::stability_paths(&changed_paths, &declared, path, changed);
            if paths.is_empty() {
                continue;
            }
            triggers.extend(paths.iter().copied());
            requirements.push(requirement(
                endpoint,
                path,
                &format!("/stability/{i}"),
                "stability",
                &paths,
                &row["tests"],
            ));
        }
    }
    let required = workflow_paths::integration_required(
        &mut triggers,
        a.workflow()["semantic_changes_require_integration"] == true,
        b.workflow()["semantic_changes_require_integration"] == true,
        &rule_paths,
    );
    if required {
        for (endpoint, r) in [("base", a), ("candidate", b)] {
            requirements.push(requirement(
                endpoint,
                r.config()["registries"]["workflow"].as_str().unwrap(),
                "/integration/tests",
                "integration",
                &triggers,
                &r.workflow()["integration"]["tests"],
            ));
        }
    }
    Ok(
        json!({"schema":"chrono-workflow-selection/v1","integration_required":required,
        "rule_changes":changes,"trigger_paths":triggers,"requirements":requirements,"schema_transitions":schema_transitions,
        "certification":"not performed; this output selects declared tests only"}),
    )
}
fn requirement(
    endpoint: &str,
    registry: &str,
    pointer: &str,
    reason: &str,
    paths: &BTreeSet<&str>,
    tests: &Value,
) -> Value {
    let tests: BTreeSet<_> = tests
        .as_array()
        .unwrap()
        .iter()
        .map(|t| format!("test:{}", t.as_str().unwrap()))
        .collect();
    let edges: Vec<_> = paths
        .iter()
        .flat_map(|p| {
            tests.iter().map(move |t| Edge {
                from: format!("file:{p}"),
                kind: EdgeKind::TestExecution,
                to: t.clone(),
            })
        })
        .collect();
    json!({"endpoint":endpoint,"registry":registry,"pointer":pointer,"reason":reason,"trigger_paths":paths,"tests":tests,"edges":edges})
}
pub fn edges(selection: &Value, endpoint: &str) -> BTreeSet<Edge> {
    selection["requirements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["endpoint"] == endpoint)
        .flat_map(|r| r["edges"].as_array().unwrap())
        .map(|e| serde_json::from_value(e.clone()).unwrap())
        .collect()
}
pub fn reference(selection: &Value, edge: &Edge) -> Option<String> {
    selection["requirements"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .find(|(_, r)| {
            r["edges"]
                .as_array()
                .unwrap()
                .contains(&serde_json::to_value(edge).unwrap())
        })
        .map(|(i, _)| format!("/impact/workflow/requirements/{i}"))
}
