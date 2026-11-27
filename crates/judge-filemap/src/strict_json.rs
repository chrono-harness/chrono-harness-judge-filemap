//! Public tool replies must preserve every object member unambiguously.
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// The JSON decoder supplies a decoded member name and the original location, or its full error.
pub fn diagnostic(original: &str, duplicate: Option<(&str, &str)>) -> (out: String)
    ensures out@ == match duplicate {
        Some((key, location)) => "duplicate JSON member "@ + key@ + " at line "@ + location@,
        None => original@,
    },
{
    match duplicate {
        None => original.to_owned(),
        Some((key, location)) => {
            use vstd::string::StringExecFns;
            "duplicate JSON member ".to_owned().concat(key).concat(" at line ").concat(location)
        }
    }
}
}
#[cfg(not(verus_keep_ghost))]
pub(crate) fn parse(bytes: &[u8]) -> Result<serde_json::Value, String> {
    let value: serde_yaml_ng::Value =
        serde_json::from_slice(bytes).map_err(|error: serde_json::Error| {
            let message = error.to_string();
            if let Some((quoted, location)) = message
                .strip_prefix("duplicate entry with key ")
                .and_then(|tail| tail.rsplit_once(" at line "))
            {
                if let Ok(key) = litrs::StringLit::parse(quoted) {
                    return diagnostic(&message, Some((key.value(), location)));
                }
            }
            diagnostic(&message, None)
        })?;
    serde_json::to_value(value).map_err(|error| error.to_string())
}
