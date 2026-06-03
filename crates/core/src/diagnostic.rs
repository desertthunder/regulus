use crate::source::Span;

/// A stable diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    Unsupported,
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

    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
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
    use crate::source::{SourceFileId, Span};

    use super::*;

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
}
