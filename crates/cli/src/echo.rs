use std::{fmt::Display, process::ExitCode};

use compiler_core::diagnostic::Diagnostics;
use owo_colors::OwoColorize;

pub fn status(label: &str, message: impl AsRef<str>) {
    eprintln!("{} {}", label.bright_magenta().bold(), message.as_ref());
}

pub fn progress(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("{} {}", "error".bright_red().bold(), message.as_ref());
}

pub fn fail(action: &str, subject: impl Display, cause: impl Display) -> ExitCode {
    error(format!("could not {action} {subject}: {cause}"));
    ExitCode::FAILURE
}

pub fn fail_with_diagnostics(action: &str, subject: impl Display, items: &Diagnostics) -> ExitCode {
    error(format!("could not {action} {subject}"));
    diagnostics(items);
    ExitCode::FAILURE
}

pub fn diagnostic(message: impl AsRef<str>) {
    eprintln!("{} {}", "diagnostic".bright_yellow().bold(), message.as_ref());
}

pub fn diagnostics(diagnostics: &Diagnostics) {
    for item in diagnostics {
        diagnostic(item.render_plain());
    }
}
