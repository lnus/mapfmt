use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

/// Reindent MapServer Mapfiles. Without FILEs, reads stdin and writes stdout.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Files to reindent in place.
    files: Vec<PathBuf>,

    /// Don't write anything; list files that would change and exit with 1.
    #[arg(long)]
    check: bool,

    /// Reindent files with parse errors too, instead of skipping them. The
    /// error is reported as a warning; indentation near it may be off.
    #[arg(long)]
    lenient: bool,

    /// Spaces per indent level.
    #[arg(long, default_value_t = 4)]
    indent: usize,
}

fn main() -> ExitCode {
    let args = Args::parse();
    if args.files.is_empty() {
        return stdin(&args);
    }

    let mut status = ExitCode::SUCCESS;
    for path in &args.files {
        let name = path.display().to_string();
        let result = std::fs::read(path)
            .map_err(|e| format!("{name}: {e}"))
            .and_then(|source| {
                let formatted = reindent(&name, &source, &args)?;
                let changed = formatted != source;
                if changed && !args.check {
                    std::fs::write(path, formatted).map_err(|e| format!("{name}: {e}"))?;
                }
                Ok(changed)
            });
        match result {
            Ok(true) if args.check => {
                println!("{name}");
                if status == ExitCode::SUCCESS {
                    status = ExitCode::from(1);
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("mapfmt: {e}");
                status = ExitCode::from(2);
            }
        }
    }
    status
}

fn stdin(args: &Args) -> ExitCode {
    let mut source = Vec::new();
    if let Err(e) = io::stdin().read_to_end(&mut source) {
        eprintln!("mapfmt: {e}");
        return ExitCode::from(2);
    }
    match reindent("<stdin>", &source, args) {
        Ok(formatted) if args.check => {
            if formatted == source {
                ExitCode::SUCCESS
            } else {
                println!("<stdin>");
                ExitCode::from(1)
            }
        }
        Ok(formatted) => {
            if let Err(e) = io::stdout().write_all(&formatted) {
                eprintln!("mapfmt: {e}");
                return ExitCode::from(2);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("mapfmt: {e}");
            ExitCode::from(2)
        }
    }
}

/// Formats `source`. A parse error fails unless `--lenient` is set, in which
/// case it's only a warning.
fn reindent(name: &str, source: &[u8], args: &Args) -> Result<Vec<u8>, String> {
    let formatted = mapfmt::format(source, args.indent);
    match formatted.error {
        Some(e) if !args.lenient => Err(format!("{name}:{e}")),
        Some(e) => {
            eprintln!("mapfmt: {name}:{e} (reindented anyway)");
            Ok(formatted.output)
        }
        None => Ok(formatted.output),
    }
}
