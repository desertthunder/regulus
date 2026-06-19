mod args;
mod commands;
mod echo;

use clap::Parser;

pub fn run() -> std::process::ExitCode {
    let args = args::Args::parse();
    let no_color = args.no_color || std::env::var_os("NO_COLOR").is_some();
    echo::set_color_enabled(!no_color);
    commands::run(args.command, no_color)
}
