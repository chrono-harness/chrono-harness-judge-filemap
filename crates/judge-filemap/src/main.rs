use std::io::Write;

fn main() -> std::process::ExitCode {
    use std::os::unix::process::CommandExt;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments == [std::ffi::OsStr::new("--version")] {
        println!("chrono-judge-filemap {}", env!("CARGO_PKG_VERSION"));
        return 0.into();
    }
    let mut args = arguments.iter();
    let name = args.next();
    let query = name.and_then(|name| name.to_str());
    if let Some(
        operation @ ("cost-data-v1"
        | "evaluate-data-v1"
        | "impact-summary"
        | "endpoint-tests"
        | "closure-v1"
        | "collection-tests-v1"),
    ) = query
    {
        if args.next().is_some() {
            match operation {
                "impact-summary" => {
                    eprintln!("E_USAGE: impact-summary reads one FILEMAP impact from stdin")
                }
                _ => eprintln!("E_USAGE: {operation} reads one query from stdin"),
            }
            return 2.into();
        }
        let input = std::io::stdin().lock();
        let value = match operation {
            "cost-data-v1" => chrono_judge_filemap::cost_inputs::prepare(input),
            "evaluate-data-v1" => chrono_judge_filemap::decision(input),
            "impact-summary" => {
                chrono_judge_filemap::impact_summary(input).map_err(|e| e.to_string())
            }
            "closure-v1" => chrono_judge_filemap::graph::query_closure(input),
            "collection-tests-v1" => std::env::current_dir()
                .map_err(|e| e.to_string())
                .and_then(|root| chrono_judge_filemap::collection_tests(&root, input)),
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
    let current = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{e}");
            return 2.into();
        }
    };
    let mut command = std::process::Command::new("chrono-harness");
    if query == Some("cost-inputs-v1") {
        if arguments.len() != 1 {
            eprintln!("E_USAGE: cost-inputs-v1 reads one query from stdin");
            return 2.into();
        }
        command
            .arg("prepare-command-v1")
            .arg(current)
            .arg("cost-data-v1");
    } else {
        command
            .args([
                "judge-command-v1",
                "chrono-judge-filemap",
                env!("CARGO_PKG_VERSION"),
            ])
            .arg(current)
            .args(["evaluate-data-v1", "E_INPUT"])
            .args(arguments);
    }
    let error = command.exec();
    eprintln!("chrono-harness: {error}");
    2.into()
}
