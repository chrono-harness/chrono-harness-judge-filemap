// Replies for the fixed DELTAs in these FILEMAP cases. The semantic judge
// owns evaluation of the rules; this suite exercises the graph consumer.
pub fn reply(paths: &[&str]) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Example {
        paths: Vec<String>,
        reply: Result<serde_json::Value, String>,
    }
    let examples: Vec<Example> =
        serde_json::from_str(include_str!("../fixtures/classifications.json")).unwrap();
    examples
        .into_iter()
        .find(|e| e.paths == paths)
        .expect("missing classifier response for FILEMAP case")
        .reply
}
