mod args;
mod commands;
mod echo;

use clap::Parser;

fn main() -> std::process::ExitCode {
    let args = args::Args::parse();
    commands::run(args.command)
}
