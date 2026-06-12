mod args;
mod commands;
mod echo;

use clap::Parser;

pub fn run() -> std::process::ExitCode {
    let args = args::Args::parse();
    commands::run(args.command)
}
