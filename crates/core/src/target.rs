use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::{ast, source::Span};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    #[default]
    Wasmtime,
    Browser,
    Wasi,
    Wasm,
}

impl CompileTarget {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wasmtime => "wasmtime",
            Self::Browser => "browser",
            Self::Wasi => "wasi",
            Self::Wasm => "wasm",
        }
    }
}

pub fn select_module(module: ast::Module, target: CompileTarget) -> Result<ast::Module, Diagnostics> {
    let mut selector = TargetSelector { target, diagnostics: Vec::new() };
    let declarations = selector.select_declarations(module.declarations);
    if !selector.diagnostics.is_empty() {
        return Err(selector.diagnostics);
    }
    let imports = declarations
        .iter()
        .filter_map(|declaration| match declaration {
            ast::Declaration::Import(import) => Some(import.clone()),
            _ => None,
        })
        .collect();
    let functions = declarations
        .iter()
        .filter_map(|declaration| match declaration {
            ast::Declaration::Function(function) => Some(function.clone()),
            _ => None,
        })
        .collect();
    Ok(ast::Module { span: module.span, declarations, imports, functions })
}

struct TargetSelector {
    target: CompileTarget,
    diagnostics: Diagnostics,
}

impl TargetSelector {
    fn select_declarations(&mut self, declarations: Vec<ast::Declaration>) -> Vec<ast::Declaration> {
        declarations
            .into_iter()
            .flat_map(|declaration| self.select_declaration(declaration))
            .collect()
    }

    fn select_declaration(&mut self, declaration: ast::Declaration) -> Vec<ast::Declaration> {
        match declaration {
            ast::Declaration::TargetGroup(group) => self.select_group(group),
            declaration => vec![declaration],
        }
    }

    fn select_group(&mut self, group: ast::TargetGroup) -> Vec<ast::Declaration> {
        let Some(group_target) = target_name(&group.target.text, group.target.span, &mut self.diagnostics) else {
            return Vec::new();
        };
        if group_target != self.target {
            return Vec::new();
        }
        self.select_declarations(group.declarations)
    }
}

fn target_name(name: &str, span: Span, diagnostics: &mut Diagnostics) -> Option<CompileTarget> {
    match name {
        "wasmtime" => Some(CompileTarget::Wasmtime),
        "browser" | "javascript" => Some(CompileTarget::Browser),
        "wasi" => Some(CompileTarget::Wasi),
        "wasm" => Some(CompileTarget::Wasm),
        "erlang" => None,
        _ => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("unsupported target group `{name}`"),
                )
                .with_label(Label::primary(span, "unsupported target here"))
                .with_note("supported targets are `wasmtime`, `browser`, `wasi`, and `wasm`"),
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFile, SourceFileId};
    use crate::{ast, parse, resolve, types};

    fn ast_for(source: &str) -> ast::Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        ast::build(&cst).expect("build ast")
    }

    #[test]
    fn selects_browser_target_group_declarations() {
        let module = select_module(
            ast_for(
                r#"
if javascript {
  pub fn browser_value() -> Int { 2 }
}

pub fn main() -> Int { 1 }
"#,
            ),
            CompileTarget::Browser,
        )
        .expect("select browser target");

        assert!(
            module
                .functions
                .iter()
                .any(|function| function.name.text == "browser_value")
        );
        assert!(module.functions.iter().any(|function| function.name.text == "main"));
    }

    #[test]
    fn excludes_non_selected_target_group_declarations_before_type_checking() {
        let module = select_module(
            ast_for(
                r#"
if javascript {
  pub fn browser_value() -> Unknown { missing }
}

pub fn main() -> Int { 1 }
"#,
            ),
            CompileTarget::Wasmtime,
        )
        .expect("select wasmtime target");

        let resolved = resolve::resolve(module).expect("resolve selected module");
        types::check(resolved).expect("non-selected invalid code should be ignored");
    }
}
