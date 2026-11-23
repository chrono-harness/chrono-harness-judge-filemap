//! Prepare the FILEMAP input and ordered registration projections for the cost reporter.
use crate::Impact;
use chrono_harness::{CommandSpec, wire::Request};
use serde_json::{Value, json};
use std::io::Read;

pub fn prepare(input: impl Read) -> Result<Value, String> {
    let bytes = {
        let mut raw = Vec::new();
        input
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)
            .map_err(|e| e.to_string())?;
        if raw.len() > 64 * 1024 * 1024 {
            return Err("E_PROTOCOL: request read/bound".into());
        }
        raw
    };
    let request: Request = chrono_harness::decode(&bytes)?;
    let mut outputs = json!({});
    let result = prepare_request(&request, &bytes, &mut outputs);
    Ok(json!({"request_id":request.request_id, "outputs":outputs,"result":result}))
}

fn prepare_request(req: &Request, original: &[u8], outputs: &mut Value) -> Result<Value, String> {
    req.validate()?;
    let config = chrono_harness::load_selected_config(&req.candidate.root, &req.config_path)?;
    let protocol = &config["protocol"];
    let command = CommandSpec {
        program: "chrono-judge-registration".into(),
        args: vec!["filemap-views".into()],
        env: serde_json::from_value(req.observations["environment"]["effective"].clone())
            .map_err(|e| e.to_string())?,
        timeout_seconds: protocol["timeout_seconds"]
            .as_u64()
            .ok_or("invalid registration timeout")?,
        output_limit_bytes: protocol["stdout_limit_bytes"]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or("invalid registration output bound")?,
    };
    let process = chrono_harness::run_process(&req.candidate.root, &command, original)?;
    if process.exit_code != 0 {
        let diag = std::str::from_utf8(&process.stderr_bytes).map_err(|e| e.to_string())?;
        return Err(if diag.is_empty() {
            format!(
                "registration filemap-views exited with {}",
                process.exit_code
            )
        } else {
            diag.strip_suffix('\n').unwrap_or(diag).into()
        });
    }
    let mut reply: Value = serde_json::from_slice(&process.stdout_bytes)
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
        serde_json::from_value(req.impact.clone()).map_err(|e| format!("E_IMPACT: {e}"))?;
    let mut sources = req
        .prior_results
        .iter()
        .filter_map(|r| r.outputs.get("impact"))
        .peekable();
    if sources.peek().is_none() || sources.any(|r| r != &req.impact) || parsed.delta != req.delta {
        return Err("E_IMPACT: missing or conflicting bound FILEMAP output".into());
    }
    let impact = crate::summarize(parsed);
    Ok(
        json!({"base":reply["base"],"candidate":reply["candidate"],"impact":impact,"binding":{
            "base":req.base.commit,"candidate":req.candidate.commit,"candidate_tree":req.candidate.tree,
            "registry_digest":req.registries.digest,"context_digest":req.context.sha256
        }}),
    )
}
