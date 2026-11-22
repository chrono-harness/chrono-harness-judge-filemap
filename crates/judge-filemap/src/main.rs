use std::io::Write;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    let name = args.next();
    let query = name.as_deref().and_then(std::ffi::OsStr::to_str);
    if let Some(operation @ ("impact-summary" | "endpoint-tests")) = query {
        if args.next().is_some() {
            match operation {
                "impact-summary" => {
                    eprintln!("E_USAGE: impact-summary reads one FILEMAP impact from stdin")
                }
                _ => eprintln!("E_USAGE: endpoint-tests reads one endpoint query from stdin"),
            }
            return 2.into();
        }
        let input = std::io::stdin().lock();
        let value = match operation {
            "impact-summary" => {
                chrono_judge_filemap::impact_summary(input).map_err(|e| e.to_string())
            }
            _ => std::env::current_dir()
                .map_err(|e| e.to_string())
                .and_then(|root| chrono_judge_filemap::endpoint_tests(&root, input)),
        };
        let mut output = std::io::stdout().lock();
        let result = value.and_then(|value| {
            serde_json::to_writer(&mut output, &value).map_err(|e| e.to_string())?;
            output.flush().map_err(|e| e.to_string())
        });
        return match result {
            Ok(()) => 0.into(),
            Err(error) => {
                eprintln!("{error}");
                2.into()
            }
        };
    }
    std::process::ExitCode::from(chrono_harness::wire::judge_main(
        "chrono-judge-filemap",
        env!("CARGO_PKG_VERSION"),
        chrono_judge_filemap::judge,
    ))
}
