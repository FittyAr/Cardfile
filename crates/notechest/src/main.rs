//! NoteChest entry point: command line, logging and startup.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

// Only called on Linux; built everywhere so its tests run on every platform.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod shell_profiles;

use std::ffi::OsString;
use std::path::PathBuf;

use notechest_ui::DataPaths;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    portable: bool,
    data_dir: Option<PathBuf>,
}

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
enum Cli {
    Run(Args),
    Version,
    Help,
}

const HELP: &str = "NoteChest - a modern digital card file

Usage: notechest [OPTIONS]

Options:
      --portable         Store data in a `data` folder next to the executable
      --data-dir <PATH>  Store data in PATH
  -V, --version          Print version
  -h, --help             Print help
";

/// Parse the arguments after the program name. Works on `OsString`s so that a
/// non-UTF-8 argument (possible on Linux, and in Windows paths with unpaired
/// surrogates) is reported as an error, or kept as-is for `--data-dir`, instead of
/// panicking like `std::env::args` would.
fn parse_args(raw: impl IntoIterator<Item = OsString>) -> Result<Cli, String> {
    let mut args = Args::default();
    let mut it = raw.into_iter();
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--portable") => args.portable = true,
            Some("--data-dir") => {
                args.data_dir = Some(it.next().ok_or("--data-dir needs a path")?.into());
            }
            Some("-V" | "--version") => return Ok(Cli::Version),
            Some("-h" | "--help") => return Ok(Cli::Help),
            _ => {
                return Err(format!("unknown argument: {}", arg.to_string_lossy()));
            }
        }
    }
    Ok(Cli::Run(args))
}

fn init_logging(paths: &DataPaths) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_env("NOTECHEST_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu=warn,naga=warn"));
    let dir = paths.logs();
    if std::fs::create_dir_all(&dir).is_err() {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return None;
    }
    let appender = match tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("notechest")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
    {
        Ok(a) => a,
        Err(e) => {
            // Without a log file, still log to stderr instead of nowhere.
            tracing_subscriber::fmt().with_env_filter(filter).init();
            tracing::warn!("cannot create the log file in {}: {e}", dir.display());
            return None;
        }
    };
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Some(guard)
}

fn main() {
    let args = match parse_args(std::env::args_os().skip(1)) {
        Ok(Cli::Run(a)) => a,
        Ok(Cli::Version) => {
            println!("notechest {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        Ok(Cli::Help) => {
            print!("{HELP}");
            return;
        }
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            std::process::exit(2);
        }
    };
    let paths = DataPaths::resolve(args.data_dir, args.portable);
    let _guard = init_logging(&paths);
    // Log panics with a backtrace (always captured: a panic is rare and the log is
    // often the only report we get), then let the default hook print as usual.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(
            "panic: {info}
{backtrace}"
        );
        default_hook(info);
    }));
    tracing::info!(version = env!("CARGO_PKG_VERSION"), data = %paths.root.display(), portable = paths.portable, "starting NoteChest");
    #[cfg(target_os = "linux")]
    shell_profiles::repair();
    notechest_ui::run(paths);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_runs_with_defaults() {
        assert_eq!(parse(&[]), Ok(Cli::Run(Args::default())));
    }

    #[test]
    fn options() {
        assert_eq!(
            parse(&["--portable", "--data-dir", "/tmp/cards"]),
            Ok(Cli::Run(Args {
                portable: true,
                data_dir: Some(PathBuf::from("/tmp/cards")),
            }))
        );
        assert_eq!(parse(&["-V"]), Ok(Cli::Version));
        assert_eq!(parse(&["--portable", "--help"]), Ok(Cli::Help));
    }

    #[test]
    fn errors() {
        assert_eq!(
            parse(&["--data-dir"]),
            Err("--data-dir needs a path".to_owned())
        );
        assert_eq!(
            parse(&["--bogus"]),
            Err("unknown argument: --bogus".to_owned())
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_arguments_do_not_panic() {
        use std::os::unix::ffi::OsStringExt as _;
        let bad = OsString::from_vec(vec![b'-', b'-', 0xff]);
        assert!(
            parse_args([bad.clone()])
                .unwrap_err()
                .starts_with("unknown argument")
        );
        // A non-UTF-8 data folder is a valid path and is kept as is.
        let got = parse_args([OsString::from("--data-dir"), bad.clone()]).unwrap();
        assert_eq!(
            got,
            Cli::Run(Args {
                portable: false,
                data_dir: Some(PathBuf::from(bad)),
            })
        );
    }

    #[cfg(windows)]
    #[test]
    fn non_utf8_arguments_do_not_panic() {
        use std::os::windows::ffi::OsStringExt as _;
        // An unpaired surrogate is valid in a Windows argument but not in UTF-8.
        let bad = OsString::from_wide(&[u16::from(b'-'), 0xD800]);
        assert!(
            parse_args([bad.clone()])
                .unwrap_err()
                .starts_with("unknown argument")
        );
        let got = parse_args([OsString::from("--data-dir"), bad.clone()]).unwrap();
        assert_eq!(
            got,
            Cli::Run(Args {
                portable: false,
                data_dir: Some(PathBuf::from(bad)),
            })
        );
    }
}
