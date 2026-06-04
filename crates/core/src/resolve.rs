use std::collections::HashMap;

use crate::ast::{self, Declaration, Expression, Pattern, Statement, UnqualifiedImportKind};
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::{parse, project::Project, source::Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    Value,
    Type,
    Constructor,
    Field,
    Module,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function { public: bool },
    Import { module: String },
    Imported { module: String, member: String },
    Parameter,
    Local,
    Type,
    Constructor,
    Field,
    Prelude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub namespace: Namespace,
    pub span: Span,
    pub kind: SymbolKind,
    pub scope: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub parent: Option<ScopeId>,
    pub symbols: HashMap<(Namespace, String), SymbolId>,
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
    QualifiedMember {
        module: SymbolId,
        member: ast::Name,
        symbol: Option<SymbolId>,
    },
}

/// AST after name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedModule {
    pub ast: ast::Module,
    pub symbols: SymbolTable,
    pub references: Vec<ResolvedReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProject {
    pub modules: Vec<ResolvedModule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ModuleInterface {
    members: HashMap<(Namespace, String), ModuleMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModuleMember {
    public: bool,
    span: Span,
}

pub fn resolve(module: ast::Module) -> Result<ResolvedModule, Diagnostics> {
    Resolver::new(module).resolve()
}

pub fn resolve_project(project: &Project) -> Result<ResolvedProject, Diagnostics> {
    let mut ast_modules = Vec::new();
    let mut diagnostics = Vec::new();

    for (module_info, source) in project.graph.modules.iter().zip(project.sources.iter()) {
        match parse::parse(source.clone()).and_then(ast::build) {
            Ok(ast) => ast_modules.push((module_info.name.clone(), ast)),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let interfaces = ast_modules
        .iter()
        .map(|(name, module)| (name.clone(), module_interface(module)))
        .collect::<HashMap<_, _>>();

    let mut modules = Vec::new();
    for (name, module) in ast_modules {
        match Resolver::with_project(module, name, interfaces.clone()).resolve() {
            Ok(resolved) => modules.push(resolved),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if diagnostics.is_empty() { Ok(ResolvedProject { modules }) } else { Err(diagnostics) }
}

struct Resolver {
    module: ast::Module,
    module_name: Option<String>,
    project_modules: HashMap<String, ModuleInterface>,
    symbols: Vec<Symbol>,
    scopes: Vec<Scope>,
    references: Vec<ResolvedReference>,
    diagnostics: Diagnostics,
}

impl Resolver {
    fn new(module: ast::Module) -> Self {
        Self {
            module,
            module_name: None,
            project_modules: HashMap::new(),
            symbols: Vec::new(),
            scopes: Vec::new(),
            references: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn with_project(
        module: ast::Module, module_name: String, project_modules: HashMap<String, ModuleInterface>,
    ) -> Self {
        Self { module_name: Some(module_name), project_modules, ..Self::new(module) }
    }

    fn resolve(mut self) -> Result<ResolvedModule, Diagnostics> {
        let module_scope = self.new_scope(None);
        self.collect_prelude(module_scope);
        self.collect_imports(module_scope);
        self.collect_declarations(module_scope);

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

    fn collect_prelude(&mut self, scope: ScopeId) {
        for name in ["Int", "Float", "String", "BitArray", "Bool", "Nil", "List", "Result"] {
            let name = ast::Name { span: self.module.span, text: name.into() };
            self.define(scope, &name, Namespace::Type, SymbolKind::Prelude);
        }
    }

    fn collect_imports(&mut self, scope: ScopeId) {
        for import in self.module.imports.clone() {
            let module_symbol_name = import.alias.clone().unwrap_or_else(|| ast::Name {
                span: import.module.span,
                text: import
                    .module
                    .text
                    .rsplit('/')
                    .next()
                    .unwrap_or(&import.module.text)
                    .to_string(),
            });
            self.define(
                scope,
                &module_symbol_name,
                Namespace::Module,
                SymbolKind::Import { module: import.module.text.clone() },
            );

            for imported in &import.unqualified {
                let local_name = imported.alias.as_ref().unwrap_or(&imported.name);
                match imported.kind {
                    UnqualifiedImportKind::Value => self.define_imported(
                        scope,
                        local_name,
                        Namespace::Value,
                        &import.module.text,
                        &imported.name.text,
                    ),
                    UnqualifiedImportKind::TypeOrConstructor => {
                        self.define_imported(
                            scope,
                            local_name,
                            Namespace::Type,
                            &import.module.text,
                            &imported.name.text,
                        );
                        self.define_imported(
                            scope,
                            local_name,
                            Namespace::Constructor,
                            &import.module.text,
                            &imported.name.text,
                        );
                    }
                }
            }
        }
    }

    fn define_imported(&mut self, scope: ScopeId, name: &ast::Name, namespace: Namespace, module: &str, member: &str) {
        if let Some(previous) = self.scopes[scope.0 as usize]
            .symbols
            .get(&(namespace, name.text.clone()))
            .copied()
            && matches!(self.symbols[previous.0 as usize].kind, SymbolKind::Imported { .. })
        {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("ambiguous imported name `{}`", name.text),
                )
                .with_label(Label::primary(name.span, "imported more than once"))
                .with_label(Label::primary(
                    self.symbols[previous.0 as usize].span,
                    "previous import here",
                )),
            );
            return;
        }
        self.define(
            scope,
            name,
            namespace,
            SymbolKind::Imported { module: module.into(), member: member.into() },
        );
    }

    fn collect_declarations(&mut self, scope: ScopeId) {
        for declaration in self.module.declarations.clone() {
            match declaration {
                Declaration::Function(function) => {
                    self.define(
                        scope,
                        &function.name,
                        Namespace::Value,
                        SymbolKind::Function { public: function.public },
                    );
                }
                Declaration::TypeDefinition(raw) => self.collect_type_definition(scope, &raw),
                Declaration::TypeAlias(raw) => {
                    if let Some(name) = type_name(&raw.source) {
                        self.define(scope, &raw_name(&raw, name), Namespace::Type, SymbolKind::Type);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_type_definition(&mut self, scope: ScopeId, raw: &ast::RawSyntax) {
        if let Some(name) = type_name(&raw.source) {
            self.define(scope, &raw_name(raw, name), Namespace::Type, SymbolKind::Type);
        }
        for constructor in constructors(&raw.source) {
            self.define(
                scope,
                &raw_name(raw, constructor),
                Namespace::Constructor,
                SymbolKind::Constructor,
            );
        }
        for field in fields(&raw.source) {
            self.define(scope, &raw_name(raw, field), Namespace::Field, SymbolKind::Field);
        }
    }

    fn resolve_function(&mut self, parent: ScopeId, function: &ast::Function) {
        let function_scope = self.new_scope(Some(parent));

        for parameter in &function.parameters {
            if let Some(name) = &parameter.name {
                self.define(function_scope, name, Namespace::Value, SymbolKind::Parameter);
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
                Statement::LetAssert(let_assert) => {
                    self.resolve_expression(scope, &let_assert.value);
                    if let Some(message) = &let_assert.message {
                        self.resolve_expression(scope, message);
                    }
                    self.bind_pattern(scope, &let_assert.pattern, SymbolKind::Local);
                }
                Statement::Expression(expression) => self.resolve_expression(scope, expression),
            }
        }
    }

    fn resolve_expression(&mut self, scope: ScopeId, expression: &Expression) {
        match expression {
            Expression::Literal(_) | Expression::Raw(_) => {}
            Expression::Variable(name) => self.resolve_name(scope, name),
            Expression::Call(call) => {
                self.resolve_expression(scope, &call.function);
                for argument in &call.arguments {
                    self.resolve_expression(scope, &argument.value);
                }
            }
            Expression::FieldAccess(field_access) => self.resolve_field_access(scope, field_access),
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
                    if let Some(guard) = &clause.guard {
                        self.resolve_expression(clause_scope, guard);
                    }
                    self.resolve_expression(clause_scope, &clause.value);
                }
            }
        }
    }

    fn resolve_field_access(&mut self, scope: ScopeId, field_access: &ast::FieldAccess) {
        if let Expression::Variable(record) = field_access.record.as_ref()
            && let Some(module) = self.lookup(scope, Namespace::Module, &record.text)
            && let SymbolKind::Import { module: module_name } = &self.symbols[module.0 as usize].kind.clone()
        {
            let symbol = self.resolve_project_member(module_name, Namespace::Value, &field_access.field);
            self.references.push(ResolvedReference {
                name: record.clone(),
                target: ReferenceTarget::QualifiedMember { module, member: field_access.field.clone(), symbol },
            });
            return;
        }

        self.resolve_expression(scope, &field_access.record);
    }

    fn resolve_project_member(
        &mut self, module_name: &str, namespace: Namespace, member: &ast::Name,
    ) -> Option<SymbolId> {
        let Some(interface) = self.project_modules.get(module_name) else {
            return None;
        };
        let Some(found) = interface.members.get(&(namespace, member.text.clone())).cloned() else {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("module `{module_name}` has no member `{}`", member.text),
                )
                .with_label(Label::primary(member.span, "unknown module member")),
            );
            return None;
        };
        if !found.public && self.module_name.as_deref() != Some(module_name) {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("member `{}` is private", member.text),
                )
                .with_label(Label::primary(member.span, "private member"))
                .with_label(Label::primary(found.span, "defined here")),
            );
        }
        None
    }

    fn bind_pattern(&mut self, scope: ScopeId, pattern: &Pattern, kind: SymbolKind) {
        let mut names = HashMap::new();
        self.bind_pattern_inner(scope, pattern, kind, &mut names);
    }

    fn bind_pattern_inner(
        &mut self, scope: ScopeId, pattern: &Pattern, kind: SymbolKind, names: &mut HashMap<String, Span>,
    ) {
        match pattern {
            Pattern::Name(name) => self.define_pattern_name(scope, name, kind, names),
            Pattern::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.bind_pattern_inner(scope, element, kind.clone(), names);
                }
            }
            Pattern::List(list) => {
                for element in &list.elements {
                    self.bind_pattern_inner(scope, element, kind.clone(), names);
                }
                if let Some(ast::ListPatternTail::Name(name)) = &list.tail {
                    self.define_pattern_name(scope, name, kind.clone(), names);
                }
            }
            Pattern::Constructor(constructor) => {
                self.resolve_constructor_pattern(scope, constructor);
                for argument in &constructor.arguments {
                    self.resolve_record_pattern_field(scope, argument);
                    if let Some(pattern) = &argument.pattern {
                        self.bind_pattern_inner(scope, pattern, kind.clone(), names);
                    } else if let Some(label) = &argument.label {
                        self.define_pattern_name(scope, label, kind.clone(), names);
                    }
                }
            }
            Pattern::Alias(alias) => {
                self.bind_pattern_inner(scope, &alias.pattern, kind.clone(), names);
                self.define_pattern_name(scope, &alias.alias, kind, names);
            }
            Pattern::Discard(_)
            | Pattern::Integer(_)
            | Pattern::Float(_)
            | Pattern::String(_)
            | Pattern::Bool(_)
            | Pattern::Nil(_)
            | Pattern::BitString(_)
            | Pattern::Raw(_) => {}
        }
    }

    fn define_pattern_name(
        &mut self, scope: ScopeId, name: &ast::Name, kind: SymbolKind, names: &mut HashMap<String, Span>,
    ) {
        if let Some(previous) = names.insert(name.text.clone(), name.span) {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("duplicate pattern binding `{}`", name.text),
                )
                .with_label(Label::primary(name.span, "bound again here"))
                .with_label(Label::primary(previous, "previously bound here")),
            );
            return;
        }
        self.define(scope, name, Namespace::Value, kind);
    }

    fn resolve_constructor_pattern(&mut self, scope: ScopeId, constructor: &ast::ConstructorPattern) {
        match &constructor.constructor {
            ast::ConstructorName::Local(name) => {
                self.resolve_pattern_symbol(scope, Namespace::Constructor, name, "constructor");
            }
            ast::ConstructorName::Remote { module, name, .. } => {
                let Some(module_symbol) = self.lookup(scope, Namespace::Module, &module.text) else {
                    self.diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::ResolveError,
                            format!("unknown module `{}`", module.text),
                        )
                        .with_label(Label::primary(module.span, "module not found")),
                    );
                    return;
                };
                if let SymbolKind::Import { module: module_name } = &self.symbols[module_symbol.0 as usize].kind.clone()
                {
                    let symbol = self.resolve_project_member(module_name, Namespace::Constructor, name);
                    self.references.push(ResolvedReference {
                        name: module.clone(),
                        target: ReferenceTarget::QualifiedMember {
                            module: module_symbol,
                            member: name.clone(),
                            symbol,
                        },
                    });
                }
            }
        }
    }

    fn resolve_record_pattern_field(&mut self, scope: ScopeId, argument: &ast::RecordPatternArgument) {
        if let Some(label) = &argument.label {
            self.resolve_pattern_symbol(scope, Namespace::Field, label, "field");
        }
    }

    fn resolve_pattern_symbol(
        &mut self, scope: ScopeId, namespace: Namespace, name: &ast::Name, label: &str,
    ) -> Option<SymbolId> {
        match self.lookup(scope, namespace, &name.text) {
            Some(symbol) => {
                if let SymbolKind::Imported { module, .. } = &self.symbols[symbol.0 as usize].kind.clone() {
                    self.resolve_project_member(module, namespace, name);
                }
                self.references
                    .push(ResolvedReference { name: name.clone(), target: ReferenceTarget::Symbol(symbol) });
                Some(symbol)
            }
            None => {
                self.diagnostics.push(
                    Diagnostic::new(DiagnosticCode::ResolveError, format!("unknown {label} `{}`", name.text))
                        .with_label(Label::primary(name.span, format!("{label} not found"))),
                );
                None
            }
        }
    }

    fn resolve_name(&mut self, scope: ScopeId, name: &ast::Name) {
        match self
            .lookup(scope, Namespace::Value, &name.text)
            .or_else(|| self.lookup(scope, Namespace::Constructor, &name.text))
        {
            Some(symbol) => self
                .references
                .push(ResolvedReference { name: name.clone(), target: ReferenceTarget::Symbol(symbol) }),
            None => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::ResolveError, format!("unknown name `{}`", name.text))
                    .with_label(Label::primary(name.span, "not found in scope")),
            ),
        }
    }

    fn define(
        &mut self, scope_id: ScopeId, name: &ast::Name, namespace: Namespace, kind: SymbolKind,
    ) -> Option<SymbolId> {
        let key = (namespace, name.text.clone());
        if let Some(previous) = self.scopes[scope_id.0 as usize].symbols.get(&key).copied() {
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
            .push(Symbol { id, name: name.text.clone(), namespace, span: name.span, kind, scope: scope_id });
        self.scopes[scope_id.0 as usize].symbols.insert(key, id);
        Some(id)
    }

    fn lookup(&self, mut scope_id: ScopeId, namespace: Namespace, name: &str) -> Option<SymbolId> {
        loop {
            let scope = &self.scopes[scope_id.0 as usize];
            if let Some(symbol) = scope.symbols.get(&(namespace, name.to_string())) {
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

fn module_interface(module: &ast::Module) -> ModuleInterface {
    let mut members = HashMap::new();

    for function in &module.functions {
        members.insert(
            (Namespace::Value, function.name.text.clone()),
            ModuleMember { public: function.public, span: function.name.span },
        );
    }

    for declaration in &module.declarations {
        match declaration {
            Declaration::TypeDefinition(raw) => {
                let public = raw.source.trim_start().starts_with("pub ");
                if let Some(name) = type_name(&raw.source) {
                    members.insert((Namespace::Type, name.into()), ModuleMember { public, span: raw.span });
                }
                for constructor in constructors(&raw.source) {
                    members.insert(
                        (Namespace::Constructor, constructor.into()),
                        ModuleMember { public, span: raw.span },
                    );
                }
                for field in fields(&raw.source) {
                    members.insert(
                        (Namespace::Field, field.into()),
                        ModuleMember { public, span: raw.span },
                    );
                }
            }
            Declaration::TypeAlias(raw) => {
                let public = raw.source.trim_start().starts_with("pub ");
                if let Some(name) = type_name(&raw.source) {
                    members.insert((Namespace::Type, name.into()), ModuleMember { public, span: raw.span });
                }
            }
            _ => {}
        }
    }

    ModuleInterface { members }
}

fn raw_name(raw: &ast::RawSyntax, name: &str) -> ast::Name {
    ast::Name { span: raw.span, text: name.into() }
}

fn type_name(source: &str) -> Option<&str> {
    let mut words = source
        .split_whitespace()
        .filter(|word| *word != "pub" && *word != "opaque");
    if words.next()? != "type" {
        return None;
    }
    words
        .next()
        .map(|word| word.split(['(', '{', '=']).next().unwrap_or(word))
}

fn constructors(source: &str) -> Vec<&str> {
    let Some((_, body)) = source.split_once('{') else {
        return Vec::new();
    };
    body.lines()
        .filter_map(|line| {
            let line = line.trim();
            let name = line.split(['(', ' ', '}']).next().unwrap_or("");
            name.chars().next().is_some_and(char::is_uppercase).then_some(name)
        })
        .collect()
}

fn fields(source: &str) -> Vec<&str> {
    source
        .split(['(', ',', ')'])
        .filter_map(|part| part.trim().split_once(':').map(|(name, _)| name.trim()))
        .filter(|name| !name.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast, parse, project,
        source::{SourceFile, SourceFileId},
    };
    use std::{fs, path::Path};
    use tempfile::tempdir;

    fn resolve_source(source: &str) -> Result<ResolvedModule, Diagnostics> {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(cst).expect("build ast");
        resolve(ast)
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture dir");
        fs::write(path, text).expect("write fixture");
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
    fn reports_duplicate_function_names() {
        let diagnostics = resolve_source("fn main() { 1 }\nfn main() { 2 }").expect_err("duplicates should fail");

        assert_eq!(diagnostics[0].code, DiagnosticCode::ResolveError);
        assert!(diagnostics[0].message.contains("duplicate name `main`"));
    }

    #[test]
    fn keeps_value_type_constructor_and_field_names_in_separate_namespaces() {
        let resolved = resolve_source(
            r#"pub type User { User(name: String) }
fn user(value) { value }
"#,
        )
        .expect("resolve names");

        assert!(
            resolved
                .symbols
                .symbols
                .iter()
                .any(|symbol| symbol.namespace == Namespace::Type && symbol.name == "User")
        );
        assert!(
            resolved
                .symbols
                .symbols
                .iter()
                .any(|symbol| symbol.namespace == Namespace::Constructor && symbol.name == "User")
        );
        assert!(
            resolved
                .symbols
                .symbols
                .iter()
                .any(|symbol| symbol.namespace == Namespace::Field && symbol.name == "name")
        );
        assert!(
            resolved
                .symbols
                .symbols
                .iter()
                .any(|symbol| symbol.namespace == Namespace::Value && symbol.name == "user")
        );
    }

    #[test]
    fn resolves_unqualified_value_imports() {
        let resolved = resolve_source("import app.{id}\nfn main() { id(1) }").expect("resolve unqualified import");

        assert!(resolved.references.iter().any(|reference| reference.name.text == "id"));
    }

    #[test]
    fn resolves_nested_pattern_bindings_constructors_fields_and_guards() {
        let resolved = resolve_source(include_str!("../../../fixtures/resolve/pattern_bindings.gleam"))
            .expect("resolve pattern names");

        assert!(resolved.references.iter().any(|reference| reference.name.text == "Ok"));
        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "Person")
        );
        assert!(resolved.references.iter().any(|reference| reference.name.text == "age"));

        let value_references = resolved
            .references
            .iter()
            .filter(|reference| reference.name.text == "value")
            .count();
        assert_eq!(value_references, 2);
    }

    #[test]
    fn reports_duplicate_bindings_in_one_pattern() {
        let diagnostics = resolve_source("fn main(pair) { case pair { #(x, x) -> x } }")
            .expect_err("duplicate pattern binding should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate pattern binding `x`"))
        );
    }

    #[test]
    fn reports_unknown_constructors_and_fields_in_patterns() {
        let diagnostics = resolve_source(
            r#"pub type Person { Person(name: String) }
fn main(person) { case person { Missing(age: value) -> value } }
"#,
        )
        .expect_err("unknown pattern names should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown constructor `Missing`"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown field `age`"))
        );
    }

    #[test]
    fn reports_ambiguous_unqualified_imports() {
        let diagnostics = resolve_source("import one.{id}\nimport two.{id}\nfn main() { id(1) }")
            .expect_err("ambiguous import should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("ambiguous imported name `id`"))
        );
    }

    #[test]
    fn resolves_project_modules_and_rejects_private_access() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(
            &dir.path().join("src/app.gleam"),
            "pub fn id(x: Int) -> Int { x }\nfn hidden() { 1 }\n",
        );
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main() { app.hidden() }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let diagnostics = resolve_project(&project).expect_err("private access should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("member `hidden` is private"))
        );
    }

    #[test]
    fn resolves_public_members_across_project_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub fn id(x: Int) -> Int { x }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main() { app.id(1) }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve project");

        assert_eq!(resolved.modules.len(), 2);
    }

    #[test]
    fn resolves_qualified_constructor_patterns_across_project_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub type Boxed { Boxed(Int) }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main(value) { case value { app.Boxed(inner) -> inner } }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve project");

        assert_eq!(resolved.modules.len(), 2);
    }

    #[test]
    fn resolves_unqualified_imported_constructor_patterns() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "pub type Boxed { Boxed(Int) }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app.{type Boxed}\nfn main(value) { case value { Boxed(inner) -> inner } }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve project");

        assert_eq!(resolved.modules.len(), 2);
    }

    #[test]
    fn rejects_private_qualified_constructor_patterns_across_project_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(&dir.path().join("src/app.gleam"), "type Boxed { Boxed(Int) }\n");
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main(value) { case value { app.Boxed(inner) -> inner } }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let diagnostics = resolve_project(&project).expect_err("private constructor should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("member `Boxed` is private"))
        );
    }
}
