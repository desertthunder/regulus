use std::fmt::Display;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::{ast, project, source::Span};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CompileTarget {
    #[default]
    Wasmtime,
    Browser,
    Bundler,
    Nodejs,
    Wasi,
    Wasm,
}

impl CompileTarget {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wasmtime => "wasmtime",
            Self::Browser => "browser",
            Self::Bundler => "bundler",
            Self::Nodejs => "nodejs",
            Self::Wasi => "wasi",
            Self::Wasm => "wasm",
        }
    }
}

impl Display for CompileTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

pub fn project_compile_target(target: Option<&project::Target>) -> CompileTarget {
    match target {
        Some(project::Target::Javascript) => CompileTarget::Browser,
        Some(project::Target::Erlang) | None => CompileTarget::Wasmtime,
    }
}

pub fn select_module(module: ast::Module, target: CompileTarget) -> Result<ast::Module, Diagnostics> {
    let mut selector = TargetSelector { target, diagnostics: Vec::new(), filtered_declarations: Vec::new() };
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
    Ok(ast::Module {
        span: module.span,
        declarations,
        imports,
        functions,
        filtered_declarations: selector.filtered_declarations,
    })
}

struct TargetSelector {
    target: CompileTarget,
    diagnostics: Diagnostics,
    filtered_declarations: Vec<ast::FilteredDeclaration>,
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
            ast::Declaration::Function(function) => self.select_function(function),
            ast::Declaration::Constant(constant) => self.select_constant(constant),
            ast::Declaration::ExternalFunction(function) => self.select_external_function(function),
            ast::Declaration::ExternalType(type_) => self.select_external_type(type_),
            ast::Declaration::TypeAlias(alias) => self.select_type_alias(alias),
            ast::Declaration::TypeDefinition(type_) => self.select_type_definition(type_),
            declaration => vec![declaration],
        }
    }

    fn select_group(&mut self, group: ast::TargetGroup) -> Vec<ast::Declaration> {
        if !target_matches(
            &group.target.text,
            group.target.span,
            self.target,
            &mut self.diagnostics,
        ) {
            self.record_filtered_declarations(&group.declarations, &group.target);
            return Vec::new();
        }
        self.select_declarations(group.declarations)
    }

    fn select_function(&mut self, function: ast::Function) -> Vec<ast::Declaration> {
        if self.declaration_target_matches(function.target.as_ref()) {
            vec![ast::Declaration::Function(function)]
        } else {
            self.record_filtered_value(&function.name, function.target.as_ref());
            Vec::new()
        }
    }

    fn select_constant(&mut self, constant: ast::Constant) -> Vec<ast::Declaration> {
        if self.declaration_target_matches(constant.target.as_ref()) {
            vec![ast::Declaration::Constant(constant)]
        } else {
            self.record_filtered_value(&constant.name, constant.target.as_ref());
            Vec::new()
        }
    }

    fn select_external_function(&mut self, function: ast::ExternalFunction) -> Vec<ast::Declaration> {
        if !self.declaration_target_matches(function.target.as_ref()) {
            self.record_filtered_value(&function.name, function.target.as_ref());
            return Vec::new();
        }
        let Some(target) = function.body.target.as_ref() else {
            return vec![ast::Declaration::ExternalFunction(function)];
        };
        if target_matches(&target.text, target.span, self.target, &mut self.diagnostics) {
            vec![ast::Declaration::ExternalFunction(function)]
        } else {
            self.record_filtered_value(&function.name, Some(target));
            Vec::new()
        }
    }

    fn select_external_type(&mut self, type_: ast::ExternalType) -> Vec<ast::Declaration> {
        if self.declaration_target_matches(type_.target.as_ref()) {
            vec![ast::Declaration::ExternalType(type_)]
        } else {
            self.record_filtered_type(&type_.name, type_.target.as_ref());
            Vec::new()
        }
    }

    fn select_type_alias(&mut self, alias: ast::TypeAlias) -> Vec<ast::Declaration> {
        if self.declaration_target_matches(alias.target.as_ref()) {
            vec![ast::Declaration::TypeAlias(alias)]
        } else {
            self.record_filtered_type(&alias.name, alias.target.as_ref());
            Vec::new()
        }
    }

    fn select_type_definition(&mut self, type_: ast::TypeDefinition) -> Vec<ast::Declaration> {
        if self.declaration_target_matches(type_.target.as_ref()) {
            vec![ast::Declaration::TypeDefinition(type_)]
        } else {
            self.record_filtered_type_definition(&type_, type_.target.as_ref());
            Vec::new()
        }
    }

    fn declaration_target_matches(&mut self, target: Option<&ast::Name>) -> bool {
        match target {
            Some(target) => target_matches(&target.text, target.span, self.target, &mut self.diagnostics),
            None => true,
        }
    }

    fn record_filtered_declarations(&mut self, declarations: &[ast::Declaration], target: &ast::Name) {
        for declaration in declarations {
            match declaration {
                ast::Declaration::TargetGroup(group) => self.record_filtered_declarations(&group.declarations, target),
                ast::Declaration::Function(function) => self.record_filtered_value(&function.name, Some(target)),
                ast::Declaration::Constant(constant) => self.record_filtered_value(&constant.name, Some(target)),
                ast::Declaration::ExternalFunction(function) => {
                    self.record_filtered_value(&function.name, function.target.as_ref().or(Some(target)));
                }
                ast::Declaration::ExternalType(type_) => self.record_filtered_type(&type_.name, Some(target)),
                ast::Declaration::TypeAlias(alias) => self.record_filtered_type(&alias.name, Some(target)),
                ast::Declaration::TypeDefinition(type_) => self.record_filtered_type_definition(type_, Some(target)),
                _ => {}
            }
        }
    }

    fn record_filtered_value(&mut self, name: &ast::Name, target: Option<&ast::Name>) {
        self.record_filtered(name, ast::FilteredDeclarationKind::Value, target);
    }

    fn record_filtered_type(&mut self, name: &ast::Name, target: Option<&ast::Name>) {
        self.record_filtered(name, ast::FilteredDeclarationKind::Type, target);
    }

    fn record_filtered_type_definition(&mut self, type_: &ast::TypeDefinition, target: Option<&ast::Name>) {
        self.record_filtered_type(&type_.name, target);
        for constructor in &type_.constructors {
            self.record_filtered(&constructor.name, ast::FilteredDeclarationKind::Constructor, target);
            for argument in &constructor.arguments {
                if let Some(label) = &argument.label {
                    self.record_filtered(label, ast::FilteredDeclarationKind::Field, target);
                }
            }
        }
    }

    fn record_filtered(&mut self, name: &ast::Name, kind: ast::FilteredDeclarationKind, target: Option<&ast::Name>) {
        let Some(target) = target else { return };
        self.filtered_declarations
            .push(ast::FilteredDeclaration { name: name.clone(), kind, target: target.clone() });
    }
}

fn target_name(name: &str, span: Span, diagnostics: &mut Diagnostics) -> Option<CompileTarget> {
    match name {
        "wasmtime" | "erlang" => Some(CompileTarget::Wasmtime),
        "browser" | "javascript" => Some(CompileTarget::Browser),
        "bundler" => Some(CompileTarget::Bundler),
        "nodejs" => Some(CompileTarget::Nodejs),
        "wasi" => Some(CompileTarget::Wasi),
        "wasm" => Some(CompileTarget::Wasm),
        _ => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("unsupported target group `{name}`"),
                )
                .with_label(Label::primary(span, "unsupported target here"))
                .with_note(
                    "supported targets are `wasmtime`, `erlang`, `browser`, \
                     `javascript`, `bundler`, `nodejs`, `wasi`, and `wasm`",
                ),
            );
            None
        }
    }
}

fn target_matches(name: &str, span: Span, selected: CompileTarget, diagnostics: &mut Diagnostics) -> bool {
    match name {
        "javascript" => matches!(
            selected,
            CompileTarget::Browser | CompileTarget::Bundler | CompileTarget::Nodejs
        ),
        _ => target_name(name, span, diagnostics) == Some(selected),
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

    #[test]
    fn selects_javascript_bodyless_externals_for_js_family_targets() {
        let source = r#"@external(javascript, "regulus/js", "request_text")
pub fn request_text(input: String) -> String
"#;

        let bundler = select_module(ast_for(source), CompileTarget::Bundler).expect("select bundler target");
        let wasmtime = select_module(ast_for(source), CompileTarget::Wasmtime).expect("select wasmtime target");

        assert!(
            bundler
                .declarations
                .iter()
                .any(|declaration| matches!(declaration, ast::Declaration::ExternalFunction(_)))
        );
        assert!(
            !wasmtime
                .declarations
                .iter()
                .any(|declaration| matches!(declaration, ast::Declaration::ExternalFunction(_)))
        );
    }

    #[test]
    fn standalone_target_attributes_filter_duplicate_declarations_before_type_checking() {
        let source = r#"
@target(javascript)
pub const selected = 1

@target(erlang)
pub const selected = 2

@target(javascript)
pub type Shape {
  Shape(value: Int)
}

@target(erlang)
pub type Shape {
  Shape(value: String)
}

@target(javascript)
@external(javascript, "regulus/js", "identity")
pub fn identity(input: Int) -> Int

@target(erlang)
@external(erlang, "env", "identity")
pub fn identity(input: Int) -> Int

pub fn main() -> Int { identity(selected) }
"#;

        for target in [
            CompileTarget::Browser,
            CompileTarget::Bundler,
            CompileTarget::Nodejs,
            CompileTarget::Wasmtime,
        ] {
            let module = select_module(ast_for(source), target).expect("select target");
            let resolved = resolve::resolve(module).expect("resolve selected module");
            types::check(resolved).expect("type check selected module");
        }
    }

    #[test]
    fn standalone_target_attributes_filter_upstream_set_shaped_duplicates() {
        let source = r#"
@target(javascript)
pub type Set(element) = List(element)

@target(erlang)
pub type Set(element) {
  Set(values: List(element))
}

@target(javascript)
pub fn new() -> Set(Int) { [] }

@target(erlang)
pub fn new() -> Set(Int) { Set([]) }

pub fn main() -> Set(Int) { new() }
"#;

        for target in [CompileTarget::Browser, CompileTarget::Wasmtime] {
            let module = select_module(ast_for(source), target).expect("select target");
            resolve::resolve(module).expect("resolve selected module");
        }
    }
}
