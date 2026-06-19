use std::fmt::Display;

use crate::source::Span;

/// A stable diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    Unsupported,
    ProjectError,
    ParseError,
    AstError,
    ResolveError,
    TypeError,
    LoweringError,
    WasmError,
}

/// A source location attached to a diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

impl Label {
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self { span, message: Some(message.into()) }
    }
}

/// A compiler diagnostic that can be rendered by the CLI or tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), labels: Vec::new(), notes: Vec::new() }
    }

    pub fn spanned(code: DiagnosticCode, message: impl Into<String>, span: Span, label: impl Into<String>) -> Self {
        Self::new(code, message).with_label(Label::primary(span, label))
    }

    pub fn expected_found(
        code: DiagnosticCode, got: impl Display, want: impl Display, span: Span, label: impl Into<String>,
    ) -> Self {
        Self::spanned(code, format!("expected `{got}` but found `{want}`"), span, label)
    }

    pub fn duplicate(
        code: DiagnosticCode, message: impl Into<String>, curr_span: Span, curr_label: impl Into<String>,
        prev_span: Span,
    ) -> Self {
        Self::spanned(code, message, curr_span, curr_label)
            .with_label(Label::primary(prev_span, "previously defined here"))
    }

    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_optional_label(mut self, label: Option<Label>) -> Self {
        if let Some(label) = label {
            self.labels.push(label);
        }
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_optional_note(mut self, note: Option<impl Into<String>>) -> Self {
        if let Some(note) = note {
            self.notes.push(note.into());
        }
        self
    }

    pub fn with_notes<I, N>(mut self, notes: I) -> Self
    where
        I: IntoIterator<Item = N>,
        N: Into<String>,
    {
        self.notes.extend(notes.into_iter().map(Into::into));
        self
    }

    pub fn unsupported(phase: &str, span: Span) -> Self {
        Self::new(
            DiagnosticCode::Unsupported,
            format!("{phase} is not implemented for this input yet"),
        )
        .with_label(Label::primary(span, "unsupported here"))
    }

    pub fn render_plain(&self) -> String {
        let mut rendered = format!("{:?}: {}", self.code, self.message);
        for label in &self.labels {
            rendered.push_str(&format!(
                "\n  --> file {} bytes {}..{}",
                label.span.file_id.0, label.span.start, label.span.end
            ));
            if let Some(message) = &label.message {
                rendered.push_str(&format!("\n      {message}"));
            }
        }
        for note in &self.notes {
            rendered.push_str(&format!("\n  note: {note}"));
        }
        rendered
    }
}

pub type Diagnostics = Vec<Diagnostic>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFileId, Span};

    #[test]
    fn renders_plain_diagnostics_without_terminal_colors() {
        let diagnostic = Diagnostic::new(DiagnosticCode::TypeError, "expected `Int` but found `String`")
            .with_label(Label::primary(Span::new(SourceFileId(0), 10, 18), "type mismatch"))
            .with_note("check the function argument");

        insta::assert_snapshot!(diagnostic.render_plain(), @r#"
TypeError: expected `Int` but found `String`
  --> file 0 bytes 10..18
      type mismatch
  note: check the function argument
"#);
    }

    #[test]
    fn builds_common_spanned_diagnostics() {
        let span = Span::new(SourceFileId(0), 10, 18);

        let diagnostic = Diagnostic::expected_found(DiagnosticCode::TypeError, "Int", "String", span, "type mismatch")
            .with_optional_note(Some("check the function argument"));

        insta::assert_snapshot!(diagnostic.render_plain(), @r#"
TypeError: expected `Int` but found `String`
  --> file 0 bytes 10..18
      type mismatch
  note: check the function argument
"#);
    }

    #[test]
    fn builds_duplicate_diagnostics() {
        let diagnostic = Diagnostic::duplicate(
            DiagnosticCode::ResolveError,
            "duplicate name `main`",
            Span::new(SourceFileId(0), 20, 24),
            "defined again here",
            Span::new(SourceFileId(0), 4, 8),
        );

        insta::assert_snapshot!(diagnostic.render_plain(), @r#"
ResolveError: duplicate name `main`
  --> file 0 bytes 20..24
      defined again here
  --> file 0 bytes 4..8
      previously defined here
"#);
    }
}
