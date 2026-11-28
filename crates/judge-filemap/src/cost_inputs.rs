//! Prepare the FILEMAP input and ordered registration projections for the cost reporter.
#![allow(unexpected_cfgs)]
use vstd::prelude::*;

verus! {
/// The JSON adapter observes an optional prior impact and compares it with the full value.
pub trait PriorImpact: Sized {
    spec fn comparison(&self, expected: &Self) -> Option<bool>;

    fn compare_impact(&self, expected: &Self) -> (result: Option<bool>)
        ensures result == self.comparison(expected);
}

/// At least one producer must supply the expected impact. Any conflicting producer rejects it.
pub fn prior_impacts_match<R: PriorImpact>(rows: &[R], expected: &R) -> (accepted: bool)
    ensures accepted == (
        (exists|i: int| 0 <= i < rows.len() && (#[trigger] rows@[i].comparison(expected)) == Some(true))
        && (forall|i: int| 0 <= i < rows.len() ==> (#[trigger] rows@[i].comparison(expected)) != Some(false))),
{
    let mut found = false;
    let mut i = 0usize;
    while i < rows.len()
        invariant
            i <= rows.len(),
            found == (exists|j: int| 0 <= j < i && (#[trigger] rows@[j].comparison(expected)) == Some(true)),
            forall|j: int| 0 <= j < i ==> (#[trigger] rows@[j].comparison(expected)) != Some(false),
        decreases rows.len() - i,
    {
        match rows[i].compare_impact(expected) {
            Some(false) => return false,
            Some(true) => found = true,
            None => {},
        }
        i += 1;
    }
    found
}
}

#[cfg(not(verus_keep_ghost))]
pub use native::prepare;

#[cfg(not(verus_keep_ghost))]
mod native {
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
    fn prepare_request(
        req: &Request,
        protocol: &Value,
        outputs: &mut Value,
    ) -> Result<Value, String> {
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
        let mut reply = crate::strict_json::parse(&bytes)
            .map_err(|e| format!("invalid registration filemap views: {e}"))?;
        if !crate::request::request_id_matches(
            Some(&req.request_id),
            reply.get("request_id").map(Value::as_str),
        ) {
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
        let parsed: Impact = serde_json::from_value(req.raw["impact"].clone())
            .map_err(|e| format!("E_IMPACT: {e}"))?;
        let prior = req.raw["prior_results"]
            .as_array()
            .ok_or("missing prior results")?;
        if !super::prior_impacts_match(prior, &req.raw["impact"])
            || crate::verified_graph::admit_delta(&req.delta, &parsed.delta).is_err()
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

    impl super::PriorImpact for Value {
        fn compare_impact(&self, expected: &Self) -> Option<bool> {
            self["outputs"]
                .get("impact")
                .map(|impact| impact == expected)
        }
    }
}
