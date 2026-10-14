//! Cardfile entry point: command line, logging and startup.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

use cardfile_ui::DataPaths;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Default)]
struct Args {
    portable: bool,
    data_dir: Option<PathBuf>,
}

const HELP: &str = "Cardfile - a modern digital card file

Usage: cardfile [OPTIONS]

Options:
      --portable         Store data in a `data` folder next to the executable
      --data-dir <PATH>  Store data in PATH
  -V, --version          Print version
  -h, --help             Print help
";

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--portable" => args.portable = true,
            "--data-dir" => {
                args.data_dir = Some(it.next().ok_or("--data-dir needs a path")?.into());
            }
            "-V" | "--version" => {
                println!("cardfile {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(args)
}

fn init_logging(paths: &DataPaths) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_env("CARDFILE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu=warn,naga=warn"));
    let dir = paths.logs();
    if std::fs::create_dir_all(&dir).is_err() {
        tracing_subscriber::fmt().with_env_filter(filter).init();
        return None;
    }
    let appender = tracing_appender::rolling::Builder::new()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("cardfile")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .init();
    Some(guard)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{HELP}");
            std::process::exit(2);
        }
    };
    let paths = DataPaths::resolve(args.data_dir, args.portable);
    let _guard = init_logging(&paths);
    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
        eprintln!("{info}");
    }));
    tracing::info!(version = env!("CARGO_PKG_VERSION"), data = %paths.root.display(), portable = paths.portable, "starting Cardfile");
    cardfile_ui::run(paths);
}
