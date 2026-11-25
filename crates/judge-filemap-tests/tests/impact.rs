#[path = "support/classification.rs"]
mod mixed_fixture;
fn classification(delta: &[chrono_judge_filemap::Delta]) -> Result<serde_json::Value, String> {
    mixed_fixture::reply(
        &delta
            .iter()
            .map(|change| change.path.as_str())
            .collect::<Vec<_>>(),
    )
}
mod support;
use chrono_judge_filemap::Finding;
use chrono_judge_filemap::{
    Impact,
    graph::{self, Edge, EdgeKind, Origin, Seed},
    produce,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
use support::*;
fn run(base: &str, candidate: &str, d: &[chrono_judge_filemap::Delta]) -> (Impact, Vec<Finding>) {
    produce(&views(base, candidate), d, classification(d))
}

fn tests(i: &Impact) -> Vec<&str> {
    i.required_tests.iter().map(|t| t.node.as_str()).collect()
}
#[test]
fn configuration_edges_preserve_two_endpoint_add_remove_impact_and_inactive_scope() {
    for (before, after, origin) in [
        ("profile", "active_profile", Origin::Candidate),
        ("active_profile", "profile", Origin::Base),
    ] {
        let (impact, findings) = run(before, after, &[]);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(tests(&impact), ["test:t"]);
        assert!(
            impact
                .edges
                .iter()
                .any(|e| e.edge.from == "environment:PATH" && e.origin == origin)
        );
        assert!(!impact.nodes.contains_key("input:foreign"));
        assert!(!impact.edges.iter().any(|e| e.edge.from == "input:foreign"));
    }
    let (impact, findings) = run("active_profile", "inactive_edge", &[]);
    assert!(findings.is_empty(), "{findings:?}");
    assert!(
        tests(&impact).is_empty(),
        "inactive policy graph must not select business tests"
    );
}
#[test]
fn operation_override_add_change_and_remove_keep_explicit_plan_impact() {
    let (disconnected, findings) = run("disconnected_plan", "disconnected_bounds", &[]);
    assert!(findings.is_empty(), "{findings:?}");
    assert!(tests(&disconnected).is_empty());
    for (before, after) in [
        ("plan", "bounds"),
        ("bounds", "changed_bounds"),
        ("bounds", "plan"),
    ] {
        let (impact, findings) = run(before, after, &[]);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(tests(&impact), ["test:t"]);
        assert!(
            impact
                .changes
                .iter()
                .any(|c| c.record.ends_with("/execution_plans/test:t")
                    && c.fields.iter().any(|f| f.starts_with("/operation_bounds")))
        );
    }
}
fn oracle_edges(rows: &[(&str, EdgeKind, &str)]) -> BTreeSet<Edge> {
    rows.iter()
        .map(|(a, k, b)| Edge {
            from: (*a).into(),
            kind: *k,
            to: (*b).into(),
        })
        .collect()
}
// Witness oracle takes independently specified endpoint edges; it never parses producer strings.
fn witnesses(i: &Impact, allowed: &BTreeSet<Edge>) {
    for (node, seeds) in &i.closure.reached {
        for id in seeds.entries() {
            let s = i.seed_causes.iter().find(|s| &s.id == id).unwrap();
            let mut at = s.node.clone();
            for e in i.closure.witness(id, node).unwrap() {
                assert!(allowed.contains(&e), "fabricated edge {e:?}");
                assert_eq!(at, e.from);
                at = e.to;
            }
            assert_eq!(&at, node);
        }
    }
    for t in &i.closure.traversed {
        assert!(allowed.contains(&t.edge));
    }
}
#[test]
fn typed_content_mode_binary_add_modify_delete_and_rename() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_endpoints(root);
    fs::write(root.join("src.bin"), [0, 255, 1]).unwrap();
    let b = commit(root);
    for (bytes, mode) in [(vec![0, 255, 2], 0o644), (vec![0, 255, 1], 0o755)] {
        fs::write(root.join("src.bin"), bytes).unwrap();
        fs::set_permissions(root.join("src.bin"), fs::Permissions::from_mode(mode)).unwrap();
        let c = commit(root);
        let d = observed_delta(root, &b, &c).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].kind, "M");
        let (i, f) = run("standard", "standard", &d);
        assert!(f.is_empty());
        assert_eq!(tests(&i), ["test:t"]);
    }
    fs::rename(root.join("src.bin"), root.join("new.bin")).unwrap();
    let c = commit(root);
    let d = observed_delta(root, &b, &c).unwrap();
    assert_eq!(
        d.iter().map(|d| (&*d.path, &*d.kind)).collect::<Vec<_>>(),
        [("new.bin", "A"), ("src.bin", "D")]
    );
    assert_eq!(d[0].new_blob, d[1].old_blob);
    let (i, f) = run("standard", "renamed_file", &d);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    assert!(i.nodes["file:src.bin"].candidate_definitions.is_empty());
}
#[test]
fn nonancestor_and_multicommit_facts_use_complete_endpoints() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_endpoints(root);
    fs::write(root.join("src.bin"), "one").unwrap();
    let first = commit(root);
    fs::write(root.join("doc.txt"), "docs").unwrap();
    let left = commit(root);
    git(root, &["checkout", "--detach", &first]);
    fs::write(root.join("src.bin"), "two").unwrap();
    commit(root);
    fs::write(root.join("doc.txt"), "other docs").unwrap();
    let right = commit(root);
    for base in [&first, &left] {
        let d = observed_delta(root, base, &right).unwrap();
        assert_eq!(d.len(), 2);
        let (i, _) = run("standard", "standard", &d);
        assert_eq!(tests(&i), ["test:t"]);
    }
    assert!(observed_delta(root, &first, &"a".repeat(40)).is_err());
}
#[test]
fn no_delta_and_disconnected_docs_only_select_every_delta_judges() {
    for d in [vec![], vec![delta("doc.txt")]] {
        let (i, f) = run("standard", "standard", &d);
        assert!(f.is_empty());
        assert!(tests(&i).is_empty());
        assert_eq!(i.judges.len(), 2);
        assert_eq!(
            i.judges.iter().map(|j| j.node.as_str()).collect::<Vec<_>>(),
            ["judge:filemap", "judge:registration"]
        );
        assert!(i.judges.iter().all(|j| j.triggers.is_empty()));
    }
}
#[test]
fn base_only_test_edge_preserves_removed_requirement_owner_and_cost() {
    let (i, f) = run("standard", "removed_test_and_cost", &[delta(FM)]);
    assert!(f.is_empty());
    assert_eq!(
        tests(&i),
        ["test:t"],
        "removed test edge must still require old test"
    );
    assert!(!i.required_tests[0].candidate_present);
    let cost = &i.records["/records/filemap/test_costs/t"];
    assert_eq!(cost.base.as_ref().unwrap().value["cost"], "unknown");
    assert!(cost.candidate.is_none());
    let model = &i.records["/records/filemap/cost_models/unknown"];
    assert_eq!(model.base.as_ref().unwrap().value["cpu_ms"], json!(null));
    assert!(model.candidate.is_none());
    let old_file = &i.nodes["file:src.bin"].base_definitions;
    assert_eq!(old_file.len(), 1);
    assert_eq!(old_file[0].value["owner"], "host");
    assert!(i.records["/records/projects/owners/host"].base.is_some());
    assert!(i.edges.iter().all(|e| e.origin == Origin::Base));
    witnesses(
        &i,
        &oracle_edges(&[
            ("file:src.bin", EdgeKind::Compile, "project:p"),
            ("project:p", EdgeKind::TestExecution, "test:t"),
        ]),
    );
}
#[test]
fn file_record_field_and_edge_add_delete_retarget_changes_seed_both_ends() {
    let (i, f) = run("standard", "retarget", &[delta(FM)]);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    assert!(i.changes.iter().any(
        |c| c.record == "/records/filemap/files/src.bin" && c.fields.contains(&"/owner".into())
    ));
    for node in ["project:p", "project:t", "test:t"] {
        assert!(
            i.seed_causes
                .iter()
                .any(|s| s.node == node && s.reason == "edge-change")
        );
    }
    assert_eq!(
        i.edges.iter().filter(|e| e.origin == Origin::Base).count(),
        1
    );
    assert_eq!(
        i.edges
            .iter()
            .filter(|e| e.origin == Origin::Candidate)
            .count(),
        1
    );
    assert_eq!(
        i.edges.iter().filter(|e| e.origin == Origin::Both).count(),
        1
    );
    for s in &i.seed_causes {
        if s.reason != "delta-path" {
            assert!(i.records.contains_key(&s.reference));
        }
    }
}
#[test]
fn equal_record_in_moved_registry_seeds_each_endpoint_target() {
    let mut endpoints = views("standard", "standard");
    let moved = ".chrono-harness/other-projects.json";
    endpoints.candidate.config["registries"]["projects"] = json!(moved);
    let (impact, findings) = produce(&endpoints, &[], classification(&[]));
    assert!(findings.is_empty(), "{findings:?}");
    let id = "/records/projects/projects/t";
    let pair = &impact.records[id];
    let before = pair.base.as_ref().unwrap();
    let after = pair.candidate.as_ref().unwrap();
    assert_eq!(before.value, after.value);
    assert_eq!((before.path.as_str(), after.path.as_str()), (PROJECTS, moved));
    assert_eq!(impact.changes.iter().find(|c| c.record == id).unwrap().fields, ["/$registry_path"]);
    let targets: BTreeSet<_> = impact.seed_causes.iter()
        .filter(|s| s.reference == id && s.reason == "record-change")
        .map(|s| s.node.clone()).collect();
    assert_eq!(targets, before.targets.union(&after.targets).cloned().collect());
}

#[test]
fn classifier_findings_keep_the_first_code_and_declared_file_obligations() {
    let endpoints = views("standard", "standard");
    for (message, code) in [
        ("E_SURFACE_REFERENCE: file:missing", "E_SURFACE_REFERENCE"),
        ("missing: E_SECOND", "E_WORKFLOW_INPUT"),
        ("E_NO_DETAIL", "E_NO_DETAIL"),
        ("", "E_WORKFLOW_INPUT"),
    ] {
        let (impact, findings) = produce(&endpoints, &[delta("src.bin")], Err(message.into()));
        assert!(impact.workflow.is_null());
        assert_eq!(tests(&impact), ["test:t"]);
        let [finding] = findings.as_slice() else { panic!("{findings:?}") };
        assert_eq!(finding.code, code);
        assert_eq!(finding.level, "error");
        assert_eq!(finding.message, message);
        assert_eq!(finding.delta_refs, ["/request"]);
        assert!(finding.causes.is_empty());
    }
}

#[test]
fn emitted_seed_ids_decode_to_the_exact_reference_reason_and_node() {
    let source = "dir/引号\"与反斜杠\\以及换行\n[]";
    let (impact, findings) = produce(
        &views("cycle", "cycle"),
        &[delta(source), delta("src.bin"), delta(source)],
        Err("E_WORKFLOW_INPUT: classification unavailable".into()),
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.code == "E_WORKFLOW_INPUT")
    );
    assert_eq!(impact.seed_causes.len(), 2);
    for seed in &impact.seed_causes {
        let fields: [String; 3] = serde_json::from_str(&seed.id).unwrap();
        assert_eq!(
            fields,
            [
                seed.reference.clone(),
                seed.reason.clone(),
                seed.node.clone()
            ]
        );
        assert_eq!(seed.node, format!("file:{}", seed.reference));
        assert_eq!(seed.reason, "delta-path");
    }
    assert!(
        impact
            .seed_causes
            .iter()
            .any(|seed| seed.reference == source)
    );
}

#[test]
fn cycles_diamonds_and_multiple_causes_retain_real_edges_and_all_seeds() {
    let rows = [
        ("file:src.bin", EdgeKind::Compile, "project:p"),
        ("file:doc.txt", EdgeKind::RuntimeInput, "project:a"),
        ("project:p", EdgeKind::Compile, "project:a"),
        ("project:p", EdgeKind::Compile, "project:b"),
        ("project:a", EdgeKind::Compile, "project:z"),
        ("project:b", EdgeKind::Compile, "project:z"),
        ("project:z", EdgeKind::Compile, "project:p"),
        ("project:z", EdgeKind::TestExecution, "test:t"),
    ];
    let (i, f) = run("cycle", "cycle", &[delta("src.bin"), delta("doc.txt")]);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    assert_eq!(i.closure.reached["test:t"].len(), 2);
    assert_eq!(i.closure.traversed.len(), rows.len());
    witnesses(&i, &oracle_edges(&rows));
}

#[test]
fn input_declaration_boundary_propagates_build_and_runtime_without_claiming_retention() {
    for (before, after) in [
        ("build_input", "build_digest"),
        ("runtime_input", "runtime_digest"),
    ] {
        let (i, f) = run(before, after, &[delta(CONFIG)]);
        assert!(f.is_empty());
        assert_eq!(tests(&i), ["test:t"]);
        assert!(i.seed_causes.iter().any(|s| s.node == "input:data"));
        assert!(i.limits.iter().any(|s| s.contains("not retained")));
    }
}
#[test]
fn pairing_and_test_record_seed_without_execution_edge_do_not_select() {
    let (i, _) = run(
        "no_execution_edge",
        "changed_cost_no_edge",
        &[delta("src.bin"), delta(FM)],
    );
    assert!(i.closure.reached.contains_key("project:p"));
    assert!(i.closure.reached.contains_key("test:t"));
    assert!(
        tests(&i).is_empty(),
        "test seed and ownership pairing cannot select execution"
    );
}
#[test]
fn wrong_edge_to_test_cannot_select_and_has_real_cause() {
    let (i, f) = run("wrong_type", "wrong_type", &[delta("src.bin")]);
    assert!(tests(&i).is_empty());
    assert_eq!(f.len(), 2);
    for f in f {
        assert_eq!(f.code, "E_EDGE_TYPE");
        assert_eq!(f.delta_refs, ["src.bin"]);
        assert_eq!(f.causes, ["file:src.bin", "project:p", "test:t"]);
    }
}
#[test]
fn reached_dangling_and_changed_invalid_types_fail_but_disconnected_history_is_context() {
    let (_, f) = run("dangling", "dangling", &[delta("src.bin")]);
    assert_eq!(f.len(), 2);
    for f in f {
        assert_eq!(f.delta_refs, ["src.bin"]);
        assert_eq!(f.causes, ["file:src.bin", "project:p", "project:missing"]);
    }
    let (i, f) = run("dangling", "dangling", &[delta("doc.txt")]);
    assert!(f.is_empty());
    assert_eq!(i.historical_context.len(), 2);
    let (i, f) = run("dangling", "wrong_source", &[delta(FM)]);
    assert!(f.iter().any(|f| f.code == "E_EDGE_TYPE"));
    for f in f {
        assert_eq!(f.delta_refs, [FM]);
        assert!(i.seed_causes.iter().any(|seed| seed.node == f.causes[0]));
    }
}
#[test]
fn judge_triggers_add_explanations_once() {
    let (i, f) = run("trigger", "trigger", &[delta("src.bin")]);
    assert!(f.is_empty());
    let j = i.judges.iter().find(|j| j.node == "judge:filemap").unwrap();
    assert!(i.judges.iter().any(|j| j.node == "judge:registration"));
    assert_eq!(j.triggers.len(), 1);
    assert_eq!(i.judges.len(), 2);
}
#[test]
fn identity_array_order_is_irrelevant_but_argv_order_changes_target() {
    let (i, f) = run(
        "standard",
        "reordered",
        &[delta(FM), delta(PROJECTS), delta(JUDGES)],
    );
    assert!(f.is_empty());
    assert!(i.changes.is_empty());
    assert!(tests(&i).is_empty());
    let (i, _) = run("standard", "reordered_argv", &[delta(PROJECTS)]);
    assert_eq!(tests(&i), ["test:t"]);
    assert!(
        i.changes
            .iter()
            .any(|c| c.fields.contains(&"/actions/execute/argv".into()))
    );
}
#[test]
fn shared_graph_does_not_need_full_registration_and_only_execution_edges_select() {
    let edge = Edge {
        from: "legacy".into(),
        kind: EdgeKind::Compile,
        to: "test:legacy".into(),
    };
    let g = graph::union(&BTreeSet::new(), &BTreeSet::from([edge]));
    let c = graph::closure(
        &g,
        &[Seed {
            id: "s".into(),
            node: "legacy".into(),
            reference: "legacy".into(),
            reason: "adapter".into(),
        }],
    )
    .unwrap();
    assert!(c.reached.contains_key("test:legacy"));
    assert!(
        c.selected_tests().is_empty(),
        "nonexecution edges cannot select tests"
    );
}

#[test]
fn edge_kind_json_adapters_use_exact_canonical_strings() {
    let pairs = [
        ("compile", EdgeKind::Compile),
        ("build-input", EdgeKind::BuildInput),
        ("runtime-input", EdgeKind::RuntimeInput),
        ("test-execution", EdgeKind::TestExecution),
        ("judge-trigger", EdgeKind::JudgeTrigger),
    ];
    for (name, kind) in pairs {
        let value = json!(name);
        assert_eq!(serde_json::to_value(kind).unwrap(), value);
        assert_eq!(serde_json::from_value::<EdgeKind>(value).unwrap(), kind);
        let text = format!("\"{name}\"");
        assert_eq!(serde_json::from_str::<EdgeKind>(&text).unwrap(), kind);
    }
    assert_eq!(
        serde_json::from_str::<EdgeKind>(r#""\u0063ompile""#).unwrap(),
        EdgeKind::Compile
    );
    for value in [
        json!(""),
        json!("Compile"),
        json!("compil"),
        json!("compilex"),
        json!("build_input"),
        json!(" compile"),
        json!("compile\0"),
        json!("编译"),
        json!(null),
        json!(0),
        json!(true),
        json!([]),
        json!({"compile": null}),
    ] {
        assert!(
            serde_json::from_value::<EdgeKind>(value.clone()).is_err(),
            "{value}"
        );
        assert!(
            serde_json::from_str::<EdgeKind>(&value.to_string()).is_err(),
            "{value}"
        );
    }
}

#[test]
fn complete_edge_order_and_inventory_match_the_declared_field_order() {
    let expected: Vec<Edge> = [
        ("", EdgeKind::JudgeTrigger, "根"),
        ("a", EdgeKind::Compile, ""),
        ("a", EdgeKind::Compile, "a"),
        ("a", EdgeKind::Compile, "a\0"),
        ("a", EdgeKind::Compile, "aa"),
        ("a", EdgeKind::Compile, "é"),
        ("a", EdgeKind::Compile, "根"),
        ("a", EdgeKind::BuildInput, ""),
        ("a", EdgeKind::RuntimeInput, ""),
        ("a", EdgeKind::TestExecution, ""),
        ("a", EdgeKind::JudgeTrigger, ""),
        ("aa", EdgeKind::Compile, ""),
        ("é", EdgeKind::Compile, ""),
        ("根", EdgeKind::Compile, ""),
    ]
    .into_iter()
    .map(|(from, kind, to)| Edge {
        from: from.into(),
        kind,
        to: to.into(),
    })
    .collect();
    for (i, a) in expected.iter().enumerate() {
        for (j, b) in expected.iter().enumerate() {
            assert_eq!(a.cmp(b), i.cmp(&j), "indices {i}, {j}");
            assert_eq!(a.partial_cmp(b), Some(i.cmp(&j)), "indices {i}, {j}");
            assert_eq!(a == b, i == j, "indices {i}, {j}");
        }
        assert_eq!(a.clone(), *a);
    }
    let row: BTreeSet<&Edge> = expected.iter().rev().chain(expected.iter()).collect();
    let actual = chrono_judge_filemap::verified_graph::edge_inventory(&row);
    assert_eq!(actual, expected.iter().collect::<Vec<_>>());
}

#[test]
fn edge_json_adapters_preserve_fields_and_report_invalid_inputs() {
    let edge = Edge {
        from: "根\0".into(),
        kind: EdgeKind::BuildInput,
        to: "é".into(),
    };
    let expected = json!({"from": "根\0", "kind": "build-input", "to": "é"});
    assert_eq!(serde_json::to_value(&edge).unwrap(), expected);
    assert_eq!(serde_json::from_value::<Edge>(expected).unwrap(), edge);
    let text = r#"{"to":"\u00e9","kind":"build-input","from":"根\u0000"}"#;
    assert_eq!(serde_json::from_str::<Edge>(text).unwrap(), edge);
    assert_eq!(
        format!("{edge:?}"),
        "Edge { from: \"根\\0\", kind: BuildInput, to: \"é\" }"
    );
    for value in [
        json!({"from": "a", "kind": "compile"}),
        json!({"kind": "compile", "to": "b"}),
        json!({"from": "a", "to": "b"}),
        json!({"from": 1, "kind": "compile", "to": "b"}),
        json!({"from": "a", "kind": "Compile", "to": "b"}),
        json!({"from": "a", "kind": "compile", "to": null}),
    ] {
        assert!(serde_json::from_value::<Edge>(value.clone()).is_err());
        assert!(serde_json::from_str::<Edge>(&value.to_string()).is_err());
    }
}

#[test]
fn complete_seed_order_matches_all_four_fields_and_preserves_btree_members() {
    let expected: Vec<Seed> = [
        ("", "z", "z", "z"),
        ("a", "", "z", "z"),
        ("a", "a", "", "z"),
        ("a", "a", "a", ""),
        ("a", "a", "a", "\0"),
        ("a", "a", "a", "a"),
        ("a", "a", "a", "a\0"),
        ("a", "a", "a", "aa"),
        ("a", "a", "a", "é"),
        ("a", "a", "a", "根"),
        ("a", "a", "b", ""),
        ("a", "b", "", ""),
        ("b", "", "", ""),
        ("根", "", "", ""),
    ]
    .into_iter()
    .map(|(id, node, reference, reason)| Seed {
        id: id.into(),
        node: node.into(),
        reference: reference.into(),
        reason: reason.into(),
    })
    .collect();
    for (i, a) in expected.iter().enumerate() {
        for (j, b) in expected.iter().enumerate() {
            assert_eq!(a.cmp(b), i.cmp(&j), "indices {i}, {j}");
            assert_eq!(a.partial_cmp(b), Some(i.cmp(&j)), "indices {i}, {j}");
            assert_eq!(a == b, i == j, "indices {i}, {j}");
        }
        assert_eq!(a.clone(), *a);
    }
    let actual: BTreeSet<Seed> = expected
        .iter()
        .rev()
        .chain(expected.iter())
        .cloned()
        .collect();
    assert_eq!(actual.into_iter().collect::<Vec<_>>(), expected);
}

#[test]
fn seed_json_adapters_preserve_each_field_and_reject_missing_or_nonstring_fields() {
    let seed = Seed {
        id: "id\0".into(),
        node: "根".into(),
        reference: "é".into(),
        reason: "".into(),
    };
    let expected = json!({"id":"id\0","node":"根","reference":"é","reason":""});
    assert_eq!(serde_json::to_value(&seed).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<Seed>(expected.clone()).unwrap(),
        seed
    );
    let text = r#"{"reason":"","reference":"\u00e9","node":"根","id":"id\u0000"}"#;
    assert_eq!(serde_json::from_str::<Seed>(text).unwrap(), seed);
    assert_eq!(
        format!("{seed:?}"),
        "Seed { id: \"id\\0\", node: \"根\", reference: \"é\", reason: \"\" }"
    );
    for field in ["id", "node", "reference", "reason"] {
        let mut missing = expected.clone();
        missing.as_object_mut().unwrap().remove(field);
        let mut nonstring = expected.clone();
        nonstring[field] = json!(null);
        for value in [missing, nonstring] {
            assert!(serde_json::from_value::<Seed>(value.clone()).is_err());
            assert!(serde_json::from_str::<Seed>(&value.to_string()).is_err());
        }
    }
}

#[test]
fn typed_union_preserves_full_order_exact_origins_and_flat_json_transport() {
    let expected = [
        ("", EdgeKind::JudgeTrigger, "根", Origin::Candidate),
        ("a", EdgeKind::Compile, "", Origin::Base),
        ("a", EdgeKind::Compile, "a\0", Origin::Both),
        ("a", EdgeKind::Compile, "aa", Origin::Candidate),
        ("a", EdgeKind::BuildInput, "", Origin::Base),
        ("a", EdgeKind::JudgeTrigger, "根", Origin::Both),
        ("é", EdgeKind::Compile, "", Origin::Candidate),
        ("根", EdgeKind::Compile, "", Origin::Base),
    ];
    let mut base = BTreeSet::new();
    let mut candidate = BTreeSet::new();
    for (from, kind, to, origin) in expected.into_iter().rev() {
        let edge = Edge {
            from: from.into(),
            kind,
            to: to.into(),
        };
        if origin != Origin::Candidate {
            base.insert(edge.clone());
            base.insert(edge.clone());
        }
        if origin != Origin::Base {
            candidate.insert(edge.clone());
            candidate.insert(edge);
        }
    }
    let out = graph::union(&base, &candidate);
    assert_eq!(out.len(), expected.len());
    for (entry, (from, kind, to, origin)) in out.iter().zip(expected) {
        assert_eq!(
            (
                &*entry.edge.from,
                entry.edge.kind,
                &*entry.edge.to,
                entry.origin
            ),
            (from, kind, to, origin)
        );
        let wire_origin = match origin {
            Origin::Base => "base",
            Origin::Candidate => "candidate",
            Origin::Both => "both",
        };
        let value = json!({"from": from, "kind": kind, "to": to, "origin": wire_origin});
        assert_eq!(serde_json::to_value(entry).unwrap(), value);
        for decoded in [
            serde_json::from_value::<graph::UnionEdge>(value.clone()).unwrap(),
            serde_json::from_str::<graph::UnionEdge>(&value.to_string()).unwrap(),
            entry.clone(),
        ] {
            assert_eq!(decoded.edge, entry.edge);
            assert_eq!(decoded.origin, origin);
        }
        assert_eq!(
            format!("{entry:?}"),
            format!("UnionEdge {{ edge: {:?}, origin: {origin:?} }}", entry.edge)
        );
    }
    for value in [
        json!({"from":"a","kind":"compile","to":"b"}),
        json!({"from":"a","kind":"compile","to":"b","origin":"Both"}),
        json!({"from":"a","kind":"compile","to":"b","origin":null}),
        json!({"from":"a","kind":"compile","origin":"both"}),
    ] {
        assert!(serde_json::from_value::<graph::UnionEdge>(value.clone()).is_err());
        assert!(serde_json::from_str::<graph::UnionEdge>(&value.to_string()).is_err());
    }
}

#[test]
fn indexed_closure_preserves_shortest_ties_edge_kinds_and_seed_identity() {
    let rows = [
        ("根", EdgeKind::RuntimeInput, "a"),
        ("根", EdgeKind::Compile, "z"),
        ("根", EdgeKind::Compile, "a"),
        ("a", EdgeKind::Compile, "end"),
        ("z", EdgeKind::Compile, "end"),
        ("end", EdgeKind::TestExecution, "test:t"),
        ("test:t", EdgeKind::RuntimeInput, "根"),
    ];
    let seed = |id: &str, node: &str| Seed {
        id: id.into(),
        node: node.into(),
        reference: "explicit".into(),
        reason: "boundary".into(),
    };
    let seeds = [seed("s", "根"), seed("other", "z"), seed("isolated", "孤")];
    let allowed = oracle_edges(&rows);
    let mut edges = graph::union(&BTreeSet::new(), &allowed);
    // Input order and repeated identical edges cannot change adjacency ordering.
    edges.reverse();
    edges.push(edges[0].clone());
    let closure = graph::closure(&edges, &seeds).unwrap();
    let expected = vec![
        Edge {
            from: "根".into(),
            kind: EdgeKind::Compile,
            to: "a".into(),
        },
        Edge {
            from: "a".into(),
            kind: EdgeKind::Compile,
            to: "end".into(),
        },
    ];
    assert_eq!(closure.witness("s", "end"), Some(expected));
    assert_eq!(closure.witness("other", "end").unwrap().len(), 1);
    assert_eq!(closure.witness("isolated", "孤"), Some(vec![]));
    assert_eq!(closure.witness("isolated", "end"), None);
    assert_eq!(closure.selected_tests(), BTreeSet::from(["test:t".into()]));
    assert_eq!(closure.traversed.len(), allowed.len());
    assert!(closure.traversed.iter().all(|edge| {
        allowed.contains(&edge.edge)
            && edge
                .seeds
                .entries()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                == ["other", "s"]
    }));
    let duplicate = graph::closure(&edges, &[seed("same", "根"), seed("same", "孤")]);
    assert_eq!(
        duplicate.unwrap_err(),
        "E_SEED_ID_CONFLICT: seed ID \"same\" refers to both \"根\" and \"孤\""
    );
}

#[test]
fn public_closure_rejects_one_id_for_different_roots_and_accepts_a_repeated_root() {
    let edge = json!({"from": "A", "kind": "test-execution", "to": "test:T"});
    let seed = |node, reference| json!({"id":"same", "node":node, "reference": reference, "reason":"explicit"});
    for reachable_only in [false, true] {
        let mut request = json!({"base":[],"candidate":[edge],"seeds":[seed("A", "one"), seed("B", "two")],"reachable_only":reachable_only});
        let query =
            |request: &serde_json::Value| graph::query_closure(request.to_string().as_bytes());
        let error = query(&request).unwrap_err();
        assert_eq!(
            error,
            "E_SEED_ID_CONFLICT: seed ID \"same\" refers to both \"A\" and \"B\""
        );
        request["seeds"][1] = seed("A", "two");
        let result = query(&request).unwrap();
        assert_eq!(result["selected_tests"], json!(["test:T"]));
        if !reachable_only {
            assert_eq!(
                result["closure"]["predecessors"]["same"],
                json!({"A": null, "test:T": edge})
            );
            assert_eq!(result["closure"]["reached"]["test:T"], json!(["same"]));
        }
    }
}

#[test]
fn public_predecessor_table_keeps_exact_distinct_ids_and_deserializes_for_path_consumers() {
    let edge = json!({"from":"A","kind":"test-execution","to":"test:T"});
    let seed = |id: &str, node: &str, reference: &str| json!({"id":id,"node":node,"reference":reference,"reason":"explicit"});
    let request = json!({"base":[edge], "candidate":[edge], "reachable_only":false,
        "seeds":[seed("é","A","one"),seed("e\u{301}","B","two"),seed("","A","three"),
            seed("é\0","C","four"),seed("é","A","again")]});
    let value = graph::query_closure(request.to_string().as_bytes()).unwrap();
    let tree = json!({"A":null,"test:T":edge});
    assert_eq!(
        value["closure"]["predecessors"],
        json!({"":tree,"é":tree,"e\u{301}":{"B":null},"é\0":{"C":null}})
    );
    let closure: graph::Closure = serde_json::from_value(value["closure"].clone()).unwrap();
    let selected = vec![Edge {
        from: "A".into(),
        kind: EdgeKind::TestExecution,
        to: "test:T".into(),
    }];
    assert_eq!(closure.witness("é", "test:T"), Some(selected.clone()));
    assert_eq!(closure.witness("", "test:T"), Some(selected));
    assert_eq!(closure.witness("e\u{301}", "test:T"), None);
    assert_eq!(closure.witness("é\0", "C"), Some(vec![]));
    assert_eq!(closure.witness("missing", "C"), None);
}

#[test]
fn cause_state_merges_exact_ids_clones_independently_and_keeps_json_arrays() {
    use graph::CauseSet;
    let mut causes = CauseSet::default();
    assert_eq!(causes.len(), 0);
    assert_eq!(causes.first(), None);
    for name in ["z", "根\0", "a\0", "é", "", "a", "e\u{301}"] {
        assert!(causes.insert(name));
        assert!(causes.contains(name));
        let before = serde_json::to_value(&causes).unwrap();
        assert!(!causes.insert(&String::from(name)));
        assert_eq!(serde_json::to_value(&causes).unwrap(), before);
    }
    let expected = json!(["", "a", "a\0", "e\u{301}", "z", "é", "根\0"]);
    assert_eq!(causes.len(), 7);
    assert_eq!(causes.first().map(String::as_str), Some(""));
    assert_eq!(serde_json::to_value(&causes).unwrap(), expected);
    for missing in ["a\0x", "根", "É", "missing"] {
        assert!(!causes.contains(missing));
    }
    let mut copy = causes.clone();
    assert!(copy.insert("new"));
    assert!(!causes.contains("new"));
    assert_eq!(serde_json::to_value(&causes).unwrap(), expected);
    let decoded: CauseSet = serde_json::from_value(json!([
        "根\0", "é", "z", "a\0", "a", "", "e\u{301}", "a", "根\0"
    ]))
    .unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
    assert_eq!(
        serde_json::to_value(serde_json::from_str::<CauseSet>("[]").unwrap()).unwrap(),
        json!([])
    );
    for invalid in [json!(null), json!({}), json!(["a", 1]), json!([null])] {
        assert!(serde_json::from_value::<CauseSet>(invalid).is_err());
    }
}

#[test]
fn propagation_projects_fifo_names_and_complete_typed_rows_into_the_actual_consumer() {
    use chrono_judge_filemap::verified_graph::{propagate, seed_topology};
    let allowed = oracle_edges(&[
        ("根\0", EdgeKind::Compile, "a"),
        ("根\0", EdgeKind::RuntimeInput, "a"),
        ("根\0", EdgeKind::TestExecution, "é"),
        ("a", EdgeKind::BuildInput, "é"),
        ("é", EdgeKind::Compile, "é"),
        ("é", EdgeKind::JudgeTrigger, "根\0"),
        ("孤", EdgeKind::TestExecution, "unreachable"),
    ]);
    let edges = graph::union(&BTreeSet::new(), &allowed);
    let seed = |id: &str, node: &str| Seed {
        id: id.into(),
        node: node.into(),
        reference: "explicit".into(),
        reason: "projection".into(),
    };
    let seeds = [seed("s", "根\0"), seed("isolated", "")];
    let topology = seed_topology(&edges, &seeds);
    let output = propagate(&topology, topology.indices.roots[0]);
    let actual: Vec<_> = output
        .rows
        .iter()
        .map(|row| {
            (
                row.node,
                row.outgoing
                    .iter()
                    .map(|edge| (edge.from.as_str(), edge.kind, edge.to.as_str()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                "根\0",
                vec![
                    ("根\0", EdgeKind::Compile, "a"),
                    ("根\0", EdgeKind::RuntimeInput, "a"),
                    ("根\0", EdgeKind::TestExecution, "é")
                ]
            ),
            ("a", vec![("a", EdgeKind::BuildInput, "é")]),
            (
                "é",
                vec![
                    ("é", EdgeKind::Compile, "é"),
                    ("é", EdgeKind::JudgeTrigger, "根\0")
                ]
            ),
        ]
    );
    assert_eq!(
        output.predecessors.witness("é").unwrap(),
        vec![Edge {
            from: "根\0".into(),
            kind: EdgeKind::TestExecution,
            to: "é".into(),
        }]
    );
    assert_eq!(output.predecessors.witness("unreachable"), None);
    let isolated = propagate(&topology, topology.indices.roots[1]);
    assert_eq!(isolated.rows.len(), 1);
    assert_eq!(isolated.rows[0].node, "");
    assert!(isolated.rows[0].outgoing.is_empty());
    assert_eq!(isolated.predecessors.witness(""), Some(vec![]));

    let closure = graph::closure(&edges, &seeds).unwrap();
    assert_eq!(
        serde_json::to_value(&closure.reached).unwrap(),
        json!({"": ["isolated"], "a": ["s"], "é": ["s"], "根\0": ["s"]})
    );
    assert_eq!(closure.traversed.len(), 6);
    assert!(closure.traversed.iter().all(|row| {
        row.seeds
            .entries()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            == ["s"]
    }));
    assert_eq!(closure.selected_tests(), BTreeSet::from(["é".into()]));
    assert_eq!(closure.witness("s", "é"), output.predecessors.witness("é"));
}

#[test]
fn predecessor_state_roundtrips_the_actual_sparse_json_and_cloned_witnesses() {
    let parents = json!({
        "": null,
        "a": {"from":"", "kind":"compile", "to":"a"},
        "根\0": {"from":"a", "kind":"test-execution", "to":"根\0"}
    });
    let value = json!({"reached":{}, "predecessors":{"seed\0":parents}, "traversed":[]});
    let state: graph::Closure = serde_json::from_value(value.clone()).unwrap();
    let expected = vec![
        Edge {
            from: "".into(),
            kind: EdgeKind::Compile,
            to: "a".into(),
        },
        Edge {
            from: "a".into(),
            kind: EdgeKind::TestExecution,
            to: "根\0".into(),
        },
    ];
    assert_eq!(state.witness("seed\0", "根\0"), Some(expected.clone()));
    assert_eq!(state.clone().witness("seed\0", "根\0"), Some(expected));
    assert_eq!(state.witness("seed\0", ""), Some(vec![]));
    assert_eq!(state.witness("missing", ""), None);
    assert_eq!(state.witness("seed\0", "missing"), None);
    assert_eq!(serde_json::to_value(&state).unwrap(), value);
    let text = serde_json::to_string(&state).unwrap();
    let decoded: graph::Closure = serde_json::from_str(&text).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), value);
}

#[test]
fn predecessor_witness_rejects_serialized_cycles_missing_rows_and_wrong_targets() {
    for parents in [
        json!({"a":{"from":"a", "kind":"compile", "to":"a"}}),
        json!({"a":{"from":"b", "kind":"compile", "to":"a"},
               "b":{"from":"a", "kind":"runtime-input", "to":"b"}}),
        json!({"a":{"from":"missing", "kind":"compile", "to":"a"}}),
        json!({"":null, "a":{"from":"", "kind":"compile", "to":"wrong"}}),
    ] {
        let state: graph::Closure = serde_json::from_value(json!({
            "reached":{}, "predecessors":{"s":parents}, "traversed":[]
        }))
        .unwrap();
        assert_eq!(state.witness("s", "a"), None, "{state:?}");
    }
    for parents in [
        json!([]),
        json!(null),
        json!({"a":42}),
        json!({"a":{"from":"", "kind":"unknown", "to":"a"}}),
    ] {
        assert!(
            serde_json::from_value::<graph::Closure>(json!({
                "reached":{}, "predecessors":{"s":parents}, "traversed":[]
            }))
            .is_err()
        );
    }
}

#[test]
fn file_origin_project_edge_change_points_to_its_actual_record() {
    let (i, f) = run("standard", "invalid_file_edge", &[delta(FM)]);
    let finding = f.iter().find(|f| f.code == "E_EDGE_TYPE").unwrap();
    assert_eq!(finding.delta_refs, [FM]);
    assert!(
        i.changes
            .iter()
            .any(|c| c.record.starts_with("/records/filemap/project_edges/"))
    );
}
#[test]
fn workflow_delta_selects_both_schemas_deleted_rules_and_rejects_foreign_paths() {
    let mut endpoints = views("standard", "standard");
    let rules = |paths: &[&str]| Ok(json!({"rule_paths":paths}));
    let (same, _) = produce(&endpoints, &[], rules(&[]));
    assert_eq!(same.workflow["integration_required"], false);
    assert_eq!(same.workflow["requirements"], json!([]));

    let workflow = ".chrono-harness/workflow.json";
    let old_path = ".chrono-harness/FILEMAP.json";
    let new_path = ".chrono-harness/new-filemap.json";
    endpoints.base.workflow["stability"] =
        json!([{"id":"removed","paths":["src.bin"],"tests":["t"]}]);
    for r in [&mut endpoints.base, &mut endpoints.candidate] {
        r.workflow["migrations"] = json!([
            {"id":"active","from_version":1,"to_version":2,"test":"t"},
            {"id":"unrelated","from_version":3,"to_version":2,"test":"absent-test"}
        ]);
        r.workflow["integration"]["tests"] = json!(["t"]);
    }
    endpoints
        .candidate
        .original_schemas
        .insert("filemap".into(), (new_path.into(), 2));
    let changes = [delta("src.bin"), delta(workflow), delta("rules.md")];
    let (impact, _) = produce(&endpoints, &changes, rules(&["rules.md"]));
    assert_eq!(impact.workflow["integration_required"], true);
    assert_eq!(
        impact.workflow["schema_transitions"],
        json!([{
            "registry":"filemap", "before":{"path":old_path,"version":1},
            "after":{"path":new_path,"version":2}
        }])
    );
    let rows = impact.workflow["requirements"].as_array().unwrap();
    let reasons: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r["endpoint"].as_str().unwrap(),
                r["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        reasons,
        [
            ("base", "schema-migration"),
            ("candidate", "schema-migration"),
            ("base", "stability"),
            ("base", "integration"),
            ("candidate", "integration")
        ]
    );
    assert_eq!(rows[0]["trigger_paths"], json!([old_path, new_path]));
    assert_eq!(rows[2]["trigger_paths"], json!([workflow, "src.bin"]));
    for row in rows {
        assert_eq!(row["tests"], json!(["test:t"]));
    }
    assert_eq!(
        impact.workflow["trigger_paths"],
        json!([old_path, new_path, workflow, "rules.md", "src.bin"])
    );
    let wire = serde_json::to_vec(&impact).unwrap();
    let summary = chrono_judge_filemap::impact_summary(wire.as_slice()).unwrap();
    assert_eq!(summary["tests"], json!(["test:t"]));
    let (denied, findings) = produce(&endpoints, &changes, rules(&["rules.md", "foreign"]));
    assert!(denied.workflow.is_null());
    assert_eq!(findings[0].code, "E_WORKFLOW_INPUT");
    assert_eq!(
        findings[0].message,
        "classification returned a rule path outside DELTA"
    );
}

#[test]
fn generated_workflow_edge_finding_names_its_fixed_stability_rule() {
    let mut endpoints = views("standard", "standard");
    endpoints.candidate.workflow["stability"] =
        json!([{"id":"hold","paths":["src.bin"],"tests":["missing"]}]);
    let changes = [delta("src.bin")];
    let (impact, findings) = produce(&endpoints, &changes, classification(&changes));
    let finding = findings
        .iter()
        .find(|f| f.code == "E_DANGLING_EDGE")
        .unwrap();
    assert_eq!(
        finding.delta_refs,
        ["/registries/candidate/workflow/stability/0"]
    );
    assert!(
        impact
            .seed_causes
            .iter()
            .any(|s| s.reference.starts_with("/impact/workflow/requirements/"))
    );
}

#[test]
fn file_edge_only_removal_and_addition_preserve_targets() {
    let (i, f) = run("standard", "removed_file_edge", &[delta(FM)]);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    assert!(
        i.seed_causes
            .iter()
            .any(|s| s.node == "project:p" && s.reason == "edge-change")
    );
    let (i, f) = run("removed_file_edge", "standard", &[delta(FM)]);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    assert!(
        i.edges
            .iter()
            .any(|e| e.origin == Origin::Candidate && e.edge.from == "file:src.bin")
    );
}
#[test]
fn identity_edge_and_input_array_reorder_is_no_semantic_change() {
    let (i, f) = run(
        "input_order",
        "reversed_inputs",
        &[delta(CONFIG), delta(FM)],
    );
    assert!(f.is_empty());
    assert!(i.changes.is_empty());
    assert!(tests(&i).is_empty());
}

#[test]
fn changed_cost_keeps_distinct_old_new_values_and_references() {
    let (i, f) = run("measured_cost", "new_cost", &[delta("src.bin"), delta(FM)]);
    assert!(f.is_empty());
    assert_eq!(tests(&i), ["test:t"]);
    let cost = &i.records["/records/filemap/test_costs/t"];
    assert_eq!(cost.base.as_ref().unwrap().value["cost"], "unknown");
    assert_eq!(
        i.records["/records/filemap/cost_models/unknown"]
            .base
            .as_ref()
            .unwrap()
            .value["cpu_ms"],
        17
    );
    assert_eq!(cost.candidate.as_ref().unwrap().value["cost"], "new");
    assert_eq!(
        i.records["/records/filemap/cost_models/new"]
            .candidate
            .as_ref()
            .unwrap()
            .value["cpu_ms"],
        json!(null)
    );
}

#[test]
fn removed_test_record_seeds_alias_and_exposes_retained_dangling_edge() {
    let (i, f) = run("standard", "removed_test", &[delta(PROJECTS)]);
    assert!(
        i.seed_causes
            .iter()
            .any(|s| s.node == "test:t" && s.reference == "/records/projects/projects/t")
    );
    assert!(
        tests(&i).is_empty(),
        "node removal alone does not traverse an incoming execution edge"
    );
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].code, "E_DANGLING_EDGE");
    assert_eq!(f[0].delta_refs, [PROJECTS]);
    assert_eq!(f[0].causes, ["test:t"]);
    // Registry selection can change even when the selected file is outside DELTA.
    let (impact, findings) = run("standard", "removed_test", &[]);
    assert_eq!(findings.len(), 1);
    let original = views("standard", "removed_test");
    let pointer = findings[0].delta_refs[0]
        .strip_prefix("/registries/base/projects")
        .unwrap();
    assert_eq!(original.base.projects.pointer(pointer).unwrap()["id"], "t");
    assert!(
        impact
            .seed_causes
            .iter()
            .any(|s| s.reference == "/records/projects/projects/t")
    );
    let (i, f) = run(
        "standard",
        "removed_test",
        &[delta(PROJECTS), delta("src.bin")],
    );
    assert_eq!(tests(&i), ["test:t"]);
    assert!(!i.required_tests[0].candidate_present);
    assert_eq!(f.len(), 1);
}

#[test]
fn execution_plan_order_is_semantic_and_selects_only_explicit_edges() {
    let (i, f) = run("ordered_plan", "reversed_plan", &[]);
    assert!(f.is_empty(), "{f:?}");
    assert_eq!(tests(&i), vec!["test:t"]);
    assert!(
        i.changes
            .iter()
            .any(|c| c.record.ends_with("/execution_plans/test:t")
                && c.fields.contains(&"/operations".into()))
    );
    assert!(tests(&run("isolated_plan", "isolated_reversed_plan", &[]).0).is_empty());
}

#[test]
fn full_assignment_changes_select_explicit_old_and_new_complete_plans() {
    let (impact, findings) = run("assigned_units", "exchanged_units", &[]);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(tests(&impact), ["test:t", "test:t2"]);
}

#[test]
fn declared_judge_inputs_remain_valid_when_migration_selects_them() {
    for after in ["judge_tool", "judge_path", "judge_digest"] {
        let (impact, findings) = run("judge_inputs", after, &[]);
        assert!(findings.is_empty(), "{findings:?}");
        assert!(impact.judges.iter().any(|j| j.node == "judge:registration"));
        assert!(
            tests(&impact).is_empty(),
            "judge inputs must not infer business test edges"
        );
    }
}

#[test]
fn tool_environment_and_retained_input_records_use_only_explicit_consumers() {
    for (after, isolated) in [
        ("consumer_tool", "isolated_tool"),
        ("consumer_path", "isolated_path"),
        ("consumer_digest", "isolated_digest"),
    ] {
        let (i, f) = run("consumer_inputs", after, &[]);
        assert!(f.is_empty(), "{f:?}");
        assert_eq!(tests(&i), vec!["test:t"]);
        assert!(tests(&run("isolated_inputs", isolated, &[]).0).is_empty());
    }
}

#[test]
fn explicit_historical_ambiguity_mapping_preserves_definitions_but_candidate_collision_fails() {
    let mut registered = views("ambiguous", "repaired_alias");
    registered.repaired.insert("test:t".into());
    let paths = [delta("src.bin")];
    let (i, f) = produce(&registered, &paths, classification(&paths));
    assert!(f.iter().all(|f| f.code != "E_NODE_AMBIGUOUS"), "{f:?}");
    assert_eq!(i.nodes["test:t"].base_definitions.len(), 2);
    assert_eq!(tests(&i), vec!["test:t"]);
    assert!(
        run("ambiguous", "absent_repair", &[delta("src.bin")])
            .1
            .iter()
            .any(|f| f.code == "E_NODE_AMBIGUOUS")
    );
    assert!(
        run("ambiguous", "remaining_collision", &[delta("src.bin")])
            .1
            .iter()
            .any(|f| f.code == "E_NODE_AMBIGUOUS")
    );
}

#[test]
fn retained_effective_environment_preserves_old_edges() {
    let mut views = views("old_environment", "removed_environment");
    views.environments = Some([
        BTreeMap::from([("MODE".into(), "old".into())]),
        BTreeMap::new(),
    ]);
    let (impact, findings) = produce(&views, &[], classification(&[]));
    assert!(findings.is_empty());
    assert_eq!(impact.tests, vec!["test:t"]);
    assert!(
        impact
            .seed_causes
            .iter()
            .any(|s| s.node == "environment:MODE")
    );
}

#[test]
fn artifact_owner_changes_and_removal_preserve_explicit_endpoint_consumers() {
    let (impact, findings) = run("project_artifact", "script_artifact", &[delta(CONFIG)]);
    assert!(findings.is_empty());
    assert_eq!(tests(&impact), ["test:st", "test:t"]);
    assert!(impact.seed_causes.iter().any(|s| s.node == "project:p"));
    assert!(impact.seed_causes.iter().any(|s| s.node == "script:s"));
    let (impact, findings) = run("project_artifact", "absent_artifact", &[delta(CONFIG)]);
    assert!(findings.is_empty());
    assert_eq!(tests(&impact), ["test:t"]);
    let (impact, findings) = run("project_artifact", "project_artifact", &[delta("doc.txt")]);
    assert!(findings.is_empty());
    assert!(tests(&impact).is_empty());
}

#[test]
fn group_registration_delta_and_endpoints_preserve_every_opaque_test_identity() {
    for paths in [vec![], vec![delta("src.bin")]] {
        let (impact, findings) = run("standard", "groups", &paths);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(tests(&impact), ["test:opaque-group", "test:t"]);
        assert!(impact.retired_tests.is_empty());
        assert_eq!(
            impact.nodes["test:opaque-group"].candidate_definitions[0].identity,
            "project:t"
        );
    }
    let (impact, findings) = run("groups", "rebound", &[]);
    assert!(findings.is_empty(), "{findings:?}");
    assert_eq!(tests(&impact), ["test:opaque-group", "test:t"]);
    let (impact, findings) = run("groups", "standard", &[]);
    assert!(
        findings.iter().all(|f| f.code != "E_REFERENCE"),
        "{findings:?}"
    );
    assert_eq!(
        impact
            .retired_tests
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["test:opaque-group"]
    );
}

#[test]
fn scheduling_cap_claim_changes_and_removal_keep_both_endpoint_targets() {
    for (other, both) in [
        ("cap", true),
        ("resource", false),
        ("no_claims", true),
        ("output", false),
    ] {
        for (before, after) in [("claims", other), (other, "claims")] {
            let (i, f) = run(before, after, &[]);
            assert!(f.is_empty(), "{f:?}");
            assert_eq!(
                tests(&i),
                if both {
                    vec!["test:st", "test:t"]
                } else {
                    vec!["test:t"]
                }
            );
        }
    }
    assert!(tests(&run("claims", "claims", &[]).0).is_empty());
}

#[test]
fn scheduling_priority_changes_reorder_and_removal_reach_both_endpoint_consumers() {
    for other in [
        "reordered_priority",
        "reduced_priority",
        "empty_priority",
        "absent_priority",
    ] {
        for (before, after) in [("priority", other), (other, "priority")] {
            let (impact, findings) = run(before, after, &[]);
            assert!(findings.is_empty(), "{findings:?}");
            assert_eq!(tests(&impact), ["test:st", "test:t"]);
        }
    }
    assert!(tests(&run("priority", "priority", &[]).0).is_empty());
    assert!(tests(&run("empty_priority", "absent_priority", &[]).0).is_empty());
    assert!(tests(&run("absent_priority", "empty_priority", &[]).0).is_empty());
}

#[test]
fn scheduling_resource_reassignment_and_artifact_changes_reach_declared_consumers() {
    for other in ["shared_resource", "reassigned_artifact"] {
        for (before, after) in [("shared_output", other), (other, "shared_output")] {
            let (i, f) = run(before, after, &[]);
            assert!(f.is_empty(), "{f:?}");
            assert_eq!(tests(&i), ["test:st", "test:t"]);
        }
    }
}

#[test]
fn impact_summary_validates_unprojected_fields_and_preserves_the_two_consumers_inputs() {
    let (impact, findings) = run("standard", "standard", &[delta("src.bin")]);
    assert!(findings.is_empty());
    let mut wire = serde_json::to_value(&impact).unwrap();
    let summary = |data: &serde_json::Value| {
        let bytes = serde_json::to_vec(data).unwrap();
        chrono_judge_filemap::impact_summary(bytes.as_slice())
    };
    assert_eq!(
        summary(&wire).unwrap(),
        json!({
            "schema": impact.schema, "delta": impact.delta,
            "reached": impact.closure.reached.keys().collect::<Vec<_>>(),
            "required_tests": impact.required_tests.iter().map(|t| &t.node).collect::<Vec<_>>(),
            "tests": impact.tests, "retired_tests": impact.retired_tests,
        })
    );
    assert!(!impact.closure.reached.is_empty());
    wire["edges"] = json!([{"edge":{"from":"a","kind":"unknown","to":"b"},"origin":"base"}]);
    assert_eq!(
        summary(&wire).unwrap_err().to_string(),
        serde_json::from_value::<Impact>(wire.clone())
            .unwrap_err()
            .to_string()
    );
    wire.as_object_mut().unwrap().remove("edges");
    assert_eq!(
        summary(&wire).unwrap_err().to_string(),
        serde_json::from_value::<Impact>(wire.clone())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn a_language_change_selects_the_registered_test_for_projects_and_scripts() {
    for (prefix, before, after) in [
        ("project", "rust_projects", "python_test_project"),
        ("script", "rust_scripts", "python_test_script"),
    ] {
        let (impact, findings) = run(before, after, &[delta(PROJECTS)]);
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(tests(&impact), ["test:t"]);
        assert!(impact.closure.reached.contains_key(&format!("{prefix}:t")));
        let (unrelated, findings) = run(after, after, &[delta(".gitignore")]);
        assert!(findings.is_empty(), "{findings:?}");
        assert!(tests(&unrelated).is_empty());
        assert_eq!(
            unrelated
                .closure
                .reached
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["file:.gitignore"]
        );
    }
}

#[test]
fn endpoint_tests_uses_fixed_trees_and_returns_blocking_findings_to_the_caller() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_endpoints(root);
    fs::write(root.join("src.bin"), b"before").unwrap();
    let base = commit(root);
    fs::write(root.join("src.bin"), b"after").unwrap();
    let head = commit(root);
    let query = |candidate: &str| {
        serde_json::to_vec(&json!({"config":CONFIG,"base":base,"candidate":candidate})).unwrap()
    };
    let selected = chrono_judge_filemap::endpoint_tests(root, query(&head).as_slice()).unwrap();
    assert_eq!(selected["tests"], json!(["test:t"]));
    assert_eq!(selected["retired_tests"], json!([]));
    assert_eq!(selected["findings"], json!([]));
    fs::write(root.join("unregistered.txt"), b"unmapped").unwrap();
    let rejected = commit(root);
    let findings = chrono_judge_filemap::endpoint_tests(root, query(&rejected).as_slice()).unwrap();
    assert!(
        findings["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["level"] == "error")
    );
    assert!(chrono_judge_filemap::endpoint_tests(root, query("dev").as_slice()).is_err());
    for (mode, expected) in [
        (
            "wrong-identity",
            "E_PROTOCOL: registration impact query identity",
        ),
        ("incomplete", "missing field `nodes`"),
        ("duplicate", "duplicate JSON member error"),
        ("error", "registry rejected"),
    ] {
        fs::write(root.join("registration-reply"), mode).unwrap();
        assert!(
            chrono_judge_filemap::endpoint_tests(root, query(&head).as_slice())
                .unwrap_err()
                .contains(expected)
        );
    }
}

#[test]
fn closure_query_keeps_removed_edges_cycles_and_only_reachable_causes_for_replay() {
    let removed = edge("a", "compile", "b");
    let back = edge("b", "runtime-input", "a");
    let test = edge("b", "test-execution", "test:t");
    let other = edge("x", "runtime-input", "y");
    let mut request = json!({"base":[removed,back,other],"candidate":[back,test,other],
        "seeds":[{"id":"first","node":"a","reference":"a","reason":"change"}],"reachable_only":true});
    let query = |request: &serde_json::Value| {
        graph::query_closure(serde_json::to_vec(request).unwrap().as_slice())
    };
    let replay = query(&request).unwrap();
    assert_eq!(replay["reached"], json!(["a", "b", "test:t"]));
    assert_eq!(replay["selected_tests"], json!(["test:t"]));
    assert!(replay.get("closure").is_none());
    let mut expected = vec![];
    for (mut edge, origin) in [(removed, "base"), (back, "both"), (test, "candidate")] {
        edge["origin"] = json!(origin);
        expected.push(edge);
    }
    let edges = replay["edges"].as_array().unwrap();
    assert_eq!(edges.len(), expected.len());
    assert!(expected.iter().all(|edge| edges.contains(edge)));
    request["reachable_only"] = json!(false);
    let full = query(&request).unwrap();
    assert_eq!(full["edges"].as_array().unwrap().len(), 4);
    assert_eq!(full["closure"]["reached"]["test:t"], json!(["first"]));
    assert_eq!(full["closure"]["traversed"].as_array().unwrap().len(), 3);
    request["candidate"][0]["kind"] = json!("unknown");
    assert!(query(&request).is_err());
}

use verus_builtin_macros::verus;

#[path = "../../judge-filemap/src/registration_environment.rs"]
mod registration_environment;

#[test]
fn registration_scheduling_gets_only_observed_inherited_values() {
    use registration_environment::child_environment;
    let inherited =
        json!({"PATH": "/bound", "BUSINESS": "original", "UNSET": null, "CREDENTIAL": null});
    let env = child_environment(Some("/lookup".into()), Some(&inherited)).unwrap();
    assert_eq!(json!(env), json!({"PATH":"/bound", "BUSINESS":"original"}));
    let absent = json!({"PATH":null});
    assert!(
        child_environment(Some("/lookup".into()), Some(&absent))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        json!(child_environment(Some("/lookup".into()), None).unwrap()),
        json!({"PATH":"/lookup"})
    );
    assert!(child_environment(None, Some(&json!({"BUSINESS":42}))).is_err());
}
