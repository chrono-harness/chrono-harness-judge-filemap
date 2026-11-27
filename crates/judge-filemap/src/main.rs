#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg(verus_keep_ghost)]
use super::equal_text;
#[cfg(not(verus_keep_ghost))]
use chrono_judge_filemap::verified_graph::equal_text;

verus! {
pub enum Operation { Version, CostData, Evaluate, Summary, Endpoint, Closure, Collection, Prepare, Judge }

/// Unknown and non-UTF-8 words are passed to the public judge. Version accepts a sole argument.
pub fn operation(first: Option<&str>, argc: usize) -> (op: Operation)
    ensures op == match first {
        None => Operation::Judge,
        Some(name) => if argc == 1 && name@ == "--version"@ { Operation::Version } else if name@ == "cost-data-v1"@ { Operation::CostData } else if name@ == "evaluate-data-v1"@ { Operation::Evaluate } else if name@ == "impact-summary"@ { Operation::Summary } else if name@ == "endpoint-tests"@ { Operation::Endpoint } else if name@ == "closure-v1"@ { Operation::Closure } else if name@ == "collection-tests-v1"@ { Operation::Collection } else if name@ == "cost-inputs-v1"@ { Operation::Prepare } else { Operation::Judge },
    },
{
    let name = match first { Some(name) => name, None => return Operation::Judge };
    if argc == 1 && equal_text(name, "--version") { Operation::Version }
    else if equal_text(name, "cost-data-v1") { Operation::CostData }
    else if equal_text(name, "evaluate-data-v1") { Operation::Evaluate }
    else if equal_text(name, "impact-summary") { Operation::Summary }
    else if equal_text(name, "endpoint-tests") { Operation::Endpoint }
    else if equal_text(name, "closure-v1") { Operation::Closure }
    else if equal_text(name, "collection-tests-v1") { Operation::Collection }
    else if equal_text(name, "cost-inputs-v1") { Operation::Prepare }
    else { Operation::Judge }
}
}

#[cfg(not(verus_keep_ghost))]
use std::io::Write;

#[cfg(not(verus_keep_ghost))]
fn main() -> std::process::ExitCode {
    use std::os::unix::process::CommandExt;
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let name = arguments.first().and_then(|name| name.to_str());
    let op = operation(name, arguments.len());
    if matches!(op, Operation::Version) {
        println!("chrono-judge-filemap {}", env!("CARGO_PKG_VERSION"));
        return 0.into();
    }
    if !matches!(op, Operation::Judge | Operation::Prepare) {
        if arguments.len() != 1 {
            if matches!(op, Operation::Summary) {
                eprintln!("E_USAGE: impact-summary reads one FILEMAP impact from stdin");
            } else {
                eprintln!("E_USAGE: {} reads one query from stdin", name.unwrap());
            }
            return 2.into();
        }
        let input = std::io::stdin().lock();
        let value = match op {
            Operation::CostData => chrono_judge_filemap::cost_inputs::prepare(input),
            Operation::Evaluate => chrono_judge_filemap::decision(input),
            Operation::Summary => {
                chrono_judge_filemap::impact_summary(input).map_err(|e| e.to_string())
            }
            Operation::Closure => chrono_judge_filemap::graph::query_closure(input),
            Operation::Collection => std::env::current_dir()
                .map_err(|e| e.to_string())
                .and_then(|root| chrono_judge_filemap::collection_tests(&root, input)),
            Operation::Endpoint => std::env::current_dir()
                .map_err(|e| e.to_string())
                .and_then(|root| chrono_judge_filemap::endpoint_tests(&root, input)),
            Operation::Judge | Operation::Prepare | Operation::Version => unreachable!(),
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
    if matches!(op, Operation::Prepare) {
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
