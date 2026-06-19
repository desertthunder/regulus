use std::collections::HashMap;

use crate::ast::{self, Declaration, Expression, Pattern, Statement, UnqualifiedImportKind};
use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label};
use crate::loader::registry::ResolveInterfaceRegistry;
use crate::{parse, project::Project, source::Span, target};

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
    Function {
        public: bool,
    },
    Constant {
        public: bool,
    },
    ExternalFunction {
        public: bool,
    },
    Import {
        package: Option<String>,
        module: String,
    },
    Imported {
        package: Option<String>,
        module: String,
        member: String,
    },
    Parameter,
    Local,
    Type {
        public: bool,
        opaque: bool,
    },
    Constructor {
        public: bool,
    },
    Field {
        public: bool,
    },
    Label,
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

pub fn resolve(module: ast::Module) -> Result<ResolvedModule, Diagnostics> {
    Resolver::new(module).with_stdlib_interfaces().resolve()
}

pub fn resolve_project(project: &Project) -> Result<ResolvedProject, Diagnostics> {
    let mut ast_modules = Vec::new();
    let mut diagnostics = Vec::new();

    for (module_info, source) in project.graph.modules.iter().zip(project.sources.iter()) {
        match parse::parse(source.clone())
            .and_then(|cst| ast::build(&cst))
            .and_then(|ast| target::select_module(ast, project.compile_target))
        {
            Ok(ast) => ast_modules.push((module_info.name.clone(), ast)),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let interfaces = ResolveInterfaceRegistry::for_project(
        &project.graph.root_package.name,
        &project.graph.dependency_interfaces,
        ast_modules.iter().map(|(name, module)| (name, module)),
    );

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
    interfaces: ResolveInterfaceRegistry,
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
            interfaces: ResolveInterfaceRegistry::default(),
            symbols: Vec::new(),
            scopes: Vec::new(),
            references: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn with_project(module: ast::Module, module_name: String, interfaces: ResolveInterfaceRegistry) -> Self {
        Self { module_name: Some(module_name), interfaces, ..Self::new(module) }
    }

    fn with_stdlib_interfaces(mut self) -> Self {
        self.interfaces = ResolveInterfaceRegistry::for_single_file();
        self
    }

    fn resolve(mut self) -> Result<ResolvedModule, Diagnostics> {
        let module_scope = self.new_scope(None);
        self.collect_prelude(module_scope);
        self.collect_imports(module_scope);
        self.collect_declarations(module_scope);

        for declaration in self.module.declarations.clone() {
            self.resolve_declaration(module_scope, &declaration);
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
        let members = self
            .interfaces
            .prelude_members()
            .map(|((namespace, name), member)| (*namespace, name.clone(), member.span))
            .collect::<Vec<_>>();
        for (namespace, name, span) in members {
            let name = ast::Name { span, text: name };
            self.define(scope, &name, namespace, SymbolKind::Prelude);
        }
    }

    fn collect_imports(&mut self, scope: ScopeId) {
        for import in self.module.imports.clone() {
            let module_known = self.validate_import_module(&import);
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
                SymbolKind::Import {
                    package: self
                        .interfaces
                        .get(&import.module.text)
                        .and_then(|interface| interface.package.clone()),
                    module: import.module.text.clone(),
                },
            );

            for imported in &import.unqualified {
                if !module_known {
                    continue;
                }
                let local_name = imported.alias.as_ref().unwrap_or(&imported.name);
                match imported.kind {
                    UnqualifiedImportKind::Value => {
                        if self.validate_imported_member(&import.module.text, Namespace::Value, &imported.name) {
                            self.define_imported(
                                scope,
                                local_name,
                                Namespace::Value,
                                &import.module.text,
                                &imported.name.text,
                            );
                        }
                    }
                    UnqualifiedImportKind::TypeOrConstructor => {
                        let imported_type =
                            self.imported_member_is_public(&import.module.text, Namespace::Type, &imported.name);
                        let imported_constructor =
                            self.imported_member_is_public(&import.module.text, Namespace::Constructor, &imported.name);
                        if !imported_type && !imported_constructor {
                            self.validate_imported_member(&import.module.text, Namespace::Type, &imported.name);
                        }
                        if imported_type {
                            self.define_imported(
                                scope,
                                local_name,
                                Namespace::Type,
                                &import.module.text,
                                &imported.name.text,
                            );
                        }
                        if imported_constructor {
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
    }

    fn validate_import_module(&mut self, import: &ast::Import) -> bool {
        if self.interfaces.get(&import.module.text).is_some() {
            return true;
        }

        self.diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::ResolveError,
                format!("unknown module `{}`", import.module.text),
            )
            .with_label(Label::primary(import.module.span, "module not found")),
        );
        false
    }

    fn imported_member_is_public(&self, module: &str, namespace: Namespace, member: &ast::Name) -> bool {
        self.interfaces
            .member(module, namespace, &member.text)
            .is_some_and(|found| found.public)
    }

    fn validate_imported_member(&mut self, module: &str, namespace: Namespace, member: &ast::Name) -> bool {
        let Some(found) = self.interfaces.member(module, namespace, &member.text).cloned() else {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("module `{module}` has no member `{}`", member.text),
                )
                .with_label(Label::primary(member.span, "unknown module member")),
            );
            return false;
        };
        if !found.public {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("member `{}` is private", member.text),
                )
                .with_label(Label::primary(member.span, "private member"))
                .with_label(Label::primary(found.span, "defined here")),
            );
            return false;
        }
        true
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
            SymbolKind::Imported {
                package: self
                    .interfaces
                    .get(module)
                    .and_then(|interface| interface.package.clone()),
                module: module.into(),
                member: member.into(),
            },
        );
    }

    fn collect_declarations(&mut self, scope: ScopeId) {
        for declaration in self.module.declarations.clone() {
            self.collect_declaration(scope, &declaration);
        }
    }

    fn collect_declaration(&mut self, scope: ScopeId, declaration: &Declaration) {
        match declaration {
            Declaration::Function(function) => {
                self.define(
                    scope,
                    &function.name,
                    Namespace::Value,
                    SymbolKind::Function { public: function.public },
                );
            }
            Declaration::Constant(constant) => {
                self.define(
                    scope,
                    &constant.name,
                    Namespace::Value,
                    SymbolKind::Constant { public: constant.public },
                );
            }
            Declaration::ExternalFunction(function) => {
                self.define(
                    scope,
                    &function.name,
                    Namespace::Value,
                    SymbolKind::ExternalFunction { public: function.public },
                );
            }
            Declaration::ExternalType(type_) => {
                self.define(
                    scope,
                    &type_.name,
                    Namespace::Type,
                    SymbolKind::Type { public: type_.public, opaque: type_.opaque },
                );
            }
            Declaration::TypeDefinition(type_) => self.collect_type_definition(scope, type_),
            Declaration::TypeAlias(alias) => {
                self.define(
                    scope,
                    &alias.name,
                    Namespace::Type,
                    SymbolKind::Type { public: alias.public, opaque: alias.opaque },
                );
            }
            Declaration::TargetGroup(group) => {
                for declaration in &group.declarations {
                    self.collect_declaration(scope, declaration);
                }
            }
            _ => {}
        }
    }

    fn collect_type_definition(&mut self, scope: ScopeId, type_: &ast::TypeDefinition) {
        self.define(
            scope,
            &type_.name,
            Namespace::Type,
            SymbolKind::Type { public: type_.public, opaque: type_.opaque },
        );
        for constructor in &type_.constructors {
            self.define(
                scope,
                &constructor.name,
                Namespace::Constructor,
                SymbolKind::Constructor { public: type_.public && !type_.opaque },
            );
            for argument in &constructor.arguments {
                if let Some(label) = &argument.label {
                    self.define(
                        scope,
                        label,
                        Namespace::Field,
                        SymbolKind::Field { public: type_.public && !type_.opaque },
                    );
                }
            }
        }
    }

    fn resolve_declaration(&mut self, scope: ScopeId, declaration: &Declaration) {
        match declaration {
            Declaration::Function(function) => self.resolve_function(scope, function),
            Declaration::Constant(constant) => {
                if let Some(type_) = &constant.type_annotation {
                    self.resolve_type_annotation(scope, type_);
                }
                self.resolve_expression(scope, &constant.value);
            }
            Declaration::ExternalFunction(function) => {
                for parameter in &function.parameters {
                    if let Some(type_) = &parameter.type_annotation {
                        self.resolve_type_annotation(scope, type_);
                    }
                }
                self.resolve_type_annotation(scope, &function.return_type);
            }
            Declaration::ExternalType(_) => {}
            Declaration::TypeAlias(alias) => self.resolve_type_annotation(scope, &alias.value),
            Declaration::TypeDefinition(type_) => {
                for constructor in &type_.constructors {
                    for argument in &constructor.arguments {
                        self.resolve_type_annotation(scope, &argument.type_annotation);
                    }
                }
            }
            Declaration::TargetGroup(group) => {
                for declaration in &group.declarations {
                    self.resolve_declaration(scope, declaration);
                }
            }
            Declaration::Import(_)
            | Declaration::Attribute(_)
            | Declaration::Comment(_)
            | Declaration::Statement(_) => {}
        }
    }

    fn resolve_function(&mut self, parent: ScopeId, function: &ast::Function) {
        let function_scope = self.new_scope(Some(parent));

        for parameter in &function.parameters {
            if let Some(type_) = &parameter.type_annotation {
                self.resolve_type_annotation(parent, type_);
            }
            if let Some(name) = &parameter.name {
                self.define(function_scope, name, Namespace::Value, SymbolKind::Parameter);
            }
        }
        if let Some(type_) = &function.return_type {
            self.resolve_type_annotation(parent, type_);
        }

        self.resolve_block(function_scope, &function.body);
    }

    fn resolve_block(&mut self, scope: ScopeId, block: &ast::Block) {
        for (index, statement) in block.statements.iter().enumerate() {
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
                Statement::Expression(Expression::Use(use_)) => {
                    self.resolve_use(scope, use_, &block.statements[index + 1..]);
                    break;
                }
                Statement::Expression(expression) => self.resolve_expression(scope, expression),
            }
        }
    }

    fn resolve_use(&mut self, scope: ScopeId, use_: &ast::Use, continuation: &[Statement]) {
        self.resolve_expression(scope, &use_.value);
        let child = self.new_scope(Some(scope));
        for assignment in &use_.assignments {
            self.bind_pattern(child, &assignment.pattern, SymbolKind::Local);
        }
        for (index, statement) in continuation.iter().enumerate() {
            match statement {
                Statement::Let(let_) => {
                    self.resolve_expression(child, &let_.value);
                    self.bind_pattern(child, &let_.pattern, SymbolKind::Local);
                }
                Statement::LetAssert(let_assert) => {
                    self.resolve_expression(child, &let_assert.value);
                    if let Some(message) = &let_assert.message {
                        self.resolve_expression(child, message);
                    }
                    self.bind_pattern(child, &let_assert.pattern, SymbolKind::Local);
                }
                Statement::Expression(Expression::Use(use_)) => {
                    self.resolve_use(child, use_, &continuation[index + 1..]);
                    break;
                }
                Statement::Expression(expression) => self.resolve_expression(child, expression),
            }
        }
    }

    fn resolve_expression(&mut self, scope: ScopeId, expression: &Expression) {
        match expression {
            Expression::Literal(_) | Expression::Raw(_) => {}
            Expression::Variable(name) => self.resolve_name(scope, name),
            Expression::Call(call) => {
                self.resolve_expression(scope, &call.function);
                self.resolve_arguments(scope, &call.arguments);
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
            Expression::BinaryOperation(operation) => {
                self.resolve_expression(scope, &operation.left);
                self.resolve_expression(scope, &operation.right);
            }
            Expression::Pipeline(pipeline) => {
                self.resolve_expression(scope, &pipeline.value);
                self.resolve_expression(scope, &pipeline.into);
            }
            Expression::UnaryOperation(operation) => self.resolve_expression(scope, &operation.value),
            Expression::Use(use_) => self.resolve_use(scope, use_, &[]),
            Expression::AnonymousFunction(function) => {
                let child = self.new_scope(Some(scope));
                for parameter in &function.parameters {
                    if let Some(name) = &parameter.name {
                        self.define(child, name, Namespace::Value, SymbolKind::Parameter);
                    }
                }
                self.resolve_block(child, &function.body);
            }
            Expression::Capture(capture) => {
                self.resolve_expression(scope, &capture.function);
                for argument in capture.arguments.iter().flatten() {
                    self.resolve_expression(scope, &argument.value);
                }
            }
            Expression::Record(record) => self.resolve_record(scope, record),
            Expression::RecordUpdate(update) => {
                self.resolve_constructor_name(scope, &update.constructor);
                self.resolve_expression(scope, &update.spread);
                self.resolve_record_arguments(scope, &update.updates);
            }
            Expression::Tuple(tuple) => {
                for element in &tuple.elements {
                    self.resolve_expression(scope, element);
                }
            }
            Expression::TupleAccess(access) => self.resolve_expression(scope, &access.tuple),
            Expression::List(list) => {
                for element in &list.elements {
                    self.resolve_expression(scope, element);
                }
                if let Some(spread) = &list.spread {
                    self.resolve_expression(scope, spread);
                }
            }
            Expression::BitArray(bit_array) => {
                for segment in &bit_array.segments {
                    self.resolve_expression(scope, &segment.value);
                    for option in &segment.options {
                        if let Some(value) = &option.value {
                            self.resolve_expression(scope, value);
                        }
                    }
                }
            }
            Expression::Panic(failure) | Expression::Todo(failure) => {
                if let Some(message) = &failure.message {
                    self.resolve_expression(scope, message);
                }
            }
            Expression::Assert(assert) => {
                self.resolve_expression(scope, &assert.value);
                self.bind_pattern(scope, &assert.pattern, SymbolKind::Local);
            }
            Expression::Echo(echo) => self.resolve_expression(scope, &echo.value),
        }
    }

    fn resolve_type_annotation(&mut self, scope: ScopeId, type_: &ast::TypeAnnotation) {
        for name in type_.names() {
            self.resolve_type_name(scope, &name);
        }
    }

    fn resolve_type_name(&mut self, scope: ScopeId, name: &ast::Name) {
        if let Some(symbol) = self.lookup(scope, Namespace::Type, &name.text) {
            if let SymbolKind::Imported { module, member, .. } = &self.symbols[symbol.0 as usize].kind.clone() {
                self.resolve_project_member(
                    module,
                    Namespace::Type,
                    &ast::Name { span: name.span, text: member.clone() },
                );
            }
            self.references
                .push(ResolvedReference { name: name.clone(), target: ReferenceTarget::Symbol(symbol) });
            return;
        }

        if self.interfaces.has_public_member(Namespace::Type, &name.text) {
            return;
        }

        self.diagnostics.push(
            Diagnostic::new(DiagnosticCode::ResolveError, format!("unknown type `{}`", name.text))
                .with_label(Label::primary(name.span, "type not found")),
        );
    }

    fn resolve_arguments(&mut self, scope: ScopeId, arguments: &[ast::Argument]) {
        for argument in arguments {
            self.resolve_expression(scope, &argument.value);
        }
    }

    fn resolve_record(&mut self, scope: ScopeId, record: &ast::Record) {
        self.resolve_constructor_name(scope, &record.constructor);
        self.resolve_record_arguments(scope, &record.arguments);
    }

    fn resolve_record_arguments(&mut self, scope: ScopeId, arguments: &[ast::Argument]) {
        for argument in arguments {
            if let Some(label) = &argument.label {
                self.resolve_pattern_symbol(scope, Namespace::Field, label, "field");
            }
            self.resolve_expression(scope, &argument.value);
        }
    }

    fn resolve_constructor_name(&mut self, scope: ScopeId, constructor: &ast::ConstructorName) {
        match constructor {
            ast::ConstructorName::Local(name) => {
                self.resolve_pattern_symbol(scope, Namespace::Constructor, name, "constructor");
            }
            ast::ConstructorName::Remote { module, name, .. } => {
                self.resolve_qualified_member(scope, module, Namespace::Constructor, name);
            }
        }
    }

    fn resolve_field_access(&mut self, scope: ScopeId, field_access: &ast::FieldAccess) {
        if let Expression::Variable(record) = field_access.record.as_ref()
            && self.resolve_qualified_member(scope, record, Namespace::Value, &field_access.field)
        {
            return;
        }

        self.resolve_expression(scope, &field_access.record);
    }

    fn resolve_qualified_member(
        &mut self, scope: ScopeId, module: &ast::Name, namespace: Namespace, member: &ast::Name,
    ) -> bool {
        let Some(module_symbol) = self.lookup(scope, Namespace::Module, &module.text) else {
            self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::ResolveError,
                    format!("unknown module `{}`", module.text),
                )
                .with_label(Label::primary(module.span, "module not found")),
            );
            return false;
        };
        let SymbolKind::Import { module: module_name, .. } = &self.symbols[module_symbol.0 as usize].kind.clone()
        else {
            return false;
        };
        let symbol = self.resolve_project_member(module_name, namespace, member);
        self.references.push(ResolvedReference {
            name: module.clone(),
            target: ReferenceTarget::QualifiedMember { module: module_symbol, member: member.clone(), symbol },
        });
        true
    }

    fn resolve_project_member(
        &mut self, module_name: &str, namespace: Namespace, member: &ast::Name,
    ) -> Option<SymbolId> {
        let interface = self.interfaces.get(module_name)?;
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
                    self.define_pattern_name(scope, name, kind, names);
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
            Pattern::BitString(raw) => {
                for name in raw.bit_string_pattern_bindings() {
                    self.define_pattern_name(scope, &name, kind.clone(), names);
                }
            }
            Pattern::Discard(_)
            | Pattern::Integer(_)
            | Pattern::Float(_)
            | Pattern::String(_)
            | Pattern::Bool(_)
            | Pattern::Nil(_)
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
                self.resolve_qualified_member(scope, module, Namespace::Constructor, name);
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
            Some(symbol) => {
                if let SymbolKind::Imported { module, member, .. } = &self.symbols[symbol.0 as usize].kind.clone() {
                    self.resolve_project_member(
                        module,
                        Namespace::Value,
                        &ast::Name { span: name.span, text: member.clone() },
                    );
                }
                self.references
                    .push(ResolvedReference { name: name.clone(), target: ReferenceTarget::Symbol(symbol) });
            }
            None if name.text == "use" => self.diagnostics.push(
                Diagnostic::new(DiagnosticCode::ResolveError, "use has no continuation")
                    .with_label(Label::primary(name.span, "`use` must start a block statement"))
                    .with_note("`use` passes the following block statements as its callback body"),
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast, parse, project,
        source::{SourceFile, SourceFileId},
    };
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use tempfile::tempdir;

    fn resolve_source(source: &str) -> Result<ResolvedModule, Diagnostics> {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        let ast = ast::build(&cst).expect("build ast");
        resolve(ast)
    }

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture dir");
        fs::write(path, text).expect("write fixture");
    }

    fn fixture_project(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/projects")
            .join(path)
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
        let resolved = resolve_source("import gleam/int.{to_string}\nfn main() { to_string(1) }")
            .expect("resolve unqualified import");

        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "to_string")
        );
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
        let diagnostics =
            resolve_source("import gleam/int.{to_string}\nimport gleam/float.{to_string}\nfn main() { to_string(1) }")
                .expect_err("ambiguous import should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("ambiguous imported name `to_string`"))
        );
    }

    #[test]
    fn reports_unknown_imported_modules() {
        let diagnostics = resolve_source("import missing\nfn main() { 1 }").expect_err("unknown import should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unknown module `missing`"))
        );
    }

    #[test]
    fn reports_unknown_unqualified_imported_members() {
        let diagnostics = resolve_source("import gleam/int.{missing}\nfn main() { 1 }")
            .expect_err("unknown imported member should fail");

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("module `gleam/int` has no member `missing`")
        }));
    }

    #[test]
    fn resolves_prelude_names_from_interface_registry() {
        let resolved = resolve_source("fn main(values: List(Int)) -> Int { 1 }").expect("resolve prelude types");

        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "List")
        );
        assert!(resolved.references.iter().any(|reference| reference.name.text == "Int"));
    }

    #[test]
    fn resolves_project_stdlib_dependency_and_unqualified_imports_through_registry() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            r#"name = "app"
version = "1.0.0"

[dependencies]
dep_pkg = { path = "dep_pkg" }
"#,
        );
        write(&dir.path().join("src/app.gleam"), "pub fn id(x: Int) -> Int { x }\n");
        write(
            &dir.path().join("src/main.gleam"),
            r#"import app.{id}
import dep/foo.{dep_id}
import gleam/int.{to_string}

pub fn main() -> String {
  to_string(id(dep_id(1)))
}
"#,
        );
        let dep = dir.path().join("dep_pkg");
        write(&dep.join("gleam.toml"), "name = \"dep_pkg\"\nversion = \"0.1.0\"\n");
        write(&dep.join("src/dep/foo.gleam"), "pub fn dep_id(x: Int) -> Int { x }\n");
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve registry imports");

        assert_eq!(resolved.modules.len(), 2);
        assert!(resolved.modules.iter().any(|module| {
            module
                .references
                .iter()
                .any(|reference| reference.name.text == "to_string")
        }));
    }

    #[test]
    fn resolves_project_modules_and_rejects_private_access() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\ntarget = \"javascript\"\n",
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
    fn fixture_rejects_module_private_members_across_project_modules() {
        let project = project::load_project(fixture_project("linking/private_member_diagnostic"))
            .expect("load private member fixture");

        let diagnostics = resolve_project(&project).expect_err("private access should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("member `hidden` is private"))
        );
    }

    #[test]
    fn dependency_interface_lookup_respects_public_visibility() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            r#"name = "app"
version = "1.0.0"

[dependencies]
dep_pkg = { path = "dep_pkg" }
"#,
        );
        write(
            &dir.path().join("src/app.gleam"),
            r#"import dep/foo.{hidden, visible}

pub fn main() {
  #(visible(), hidden())
}
"#,
        );
        let dep = dir.path().join("dep_pkg");
        write(&dep.join("gleam.toml"), "name = \"dep_pkg\"\nversion = \"0.1.0\"\n");
        write(
            &dep.join("src/dep/foo.gleam"),
            r#"pub fn visible() -> Int {
  1
}

fn hidden() -> Int {
  2
}
"#,
        );

        let project = project::load_project(dir.path()).expect("load project");
        let diagnostics = resolve_project(&project).expect_err("private dependency member should be hidden");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("module `dep/foo` has no member `hidden`")),
            "expected hidden member lookup diagnostic, got {diagnostics:#?}"
        );
        assert!(
            !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("visible"))
        );
    }

    #[test]
    fn filters_project_target_groups_before_interface_resolution() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\ntarget = \"javascript\"\n",
        );
        write(
            &dir.path().join("src/app.gleam"),
            r#"if javascript {
  pub external fn load(key: String) -> String = "browser" "load"
}

if erlang {
  pub external fn load(key: String) -> String = "env" "load"
}"#,
        );
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main() { app.load(\"key\") }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve selected project target");

        assert_eq!(resolved.modules.len(), 2);
        assert!(resolved.modules.iter().any(|module| {
            module.ast.declarations.iter().any(|declaration| {
                matches!(
                    declaration,
                    Declaration::ExternalFunction(function)
                        if function.body.module.source == "\"browser\""
                )
            })
        }));
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
    fn resolves_bodyless_externals_across_project_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\ntarget = \"javascript\"\n",
        );
        write(
            &dir.path().join("src/host.gleam"),
            "@external(javascript, \"regulus/js\", \"read\")\npub fn read(key: String) -> String\n",
        );
        write(
            &dir.path().join("src/main.gleam"),
            "import host\nfn main() { host.read(\"key\") }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let resolved = resolve_project(&project).expect("resolve project");

        assert_eq!(resolved.modules.len(), 2);
        assert!(resolved.modules.iter().any(|module| {
            module.ast.declarations.iter().any(|declaration| {
                matches!(
                    declaration,
                    Declaration::ExternalFunction(function)
                        if function.body.target.as_ref().is_some_and(|target| target.text == "javascript")
                )
            })
        }));
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
    fn resolves_records_constants_externals_and_type_annotations() {
        let resolved = resolve_source(
            r#"pub const answer: Int = 42
pub external type Handle
pub type User { User(name: String) }
external fn load(handle: Handle) -> User = "host" "load"
fn main(handle: Handle) { User(name: "Ada") }
"#,
        )
        .expect("resolve declarations and records");

        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "User")
        );
        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "name")
        );
        assert!(
            resolved
                .references
                .iter()
                .any(|reference| reference.name.text == "Handle")
        );
    }

    #[test]
    fn rejects_opaque_constructors_across_project_modules() {
        let dir = tempdir().expect("tempdir");
        write(
            &dir.path().join("gleam.toml"),
            "name = \"sample\"\nversion = \"1.0.0\"\n",
        );
        write(
            &dir.path().join("src/app.gleam"),
            "pub opaque type Boxed { Boxed(Int) }\n",
        );
        write(
            &dir.path().join("src/main.gleam"),
            "import app\nfn main() { app.Boxed(1) }\n",
        );
        let project = project::load_project(dir.path()).expect("load project");

        let diagnostics = resolve_project(&project).expect_err("opaque constructor should fail");

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("member `Boxed` is private"))
        );
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
