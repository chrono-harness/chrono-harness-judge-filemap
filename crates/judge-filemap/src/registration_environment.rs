use serde_json::Value;
use std::collections::BTreeMap;

/// Scheduling opens a new Git reader. Give it the inherited inputs the parent
/// observed. Its own configuration supplies literals and excludes credentials.
pub(crate) fn child_environment(
    search_path: Option<String>,
    inherited: Option<&Value>,
) -> Result<BTreeMap<String, String>, String> {
    let mut env: BTreeMap<_, _> = search_path
        .into_iter()
        .map(|p| ("PATH".to_owned(), p))
        .collect();
    if let Some(inherited) = inherited {
        let values: BTreeMap<String, Option<String>> =
            serde_json::from_value(inherited.clone()).map_err(|e| e.to_string())?;
        for (name, value) in values {
            if let Some(value) = value {
                env.insert(name, value);
            } else {
                env.remove(&name);
            }
        }
    }
    Ok(env)
}
