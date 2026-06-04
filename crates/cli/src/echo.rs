use compiler_core::diagnostic::Diagnostics;
use owo_colors::OwoColorize;

pub fn status(label: &str, message: impl AsRef<str>) {
    eprintln!("{} {}", label.bright_magenta().bold(), message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    eprintln!("{} {}", "error".bright_red().bold(), message.as_ref());
}

pub fn diagnostic(message: impl AsRef<str>) {
    eprintln!("{} {}", "diagnostic".bright_yellow().bold(), message.as_ref());
}

pub fn diagnostics(diagnostics: &Diagnostics) {
    for item in diagnostics {
        diagnostic(item.render_plain());
    }
}
