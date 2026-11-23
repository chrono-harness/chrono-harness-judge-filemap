//! The fields used by FILEMAP after the runner decodes the complete sealed wire value.
use crate::Delta;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub level: String,
    pub message: String,
    pub delta_refs: Vec<String>,
    pub causes: Vec<String>,
}
#[derive(Deserialize)]
pub(crate) struct Endpoint {
    pub commit: String,
    pub tree: String,
    pub root: PathBuf,
}
#[derive(Deserialize)]
pub(crate) struct Fields {
    pub request_id: String,
    pub base: Endpoint,
    pub candidate: Endpoint,
    pub delta: Vec<Delta>,
    pub observations: Value,
    pub scope: Option<Value>,
}
pub(crate) struct Request {
    pub raw: Value,
    pub fields: Fields,
}
impl Request {
    pub fn new(raw: Value) -> Result<Self, String> {
        let fields = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
        Ok(Self { raw, fields })
    }
}
impl std::ops::Deref for Request {
    type Target = Fields;
    fn deref(&self) -> &Fields {
        &self.fields
    }
}
