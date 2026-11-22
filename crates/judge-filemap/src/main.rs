use std::io::Write;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() == Some(std::ffi::OsStr::new("impact-summary")) {
        if args.next().is_some() {
            eprintln!("E_USAGE: impact-summary reads one FILEMAP impact from stdin");
            return 2.into();
        }
        let mut output = std::io::stdout().lock();
        let result = chrono_judge_filemap::impact_summary(std::io::stdin().lock())
            .and_then(|summary| serde_json::to_writer(&mut output, &summary))
            .and_then(|()| output.flush().map_err(serde_json::Error::io));
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
