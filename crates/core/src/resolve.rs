use std::collections::HashMap;

use crate::{
    ast::{self, Expression, Pattern, Statement},
    diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label},
    source::Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Import { module: String },
    Parameter,
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub span: Span,
    pub kind: SymbolKind,
    pub scope: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<String, SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    pub symbols: Vec<Symbol>,
    pub scopes: Vec<Scope>,
}

impl SymbolTable {
    pub fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReference {
    pub name: ast::Name,
    pub target: ReferenceTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceTarget {
    Symbol(SymbolId),
    QualifiedMember { module: SymbolId, member: ast::Name },
}

/// AST after name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    pub ast: ast::Module,
    pub symbols: SymbolTable,
    pub references: Vec<ResolvedReference>,
}

pub fn resolve(module: ast::Module) -> Result<ResolvedModule, Diagnostics> {
    Resolver::new(module).resolve()
}

struct Resolver {
    module: ast::Module,
    symbols: Vec<Symbol>,
    scopes: Vec<Scope>,
    references: Vec<ResolvedReference>,
    diagnostics: Diagnostics,
}

impl Resolver {
    fn new(module: ast::Module) -> Self {
        Self { module, symbols: Vec::new(), scopes: Vec::new(), references: Vec::new(), diagnostics: Vec::new() }
    }

    fn resolve(mut self) -> Result<ResolvedModule, Diagnostics> {
        let module_scope = self.new_scope(None);
        self.collect_imports(module_scope);
        self.collect_functions(module_scope);

        for function in self.module.functions.clone() {
            self.resolve_function(module_scope, &function);
        }

        if self.diagnostics.is_empty() {
            Ok(ResolvedModule {
                ast: self.module,
                symbols: SymbolTable { symbols: self.symbols, scopes: self.scopes },
                references: self.references,
            })
        } else {
            Err(self.diagnostics)
        }
    }

    fn collect_imports(&mut self, scope: ScopeId) {
        for import in self.module.imports.clone() {
            let name = import.alias.clone().unwrap_or_else(|| ast::Name {
                span: import.module.span,
                text: import
                    .module
                    .text
                    .rsplit('/')
                    .next()
                    .unwrap_or(&import.module.text)
                    .to_string(),
            });
            self.define(scope, &name, SymbolKind::Import { module: import.module.text });
        }
    }

    fn collect_functions(&mut self, scope: ScopeId) {
        for function in self.module.functions.clone() {
            self.define(scope, &function.name, SymbolKind::Function);
        }
    }

    fn resolve_function(&mut self, parent: ScopeId, function: &ast::Function) {
        let function_scope = self.new_scope(Some(parent));

        for parameter in &function.parameters {
            if let Some(name) = &parameter.name {
                self.define(function_scope, name, SymbolKind::Parameter);
            }
        }

        self.resolve_block(function_scope, &function.body);
    }

    fn resolve_block(&mut self, scope: ScopeId, block: &ast::Block) {
        for statement in &block.statements {
            match statement {
                Statement::Let(let_) => {
                    self.resolve_expression(scope, &let_.value);
                    self.bind_pattern(scope, &let_.pattern, SymbolKind::Local);
                }
                Statement::Expression(expression) => self.resolve_expression(scope, expression),
            }
        }
    }

    fn resolve_expression(&mut self, scope: ScopeId, expression: &Expression) {
        match expression {
            Expression::Literal(_) => {}
            Expression::Variable(name) => self.resolve_name(scope, name),
            Expression::Call(call) => {
                self.resolve_expression(scope, &call.function);
                for argument in &call.arguments {
                    self.resolve_expression(scope, &argument.value);
                }
            }
            Expression::FieldAccess(field_access) => {
                if let Expression::Variable(record) = field_access.record.as_ref()
                    && let Some(module) = self.lookup(scope, &record.text)
                    && matches!(self.symbols[module.0 as usize].kind, SymbolKind::Import { .. })
                {
                    self.references.push(ResolvedReference {
                        name: record.clone(),
                        target: ReferenceTarget::QualifiedMember { module, member: field_access.field.clone() },
                    });
                    return;
                }

                self.resolve_expression(scope, &field_access.record);
            }
            Expression::Block(block) => {
                let child = self.new_scope(Some(scope));
                self.resolve_block(child, block);
            }
            Expression::Case(case) => {
                for subject in &case.subjects {
                    self.resolve_expression(scope, subject);
                }

                for clause in &case.clauses {
                    let clause_scope = self.new_scope(Some(scope));
                    for pattern in &clause.patterns {
                        self.bind_pattern(clause_scope, pattern, SymbolKind::Local);
                    }
                    self.resolve_expression(clause_scope, &clause.value);
                }
            }
        }
    }

    fn bind_pattern(&mut self, scope: ScopeId, pattern: &Pattern, kind: SymbolKind) {
        match pattern {
            Pattern::Name(name) => {
                self.define(scope, name, kind);
            }
            Pattern::Discard(_) | Pattern::Integer(_) | Pattern::Float(_) | Pattern::String(_) => {}
        }
    }

    fn resolve_name(&mut self, scope: ScopeId, name: &ast::Name) {
        match self.lookup(scope, &name.text) {
            Some(symbol) => self
                .references
                .push(ResolvedReference { name: name.clone(), target: ReferenceTarget::Symbol(symbol) }),
            None => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::ResolveError, format!("unknown name `{}`", name.text))
                    .with_label(Label::primary(name.span, "not found in scope")),
            ),
        }
    }

    fn define(&mut self, scope_id: ScopeId, name: &ast::Name, kind: SymbolKind) -> Option<SymbolId> {
        if let Some(previous) = self.scopes[scope_id.0 as usize].symbols.get(&name.text).copied() {
            let previous_span = self.symbols[previous.0 as usize].span;
            self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::ResolveError, format!("duplicate name `{}`", name.text))
                    .with_label(Label::primary(name.span, "defined again here"))
                    .with_label(Label::primary(previous_span, "previously defined here")),
            );
            return None;
        }

        let id = SymbolId(self.symbols.len() as u32);
        self.symbols
            .push(Symbol { id, name: name.text.clone(), span: name.span, kind, scope: scope_id });
        self.scopes[scope_id.0 as usize].symbols.insert(name.text.clone(), id);
        Some(id)
    }

    fn lookup(&self, mut scope_id: ScopeId, name: &str) -> Option<SymbolId> {
        loop {
            let scope = &self.scopes[scope_id.0 as usize];
            if let Some(symbol) = scope.symbols.get(name) {
                return Some(*symbol);
            }
            match scope.parent {
                Some(parent) => scope_id = parent,
                None => return None,
            }
        }
    }

    fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(Scope { id, parent, symbols: HashMap::new() });
        id
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ast, parse,
        source::{SourceFile, SourceFileId},
    };

    use super::*;

    fn resolve_source(source: &str) -> Result<ResolvedModule, Diagnostics> {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        resolve(ast)
    }

    #[test]
    fn resolves_parameters_locals_and_top_level_functions() {
        let resolved = resolve_source("fn add(a, b) { let total = a total }").expect("resolve names");

        let names = resolved
            .references
            .iter()
            .map(|reference| reference.name.text.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["a", "total"]);
    }

    #[test]
    fn allows_shadowing_in_nested_scopes() {
        let resolved = resolve_source("fn main(x) { { let x = 1 x } x }").expect("resolve names");

        let x_targets = resolved
            .references
            .iter()
            .filter_map(|reference| match (&reference.name.text[..], &reference.target) {
                ("x", ReferenceTarget::Symbol(symbol)) => Some(*symbol),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(x_targets.len(), 2);
        assert_ne!(x_targets[0], x_targets[1]);
    }

    #[test]
    fn reports_duplicate_names_in_same_scope() {
        let diagnostics = resolve_source("fn main(x, x) { x }").expect_err("duplicates should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ResolveError);
        assert!(diagnostics[0].message.contains("duplicate name `x`"));
    }

    #[test]
    fn reports_unknown_names() {
        let diagnostics = resolve_source("fn main() { missing }").expect_err("unknown name should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ResolveError);
        assert!(diagnostics[0].message.contains("unknown name `missing`"));
    }

    #[test]
    fn resolves_qualified_module_imports() {
        let resolved = resolve_source("import gleam/io\nfn main() { io.println(\"hi\") }").expect("resolve import");

        assert!(
            resolved
                .references
                .iter()
                .any(|reference| matches!(reference.target, ReferenceTarget::QualifiedMember { .. }))
        );
    }

    #[test]
    fn reports_duplicate_import_and_function_names() {
        let diagnostics =
            resolve_source("import gleam/io as main\nfn main() { 1 }").expect_err("duplicates should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ResolveError);
        assert!(diagnostics[0].message.contains("duplicate name `main`"));
    }
}
