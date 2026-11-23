//! Candidate FILEMAP impact policy. No operation execution, retirement or cost verdicts.
mod classification;
pub mod cost_inputs;
pub mod graph;
mod records;
/// Public change and finding types in FILEMAP impact evaluations.
mod request;
mod runtime;
mod strict_json;
#[allow(unexpected_cfgs)]
pub mod verified_graph;
mod workflow;
pub use request::Finding;
use request::Request;
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delta {
    pub kind: String,
    pub path: String,
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
}
pub mod registration;
use graph::{Closure, Edge, EdgeKind, Origin, Seed, UnionEdge};
pub use records::{Change, Record, RecordPair};
use registration::{Endpoint, NodeKind, NodeView, Views};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use verified_graph::valid_types;

pub const IMPACT_SCHEMA: &str = "chrono-filemap-impact/v6";
/// Complete endpoint definitions; zero is absent, several are ambiguous.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodePair {
    pub base_definitions: Vec<NodeDefinition>,
    pub candidate_definitions: Vec<NodeDefinition>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeDefinition {
    pub identity: String,
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeAmbiguity {
    pub node: String,
    pub endpoint: String,
    pub definitions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequiredTest {
    pub node: String,
    pub candidate_present: bool,
    pub execution_edges: Vec<Edge>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JudgeSelection {
    pub node: String,
    pub triggers: Vec<Edge>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EdgeProblem {
    pub edge: Edge,
    pub endpoint: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Impact {
    pub schema: String,
    pub workflow: Value,
    pub delta: Vec<Delta>,
    pub records: BTreeMap<String, RecordPair>,
    pub changes: Vec<Change>,
    pub nodes: BTreeMap<String, NodePair>,
    pub edges: Vec<UnionEdge>,
    pub seed_causes: Vec<Seed>,
    pub closure: Closure,
    pub tests: Vec<String>,
    pub retired_tests: Vec<String>,
    pub required_tests: Vec<RequiredTest>,
    pub judges: Vec<JudgeSelection>,
    pub historical_context: Vec<EdgeProblem>,
    pub historical_ambiguities: Vec<NodeAmbiguity>,
    pub limits: Vec<String>,
}
/// Decode the complete public impact before projecting fields used by projects and costs.
pub fn impact_summary(input: impl std::io::Read) -> Result<Value, serde_json::Error> {
    // The judge protocol hands its consumers a JSON value. Decode from that same
    // representation so a rejected field keeps the original Serde diagnostic.
    let value: Value = serde_json::from_reader(input)?;
    let impact: Impact = serde_json::from_value(value)?;
    Ok(summarize(impact))
}

fn summarize(impact: Impact) -> Value {
    json!({
        "schema": impact.schema,
        "delta": impact.delta,
        "reached": impact.closure.reached.into_keys().collect::<Vec<_>>(),
        "required_tests": impact.required_tests.into_iter().map(|test| test.node).collect::<Vec<_>>(),
        "tests": impact.tests,
        "retired_tests": impact.retired_tests,
    })
}

/// Read explicit edges. Consistency adapters may consume this inventory; it
/// never discovers an input or creates an authoritative dependency.
pub fn edges(r: &Endpoint) -> BTreeSet<Edge> {
    let mut out = BTreeSet::new();
    let mut add = |from: &str, e: &Value| {
        out.insert(Edge {
            from: from.into(),
            kind: serde_json::from_value(e["kind"].clone()).unwrap(),
            to: e["to"].as_str().unwrap().into(),
        });
    };
    for f in r.filemap()["files"].as_array().unwrap() {
        for e in f["edges"].as_array().unwrap() {
            add(&format!("file:{}", f["path"].as_str().unwrap()), e);
        }
    }
    for e in r.filemap()["project_edges"].as_array().unwrap() {
        add(e["from"].as_str().unwrap(), e);
    }
    out
}
/// Pure declaration-level impact. External input records are compared, not retained input bytes.
/// Callers must supply factual, complete DELTA; the judge entrypoint binds it to fixed Git objects.
pub fn produce(
    views: &Views,
    delta: &[Delta],
    classification: Result<Value, String>,
) -> (Impact, Vec<Finding>) {
    let (base, candidate) = (&views.base, &views.candidate);
    let (an, bn) = (&base.nodes, &candidate.nodes);
    let environments = views.environments.as_ref();
    let a = records::inventory(base, an, environments.map(|p| &p[0]));
    let b = records::inventory(candidate, bn, environments.map(|p| &p[1]));
    let mut findings = vec![];
    let workflow = match classification
        .and_then(|changes| workflow::select(base, candidate, delta, [&a, &b], changes))
    {
        Ok(selection) => selection,
        Err(message) => {
            findings.push(Finding {
                code: message
                    .split(':')
                    .next()
                    .filter(|s| s.starts_with("E_"))
                    .unwrap_or("E_WORKFLOW_INPUT")
                    .into(),
                level: "error".into(),
                message,
                delta_refs: vec!["/request".into()],
                causes: vec![],
            });
            Value::Null
        }
    };
    let mut records = BTreeMap::new();
    let mut changes = vec![];
    let mut seeds = BTreeSet::new();
    let seed = |node: String, reference: String, reason: &str| Seed {
        id: serde_json::to_string(&json!([reference, reason, node])).unwrap(),
        node,
        reference,
        reason: reason.into(),
    };
    for d in delta {
        seeds.insert(seed(
            format!("file:{}", d.path),
            d.path.clone(),
            "delta-path",
        ));
    }
    for id in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        let av = a.get(id);
        let bv = b.get(id);
        let mut fields = vec![];
        records::changed_fields(av.map(|r| &r.value), bv.map(|r| &r.value), "", &mut fields);
        // Registry relocation is an input change even when the record's value is identical.
        if av.zip(bv).is_some_and(|(a, b)| a.path != b.path) {
            fields.push("/$registry_path".into());
        }
        if !fields.is_empty() {
            for record in av.into_iter().chain(bv) {
                for target in &record.targets {
                    seeds.insert(seed(target.clone(), id.clone(), "record-change"));
                }
            }
            changes.push(Change {
                record: id.clone(),
                fields,
            });
        }
        records.insert(
            id.clone(),
            RecordPair {
                base: av.cloned(),
                candidate: bv.cloned(),
            },
        );
    }
    let declared_ae = edges(base);
    let declared_be = edges(candidate);
    let mut ae = declared_ae.clone();
    let mut be = declared_be.clone();
    ae.extend(workflow::edges(&workflow, "base"));
    be.extend(workflow::edges(&workflow, "candidate"));
    let edges = graph::union(&ae, &be);
    for e in ae.symmetric_difference(&be) {
        let file_record = e
            .from
            .strip_prefix("file:")
            .map(|p| format!("/records/filemap/files/{}", records::escape(p)));
        let declared_on_file = [base, candidate].iter().any(|r| {
            r.filemap()["files"].as_array().unwrap().iter().any(|f| {
                format!("file:{}", f["path"].as_str().unwrap()) == e.from
                    && f["edges"].as_array().unwrap().iter().any(|row| {
                        row["to"] == e.to && row["kind"] == serde_json::to_value(e.kind).unwrap()
                    })
            })
        });
        let reference = file_record
            .filter(|_| declared_on_file)
            .or_else(|| workflow::reference(&workflow, e))
            .unwrap_or_else(|| {
                format!(
                    "/records/filemap/project_edges/{}",
                    records::escape(
                        &serde_json::to_string(&json!([e.from, e.kind, e.to])).unwrap()
                    )
                )
            });
        for node in [&e.from, &e.to] {
            seeds.insert(seed(node.clone(), reference.clone(), "edge-change"));
        }
    }
    let seeds: Vec<_> = seeds.into_iter().collect();
    let closure = graph::closure(&edges, &seeds);
    let definitions = |view: Option<&NodeView>| {
        view.into_iter()
            .flat_map(|v| &v.definitions)
            .map(|d| NodeDefinition {
                identity: d.identity.clone(),
                value: d.value.clone(),
            })
            .collect()
    };
    let nodes = an
        .keys()
        .chain(bn.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            (
                id.clone(),
                NodePair {
                    base_definitions: definitions(an.get(id)),
                    candidate_definitions: definitions(bn.get(id)),
                },
            )
        })
        .collect();
    let mut historical_context = vec![];
    let mut historical_ambiguities = vec![];
    for (endpoint, inventory) in [("base", an), ("candidate", bn)] {
        for (node, view) in inventory {
            if view.definitions.len() <= 1 {
                continue;
            }
            let ambiguity = NodeAmbiguity {
                node: node.clone(),
                endpoint: endpoint.into(),
                definitions: view
                    .definitions
                    .iter()
                    .map(|d| d.identity.clone())
                    .collect(),
            };
            if let Some(causes) = closure.reached.get(node) {
                if endpoint == "base" && views.repaired.contains(node) {
                    historical_ambiguities.push(ambiguity);
                    continue;
                }
                let cause = causes.first().unwrap();
                let seed = verified_graph::seed_by_id(&seeds, cause).unwrap();
                let mut witness = vec![seed.node.clone()];
                witness.extend(
                    closure
                        .witness(cause, node)
                        .unwrap()
                        .iter()
                        .map(|e| e.to.clone()),
                );
                findings.push(Finding {
                    code: "E_NODE_AMBIGUOUS".into(),
                    level: "error".into(),
                    message: format!(
                        "{endpoint}: ambiguous node {node}: {:?}",
                        ambiguity.definitions
                    ),
                    delta_refs: vec![seed.reference.clone()],
                    causes: witness,
                });
            } else {
                historical_ambiguities.push(ambiguity);
            }
        }
    }
    for e in &edges {
        for (endpoint, inventory) in [("base", an), ("candidate", bn)] {
            if matches!(
                (e.origin, endpoint),
                (Origin::Base, "candidate") | (Origin::Candidate, "base")
            ) {
                continue;
            }
            let edge = &e.edge;
            // Workflow conditions refer to this DELTA's union of paths. A new or
            // deleted trigger can require tests declared at the other endpoint.
            // Ordinary FILEMAP edges retain their endpoint-local validation.
            let declared = if endpoint == "base" {
                &declared_ae
            } else {
                &declared_be
            };
            let from = inventory.get(&edge.from).or_else(|| {
                if !declared.contains(edge) && workflow::reference(&workflow, edge).is_some() {
                    an.get(&edge.from).or_else(|| bn.get(&edge.from))
                } else {
                    None
                }
            });
            let message = match (from, inventory.get(&edge.to)) {
                (Some(from), Some(to)) if !valid_types(edge.kind, from.kind, to.kind) => {
                    Some("invalid typed edge endpoints".to_string())
                }
                (None, _) => Some(format!("missing source node {}", edge.from)),
                (_, None) => Some(format!("missing target node {}", edge.to)),
                _ => None,
            };
            if let Some(message) = message {
                let problem = EdgeProblem {
                    edge: edge.clone(),
                    endpoint: endpoint.into(),
                    message: message.clone(),
                };
                let reached = closure
                    .reached
                    .get(&edge.from)
                    .map(|s| (&edge.from, s))
                    .or_else(|| closure.reached.get(&edge.to).map(|s| (&edge.to, s)));
                if let Some((node, causes)) = reached {
                    let cause = causes.first().unwrap();
                    let seed = verified_graph::seed_by_id(&seeds, cause).unwrap();
                    let path = closure.witness(cause, node).unwrap();
                    let mut witness = vec![seed.node.clone()];
                    witness.extend(path.iter().map(|e| e.to.clone()));
                    if node == &edge.from {
                        witness.push(edge.to.clone());
                    }
                    findings.push(Finding {
                        code: if message.starts_with("missing") {
                            "E_DANGLING_EDGE"
                        } else {
                            "E_EDGE_TYPE"
                        }
                        .into(),
                        level: "error".into(),
                        message: format!("{endpoint}: {message}: {edge:?}"),
                        delta_refs: vec![seed.reference.clone()],
                        causes: witness,
                    });
                } else {
                    historical_context.push(problem);
                }
            }
        }
    }
    let required_tests: Vec<_> = closure
        .selected_tests()
        .into_iter()
        .map(|node| RequiredTest {
            candidate_present: bn.get(&node).is_some_and(|n| n.kind == NodeKind::Test),
            execution_edges: closure
                .traversed
                .iter()
                .filter(|e| e.edge.kind == EdgeKind::TestExecution && e.edge.to == node)
                .map(|e| e.edge.clone())
                .collect(),
            node,
        })
        .collect();
    let mut judges: Vec<_> = candidate.judges()["judges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| {
            let node = format!("judge:{}", j["id"].as_str().unwrap());
            JudgeSelection {
                triggers: closure
                    .traversed
                    .iter()
                    .filter(|e| e.edge.kind == EdgeKind::JudgeTrigger && e.edge.to == node)
                    .map(|e| e.edge.clone())
                    .collect(),
                node,
            }
        })
        .collect();
    judges.sort_by(|a, b| a.node.cmp(&b.node));
    (
        Impact {
            schema: IMPACT_SCHEMA.into(),
            workflow,
            delta: delta.to_vec(),
            records,
            changes,
            nodes,
            edges,
            seed_causes: seeds,
            closure,
            tests: required_tests
                .iter()
                .filter(|t| t.candidate_present)
                .map(|t| t.node.clone())
                .collect(),
            retired_tests: required_tests
                .iter()
                .filter(|t| !t.candidate_present)
                .map(|t| t.node.clone())
                .collect(),
            required_tests,
            judges,
            historical_context,
            historical_ambiguities,
            limits: vec![
                "Declaration impact only; no test execution, retirement approval or cost verdict"
                    .into(),
                "External input declaration changes are not retained input evidence".into(),
                if environments.is_some() {
                    "Retained effective environments compared; completeness remains unproven".into()
                } else {
                    "Declaration-only API: retained effective environment differences not evaluated"
                        .into()
                },
                "Actual input completeness and locality are unproven; unknown costs remain null"
                    .into(),
            ],
        },
        findings,
    )
}
/// Fixed semantic documents are consumed through the same full impact producer.
/// Declaration-level scheduling impact, before external SDK snapshots exist.
/// Final admission continues to use produce_for_request with actual validated inputs.
/// Query declaration-level test obligations from two fixed commits of this host.
pub fn endpoint_tests(root: &std::path::Path, input: impl std::io::Read) -> Result<Value, String> {
    #[derive(Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        config: String,
        base: String,
        candidate: String,
        #[serde(default)]
        scheduling: bool,
    }
    let query: Query = serde_json::from_reader(input).map_err(|error| error.to_string())?;
    let mut reader = runtime::Reader::open(
        root,
        json!({"mode":"config","config":query.config,"base":query.base,"candidate":query.candidate}),
    )?;
    let reply = registration::read(
        root,
        &mut reader,
        json!({"mode":"endpoints","base":query.base,"candidate":query.candidate,"config":query.config,"scheduling":query.scheduling}),
        [&query.base, &query.candidate],
        None,
        None,
    )?;
    let views = reply.views?;
    let delta = reader.delta()?;
    let changes = classification::for_endpoints(
        root,
        [&views.base, &views.candidate],
        &delta,
        &mut reader,
        None,
    )?;
    let (impact, findings) = produce(&views, &delta, changes);
    let mut result =
        json!({"tests":impact.tests,"retired_tests":impact.retired_tests,"findings":findings});
    if query.scheduling {
        result["reached"] = json!(impact.closure.reached.keys().collect::<Vec<_>>());
        result["changed_paths"] = json!(delta.iter().map(|item| &item.path).collect::<Vec<_>>());
        result["git_facts"] = registration::with_registration_facts(
            reader.observation()?,
            reply.position,
            reply.git_facts,
        )?;
    }
    reader.close()?;
    Ok(result)
}

/// Scheduling projects fixed endpoints. Collection projects the caller's validated
/// registration view and effective inputs, including historical conversion.
pub fn collection_tests(
    root: &std::path::Path,
    input: impl std::io::Read,
) -> Result<Value, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        request: Value,
        migration: Option<Value>,
    }
    let query: Query = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let req = &Request::new(query.request)?;
    if req.candidate.root != root
        || req.scope.as_ref().and_then(|s| s["kind"].as_str()) != Some("collect")
    {
        return Err("FILEMAP collection requires this checkout and a collection scope".into());
    }
    let mut reader = runtime::Reader::open(root, json!({"mode":"request","request":req.raw}))?;
    let reply = registration::read(
        root,
        &mut reader,
        json!({"mode":"collection","request":req.raw,"migration":query.migration}),
        [&req.base.commit, &req.candidate.commit],
        Some(&req.request_id),
        req.observations["environment"]["effective"]["PATH"].as_str(),
    )?;
    let views = reply.views?;
    let (impact, findings) = produce_for_request(req, &views, &mut reader)?;
    let git_facts = registration::with_registration_facts(
        reader.observation()?,
        reply.position,
        reply.git_facts,
    )?;
    reader.close()?;
    Ok(
        json!({"tests":impact.tests,"retired_tests":impact.retired_tests,"findings":findings,
        "reached":impact.closure.reached.keys().collect::<Vec<_>>(), "git_facts":git_facts}),
    )
}

fn produce_for_request(
    req: &Request,
    views: &Views,
    reader: &mut runtime::Reader,
) -> Result<(Impact, Vec<Finding>), String> {
    let path = req.observations["environment"]["effective"]["PATH"].as_str();
    let changes = classification::for_endpoints(
        &req.candidate.root,
        [&views.base, &views.candidate],
        &req.delta,
        reader,
        path,
    )?;
    if let Some(error) = &views.environment_error {
        return Err(error.clone());
    }
    Ok(produce(views, &req.delta, changes))
}
pub fn decision(input: impl std::io::Read) -> Result<Value, String> {
    let envelope: Value = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let req = Request::new(envelope["request"].clone())?;
    let mut outputs = serde_json::Map::new();
    let result = (|| {
        let mut reader = runtime::Reader::open(
            &req.candidate.root,
            json!({"mode":"request","request":req.raw}),
        )?;
        let mut remote = None;
        let result = evaluate(&req, &mut reader, &mut remote);
        let (position, remote) = remote.unwrap_or_default();
        let facts = registration::with_registration_facts(reader.observation()?, position, remote)?;
        if !facts.is_null() {
            outputs.insert("git_facts".into(), facts);
        }
        reader.close()?;
        result
    })();
    let (status, findings) = match result {
        Ok((impact, findings, view)) => {
            let status = if findings.is_empty() { "pass" } else { "fail" };
            outputs.insert(
                "impact".into(),
                serde_json::to_value(impact).map_err(|e| e.to_string())?,
            );
            outputs.insert("registration_view".into(), view);
            (status, findings)
        }
        Err(message) => {
            let code = message
                .split(':')
                .next()
                .filter(|code| code.starts_with("E_SEMANTIC_"))
                .unwrap_or("E_INPUT")
                .to_owned();
            (
                "error",
                vec![Finding {
                    code,
                    message,
                    level: "error".into(),
                    delta_refs: vec!["/request".into()],
                    causes: vec![],
                }],
            )
        }
    };
    Ok(json!({"status":status,"findings":findings,"outputs":outputs}))
}
fn evaluate(
    req: &Request,
    reader: &mut runtime::Reader,
    remote_facts: &mut Option<(usize, Option<Value>)>,
) -> Result<(Impact, Vec<Finding>, Value), String> {
    let root = &req.candidate.root;
    if reader.verify_tree("base")? != req.base.tree
        || reader.verify_tree("candidate")? != req.candidate.tree
    {
        return Err("fixed endpoint tree mismatch".into());
    }
    if reader.delta()? != req.delta {
        return Err("DELTA differs from fixed endpoints".into());
    }
    let reply = registration::read(
        root,
        reader,
        json!({"mode":"request","request":req.raw}),
        [&req.base.commit, &req.candidate.commit],
        Some(&req.request_id),
        req.observations["environment"]["effective"]["PATH"].as_str(),
    )?;
    *remote_facts = Some((reply.position, reply.git_facts));
    let mut views = reply.views?;
    let view = views
        .registration_view
        .take()
        .ok_or("missing registration view")?;
    let (impact, findings) = produce_for_request(req, &views, reader)?;
    Ok((impact, findings, view))
}
