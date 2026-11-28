//! The fields used by FILEMAP after the runner decodes the complete sealed wire value.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use super::compare_bytes;
#[cfg(verus_keep_ghost)]
use super::equal_text;
#[cfg(not(verus_keep_ghost))]
use crate::verified_graph::{compare_bytes, equal_text};
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;

verus! {
/// A bound reply needs the original string; an unbound reply omits the member entirely.
pub fn request_id_matches(expected: Option<&str>, observed: Option<Option<&str>>) -> (same: bool)
    ensures same == match (expected, observed) {
        (None, None) => true,
        (Some(a), Some(Some(b))) => a.spec_bytes() == b.spec_bytes(),
        _ => false,
    },
{
    match (expected, observed) {
        (None, None) => true,
        (Some(a), Some(Some(b))) => compare_bytes(a.as_bytes(), b.as_bytes()) == 0,
        _ => false,
    }
}

pub fn collection_request_matches(checkout_matches: bool, scope: Option<&str>) -> (admitted: bool)
    ensures admitted == (checkout_matches && match scope {
        Some(kind) => kind@ == "collect"@,
        None => false,
    }),
{
    checkout_matches && match scope {
        Some(kind) => equal_text(kind, "collect"),
        None => false,
    }
}

/// The observation and close must both succeed before the evaluation is usable.
/// Keep a successful observation even when close or the evaluation fails.
pub fn finish_evaluation<T, O, E>(
    observation: Result<O, E>, close: Result<(), E>, evaluation: Result<T, E>,
) -> (result: (Option<O>, Result<T, E>))
    ensures result == (match observation {
        Err(error) => (None, Err(error)),
        Ok(facts) => (Some(facts), match close {
            Err(error) => Err(error),
            Ok(()) => evaluation,
        }),
    }),
{
    match observation {
        Err(error) => (None, Err(error)),
        Ok(facts) => (Some(facts), match close {
            Err(error) => Err(error),
            Ok(()) => evaluation,
        }),
    }
}

pub fn evaluation_code(field: &str) -> (code: &str)
    ensures code@ == if field.spec_bytes().len() >= "E_SEMANTIC_".spec_bytes().len()
        && field.spec_bytes().take("E_SEMANTIC_".spec_bytes().len() as int) == "E_SEMANTIC_".spec_bytes()
        { field@ } else { "E_INPUT"@ },
{
    let bytes = field.as_bytes();
    let prefix = "E_SEMANTIC_".as_bytes();
    if bytes.len() < prefix.len() {
        return "E_INPUT";
    }
    for i in 0..prefix.len()
        invariant bytes.len() >= prefix.len(),
            bytes@ == field.spec_bytes(), prefix@ == "E_SEMANTIC_".spec_bytes(),
            forall|j: int| 0 <= j < i ==> #[trigger] bytes@[j] == prefix@[j],
    {
        if bytes[i] != prefix[i] {
            assert(bytes@.take(prefix.len() as int)[i as int] != prefix@[i as int]);
            assert(bytes@.take(prefix.len() as int) != prefix@);
            return "E_INPUT";
        }
    }
    assert(bytes@.take(prefix.len() as int) =~= prefix@);
    field
}
}
#[cfg(not(verus_keep_ghost))]
use crate::Delta;
#[cfg(not(verus_keep_ghost))]
use serde::Deserialize;
#[cfg(not(verus_keep_ghost))]
use serde_json::Value;
#[cfg(not(verus_keep_ghost))]
use std::path::PathBuf;

#[cfg(not(verus_keep_ghost))]
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub level: String,
    pub message: String,
    pub delta_refs: Vec<String>,
    pub causes: Vec<String>,
}
#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
pub(crate) struct Endpoint {
    pub commit: String,
    pub tree: String,
    pub root: PathBuf,
}
#[cfg(not(verus_keep_ghost))]
#[derive(Deserialize)]
pub(crate) struct Fields {
    pub request_id: String,
    pub base: Endpoint,
    pub candidate: Endpoint,
    pub delta: Vec<Delta>,
    pub observations: Value,
    pub scope: Option<Value>,
}
#[cfg(not(verus_keep_ghost))]
pub(crate) struct Request {
    pub raw: Value,
    pub fields: Fields,
}
#[cfg(not(verus_keep_ghost))]
impl Request {
    pub fn new(raw: Value) -> Result<Self, String> {
        let fields = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
        Ok(Self { raw, fields })
    }
}
#[cfg(not(verus_keep_ghost))]
impl std::ops::Deref for Request {
    type Target = Fields;
    fn deref(&self) -> &Fields {
        &self.fields
    }
}
