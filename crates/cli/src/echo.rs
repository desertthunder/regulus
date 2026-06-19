use std::{fmt::Display, process::ExitCode};

use compiler_core::diagnostic::{Diagnostic, Diagnostics};
use compiler_core::source::{SourceFile, Span};
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

pub fn fail_with_source_diagnostics(
    action: &str, subject: impl Display, items: &Diagnostics, sources: &[SourceFile],
) -> ExitCode {
    error(format!("could not {action} {subject}"));
    diagnostics_with_sources(items, sources);
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

pub fn diagnostics_with_sources(diagnostics: &Diagnostics, sources: &[SourceFile]) {
    let mut ordered = diagnostics.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|diagnostic| {
        diagnostic
            .labels
            .first()
            .map(|label| (label.span.file_id.0, label.span.start, label.span.end))
            .unwrap_or((u32::MAX, usize::MAX, usize::MAX))
    });
    for item in ordered {
        diagnostic(render_with_sources(item, sources));
    }
}

fn render_with_sources(diagnostic: &Diagnostic, sources: &[SourceFile]) -> String {
    let mut rendered = format!("{:?}: {}", diagnostic.code, diagnostic.message);
    for label in &diagnostic.labels {
        let Some(source) = sources.iter().find(|source| source.id == label.span.file_id) else {
            rendered.push_str(&format!(
                "\n  --> file {} bytes {}..{}",
                label.span.file_id.0, label.span.start, label.span.end
            ));
            if let Some(message) = &label.message {
                rendered.push_str(&format!("\n      {message}"));
            }
            continue;
        };
        let snippet = SourceSnippet::new(source, label.span);
        let path = source
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| format!("file {}", label.span.file_id.0));
        rendered.push_str(&format!("\n  --> {path}:{}:{}", snippet.line_number, snippet.column));
        rendered.push_str(&format!("\n{:>4} | {}", snippet.line_number, snippet.line));
        rendered.push_str(&format!(
            "\n     | {}{}",
            " ".repeat(snippet.caret_start),
            "^".repeat(snippet.caret_len)
        ));
        if let Some(message) = &label.message {
            rendered.push(' ');
            rendered.push_str(message);
        }
    }
    for note in &diagnostic.notes {
        rendered.push_str(&format!("\n  note: {note}"));
    }
    rendered
}

struct SourceSnippet {
    line_number: usize,
    column: usize,
    line: String,
    caret_start: usize,
    caret_len: usize,
}

impl SourceSnippet {
    fn new(source: &SourceFile, span: Span) -> Self {
        let start = floor_char_boundary(&source.text, span.start.min(source.text.len()));
        let end = ceil_char_boundary(&source.text, span.end.min(source.text.len())).max(start);
        let line_start = source.text[..start].rfind('\n').map_or(0, |index| index + 1);
        let line_end = source.text[end..]
            .find('\n')
            .map_or(source.text.len(), |index| end + index);
        let line_number = source.text[..line_start].bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = source.text[line_start..start].chars().count() + 1;
        let line = source.text[line_start..line_end].to_string();
        let caret_start = source.text[line_start..start].chars().count();
        let caret_end =
            if end > line_end { line.chars().count() } else { source.text[line_start..end].chars().count() };
        let caret_len = caret_end.saturating_sub(caret_start).max(1);
        Self { line_number, column, line, caret_start, caret_len }
    }
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}
