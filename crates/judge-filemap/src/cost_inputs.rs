//! Prepare the FILEMAP input and ordered registration projections for the cost reporter.
use crate::Impact;
use crate::{request::Request, runtime::Program};
use serde_json::{Value, json};
use std::io::Read;

pub fn prepare(input: impl Read) -> Result<Value, String> {
    let envelope: Value = serde_json::from_reader(input).map_err(|e| e.to_string())?;
    let req = Request::new(envelope["request"].clone())?;
    let mut outputs = json!({});
    let result = prepare_request(&req, &envelope["protocol"], &mut outputs);
    Ok(json!({"request_id":req.request_id,"outputs":outputs,"result":result}))
}
fn prepare_request(req: &Request, protocol: &Value, outputs: &mut Value) -> Result<Value, String> {
    let program = Program::resolve(
        &req.candidate.root,
        "chrono-judge-registration",
        req.observations["environment"]["effective"]["PATH"].as_str(),
    )?;
    let env = serde_json::from_value(req.observations["environment"]["effective"].clone())
        .map_err(|e| e.to_string())?;
    let original = serde_json::to_vec(&req.raw).map_err(|e| e.to_string())?;
    let bytes = program.run(
        &req.candidate.root,
        "filemap-views",
        &env,
        protocol,
        &original,
    )?;
    let mut reply: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("invalid registration filemap views: {e}"))?;
    if reply["request_id"] != req.request_id {
        return Err("E_PROTOCOL: registration query identity".into());
    }
    if let Some(facts) = reply
        .as_object_mut()
        .and_then(|obj| obj.remove("git_facts"))
    {
        outputs["git_facts"] = facts;
    }
    if let Some(error) = reply.get("error") {
        return Err(error.as_str().ok_or("invalid registration error")?.into());
    }
    let parsed: Impact =
        serde_json::from_value(req.raw["impact"].clone()).map_err(|e| format!("E_IMPACT: {e}"))?;
    let mut sources = req.raw["prior_results"]
        .as_array()
        .ok_or("missing prior results")?
        .iter()
        .filter_map(|r| r["outputs"].get("impact"))
        .peekable();
    if sources.peek().is_none()
        || sources.any(|r| r != &req.raw["impact"])
        || parsed.delta != req.delta
    {
        return Err("E_IMPACT: missing or conflicting bound FILEMAP output".into());
    }
    let impact = crate::summarize(parsed);
    Ok(
        json!({"base":reply["base"],"candidate":reply["candidate"],"impact":impact,"binding":{
            "base":req.base.commit,"candidate":req.candidate.commit,"candidate_tree":req.candidate.tree,
            "registry_digest":req.raw["registries"]["digest"],"context_digest":req.raw["context"]["sha256"]
        }}),
    )
}
