//! Candidate FILEMAP impact policy. No operation execution, retirement or cost verdicts.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use super::RequiredTest;

verus! {
/// The public summary retains every requested test's node, in the original report order.
pub fn required_names(tests: Vec<RequiredTest>) -> (names: Vec<String>)
    ensures names@ == tests@.map_values(|test: RequiredTest| test.node),
{
    let mut names = Vec::new();
    for test in it:tests.into_iter()
        invariant names@ == it.elements.take(it.pos as int).map_values(|t: RequiredTest| t.node),
    {
        names.push(test.node);
    }
    names
}

pub fn evaluated_status(findings: usize) -> (status: &'static str)
    ensures status@ == if findings == 0 { "pass"@ } else { "fail"@ },
{
    if findings == 0 { "pass" } else { "fail" }
}
}
#[cfg(not(verus_keep_ghost))]
mod classification;
#[cfg(not(verus_keep_ghost))]
pub mod cost_inputs;
#[cfg(not(verus_keep_ghost))]
pub mod graph;
#[cfg(not(verus_keep_ghost))]
mod records;
/// Public change and finding types in FILEMAP impact evaluations.
#[cfg(not(verus_keep_ghost))]
mod request;
#[cfg(not(verus_keep_ghost))]
mod runtime;
#[cfg(not(verus_keep_ghost))]
mod strict_json;
#[allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
pub mod verified_graph;
#[cfg(not(verus_keep_ghost))]
mod workflow;
#[cfg(not(verus_keep_ghost))]
pub use request::Finding;
#[cfg(not(verus_keep_ghost))]
use request::Request;
#[cfg(not(verus_keep_ghost))]
pub use verified_graph::Delta;
#[cfg(not(verus_keep_ghost))]
pub mod registration;
#[cfg(not(verus_keep_ghost))]
use graph::{Closure, Edge, EdgeKind, Origin, Seed, UnionEdge};
#[cfg(not(verus_keep_ghost))]
pub use records::{Change, Record, RecordPair};
#[cfg(not(verus_keep_ghost))]
use registration::{Endpoint, NodeKind, NodeView, Views};
#[cfg(not(verus_keep_ghost))]
use serde::{Deserialize, Serialize};
#[cfg(not(verus_keep_ghost))]
use serde_json::{Value, json};
#[cfg(not(verus_keep_ghost))]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(verus_keep_ghost))]
use verified_graph::{finding_origin, valid_types};

#[cfg(not(verus_keep_ghost))]
struct CandidateTests<'a>(&'a BTreeMap<String, NodeView>);

#[cfg(not(verus_keep_ghost))]
impl verified_graph::CandidateTestLookup for CandidateTests<'_> {
    fn is_test(&self, node: &str) -> bool {
        self.0
            .get(node)
            .is_some_and(|row| row.kind == NodeKind::Test)
    }
}

#[cfg(not(verus_keep_ghost))]
pub const IMPACT_SCHEMA: &str = "chrono-filemap-impact/v6";
/// Complete endpoint definitions; zero is absent, several are ambiguous.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct NodePair {
    pub base_definitions: Vec<NodeDefinition>,
    pub candidate_definitions: Vec<NodeDefinition>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct NodeDefinition {
    pub identity: String,
    pub value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct NodeAmbiguity {
    pub node: String,
    pub endpoint: String,
    pub definitions: Vec<String>,
}
#[cfg(not(verus_keep_ghost))]
pub use verified_graph::RequiredTest;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct JudgeSelection {
    pub node: String,
    pub triggers: Vec<Edge>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
pub struct EdgeProblem {
    pub edge: Edge,
    pub endpoint: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg(not(verus_keep_ghost))]
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
#[cfg(not(verus_keep_ghost))]
pub fn impact_summary(input: impl std::io::Read) -> Result<Value, serde_json::Error> {
    // The judge protocol hands its consumers a JSON value. Decode from that same
    // representation so a rejected field keeps the original Serde diagnostic.
    let value: Value = serde_json::from_reader(input)?;
    let impact: Impact = serde_json::from_value(value)?;
    Ok(summarize(impact))
}

#[cfg(not(verus_keep_ghost))]
fn summarize(impact: Impact) -> Value {
    json!({
        "schema": impact.schema,
        "delta": impact.delta,
        "reached": impact.closure.reached.into_keys().collect::<Vec<_>>(),
        "required_tests": required_names(impact.required_tests),
        "tests": impact.tests,
        "retired_tests": impact.retired_tests,
    })
}

/// Read explicit edges. Consistency adapters may consume this inventory; it
/// never discovers an input or creates an authoritative dependency.
#[cfg(not(verus_keep_ghost))]
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
#[cfg(not(verus_keep_ghost))]
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
                code: verified_graph::classification_code(
                    message.split(':').next().unwrap_or_default(),
                )
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
    verified_graph::insert_delta_seeds(&mut seeds, delta, &graph::JsonSeedId);
    for id in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        let av = a.get(id).map(|r| &r.record);
        let bv = b.get(id).map(|r| &r.record);
        let mut fields = vec![];
        records::changed_fields(av.map(|r| &r.value), bv.map(|r| &r.value), "", &mut fields);
        if verified_graph::register_record_change(
            &mut fields,
            &mut seeds,
            &graph::JsonSeedId,
            id,
            av.map(|r| (r.path.as_str(), r)),
            bv.map(|r| (r.path.as_str(), r)),
        ) {
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
        let reference = workflow::reference(&workflow, e, file_record.filter(|_| declared_on_file));
        verified_graph::insert_edge_seeds(&mut seeds, &graph::JsonSeedId, e, &reference);
    }
    let seeds: Vec<_> = seeds.into_iter().collect();
    let closure = graph::closure_for_bound_roots(&edges, &seeds);
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
    // Stable impact keys identify seeds across endpoints. A wire finding points
    // into the fixed input that supplied its seed, including base-only records.
    let finding_reference = |seed: &Seed, endpoint: &str| -> String {
        if seed.reference.starts_with("/records/") {
            let rank = |record: &records::InputRecord| {
                finding_origin::record_rank(
                    delta,
                    &record.record.path,
                    record.record.targets.contains(&seed.node),
                )
            };
            let ((side, record), rank) = finding_origin::record_source(
                a.get(&seed.reference)
                    .map(|record| (("base", record), rank(record))),
                b.get(&seed.reference)
                    .map(|record| (("candidate", record), rank(record))),
                endpoint == "base",
            )
            .expect("seed has a record at one endpoint");
            return finding_origin::record_reference(
                rank,
                &record.record.path,
                side,
                &record.pointer,
            );
        }
        if let Some(row) = seed
            .reference
            .strip_prefix("/impact/workflow")
            .and_then(|pointer| workflow.pointer(pointer))
        {
            let side = row["endpoint"].as_str().unwrap();
            let pointer = row["pointer"].as_str().unwrap();
            let origin = if side == "base" { base } else { candidate };
            return finding_origin::workflow_reference(
                side,
                pointer,
                origin.workflow()["schema_version"].as_u64(),
                origin.original_schemas()["workflow"].1,
            );
        }
        seed.reference.clone() // direct DELTA path
    };
    let mut historical_context = vec![];
    let mut historical_ambiguities = vec![];
    for (endpoint, inventory) in [("base", an), ("candidate", bn)] {
        for (node, view) in inventory {
            use finding_origin::Ambiguity;
            let disposition = finding_origin::ambiguity(
                view.definitions.len(),
                endpoint == "base",
                views.repaired.contains(node),
                closure.reached.get(node),
            );
            if matches!(disposition, Ambiguity::Unique) {
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
            if let Ambiguity::Reached(causes) = disposition {
                let cause = causes.first().unwrap();
                let seed = verified_graph::seed_by_id(&seeds, cause).unwrap();
                let path = closure.witness(cause, node).unwrap();
                let witness = finding_origin::witness(&seed.node, &path, None);
                findings.push(Finding {
                    code: "E_NODE_AMBIGUOUS".into(),
                    level: "error".into(),
                    message: format!(
                        "{endpoint}: ambiguous node {node}: {:?}",
                        ambiguity.definitions
                    ),
                    delta_refs: vec![finding_reference(seed, endpoint)],
                    causes: witness,
                });
            } else {
                historical_ambiguities.push(ambiguity);
            }
        }
    }
    for e in &edges {
        for (endpoint, inventory) in [("base", an), ("candidate", bn)] {
            if !finding_origin::on_endpoint(&e.origin, endpoint == "base") {
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
                if !declared.contains(edge) {
                    an.get(&edge.from).or_else(|| bn.get(&edge.from))
                } else {
                    None
                }
            });
            let to = inventory.get(&edge.to);
            let valid = from
                .zip(to)
                .is_some_and(|(f, t)| valid_types(edge.kind, f.kind, t.kind));
            if let Some(issue) = finding_origin::problem(from.is_some(), to.is_some(), valid) {
                let message = match issue {
                    finding_origin::EdgeProblem::MissingSource => {
                        format!("missing source node {}", edge.from)
                    }
                    finding_origin::EdgeProblem::MissingTarget => {
                        format!("missing target node {}", edge.to)
                    }
                    finding_origin::EdgeProblem::InvalidType => {
                        "invalid typed edge endpoints".to_owned()
                    }
                };
                let problem = EdgeProblem {
                    edge: edge.clone(),
                    endpoint: endpoint.into(),
                    message: message.clone(),
                };
                let reached = finding_origin::reached_edge(
                    closure.reached.get(&edge.from).map(|s| (&edge.from, s)),
                    closure.reached.get(&edge.to).map(|s| (&edge.to, s)),
                );
                if let Some(((node, causes), append_target)) = reached {
                    let cause = causes.first().unwrap();
                    let seed = verified_graph::seed_by_id(&seeds, cause).unwrap();
                    let path = closure.witness(cause, node).unwrap();
                    let witness = finding_origin::witness(
                        &seed.node,
                        &path,
                        append_target.then_some(&edge.to),
                    );
                    findings.push(Finding {
                        code: issue.code().into(),
                        level: "error".into(),
                        message: format!("{endpoint}: {message}: {edge:?}"),
                        delta_refs: vec![finding_reference(seed, endpoint)],
                        causes: witness,
                    });
                } else {
                    historical_context.push(problem);
                }
            }
        }
    }
    let selection = verified_graph::report_tests(&closure.traversed, &CandidateTests(bn));
    let mut judges: Vec<_> = candidate.judges()["judges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| {
            let node = format!("judge:{}", j["id"].as_str().unwrap());
            JudgeSelection {
                triggers: verified_graph::target_edges(
                    &closure.traversed,
                    EdgeKind::JudgeTrigger,
                    &node,
                ),
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
            tests: selection.current,
            retired_tests: selection.retired,
            required_tests: selection.required,
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
#[cfg(not(verus_keep_ghost))]
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
    if let Some(paths) = verified_graph::scheduling_paths(query.scheduling, &delta) {
        result["reached"] = json!(impact.closure.reached.keys().collect::<Vec<_>>());
        result["changed_paths"] = json!(paths);
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
#[cfg(not(verus_keep_ghost))]
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
    if !request::collection_request_matches(
        req.candidate.root == root,
        req.scope.as_ref().and_then(|s| s["kind"].as_str()),
    ) {
        return Err("FILEMAP collection requires this checkout and a collection scope".into());
    }
    let mut reader = runtime::Reader::open(root, json!({"mode":"request","request":req.raw}))?;
    verify_request_endpoints(req, &mut reader)?;
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

#[cfg(not(verus_keep_ghost))]
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
#[cfg(not(verus_keep_ghost))]
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
        let observation = reader
            .observation()
            .and_then(|local| registration::with_registration_facts(local, position, remote));
        let (facts, result) = request::finish_evaluation(observation, reader.close(), result);
        if let Some(facts) = facts.filter(|facts| !facts.is_null()) {
            outputs.insert("git_facts".into(), facts);
        }
        result
    })();
    let (status, findings) = match result {
        Ok((impact, findings, view)) => {
            let status = evaluated_status(findings.len());
            outputs.insert(
                "impact".into(),
                serde_json::to_value(impact).map_err(|e| e.to_string())?,
            );
            outputs.insert("registration_view".into(), view);
            (status, findings)
        }
        Err(message) => {
            let code =
                request::evaluation_code(message.split(':').next().unwrap_or_default()).to_owned();
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
#[cfg(not(verus_keep_ghost))]
fn verify_request_endpoints(req: &Request, reader: &mut runtime::Reader) -> Result<(), String> {
    verified_graph::admit_tree(&req.base.tree, &reader.verify_tree("base")?)?;
    verified_graph::admit_tree(&req.candidate.tree, &reader.verify_tree("candidate")?)?;
    verified_graph::admit_delta(&req.delta, &reader.delta()?)?;
    Ok(())
}

#[cfg(not(verus_keep_ghost))]
fn evaluate(
    req: &Request,
    reader: &mut runtime::Reader,
    remote_facts: &mut Option<(usize, Option<Value>)>,
) -> Result<(Impact, Vec<Finding>, Value), String> {
    let root = &req.candidate.root;
    verify_request_endpoints(req, reader)?;
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
    let view = registration::required_registration_view(views.registration_view.take())?;
    let (impact, findings) = produce_for_request(req, &views, reader)?;
    Ok((impact, findings, view))
}
