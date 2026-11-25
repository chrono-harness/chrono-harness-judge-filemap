use super::verus;
#[cfg(not(verus_keep_ghost))]
use serde_json::Value;
#[cfg(not(verus_keep_ghost))]
use std::collections::BTreeMap;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {
/// A recorded PATH replaces the launcher search path, including an explicit
/// absence. Only an unobserved PATH inherits the launcher's value.
pub fn effective_child_path<T>(
    search_path: Option<T>,
    recorded: Option<Option<T>>,
) -> (effective: Option<T>)
    ensures
        effective == match recorded {
            None => search_path,
            Some(observed) => observed,
        },
{
    match recorded {
        Some(value) => value,
        None => search_path,
    }
}
}

/// Scheduling opens a new Git reader with the values the parent observed.
#[cfg(not(verus_keep_ghost))]
pub(crate) fn child_environment(
    search_path: Option<String>,
    inherited: Option<&Value>,
) -> Result<BTreeMap<String, String>, String> {
    let mut values: BTreeMap<String, Option<String>> = inherited
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let path = effective_child_path(search_path, values.remove("PATH"));
    let mut environment: BTreeMap<_, _> = values
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name, value)))
        .collect();
    if let Some(path) = path {
        environment.insert("PATH".to_owned(), path);
    }
    Ok(environment)
}
