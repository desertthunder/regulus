use std::path::PathBuf;

/// Stable identifier for a source file known to the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceFileId(pub u32);

/// A byte span inside a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub file_id: SourceFileId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(file_id: SourceFileId, start: usize, end: usize) -> Self {
        Self { file_id, start, end }
    }
}

/// Source text plus the metadata needed to report diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub id: SourceFileId,
    pub path: Option<PathBuf>,
    pub text: String,
}

impl SourceFile {
    pub fn new(id: SourceFileId, text: impl Into<String>) -> Self {
        Self { id, path: None, text: text.into() }
    }

    pub fn with_path(id: SourceFileId, path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self { id, path: Some(path.into()), text: text.into() }
    }

    pub fn whole_span(&self) -> Span {
        Span::new(self.id, 0, self.text.len())
    }
}
