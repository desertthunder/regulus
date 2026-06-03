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
}

pub type Diagnostics = Vec<Diagnostic>;
