use std::collections::HashMap;

use crate::ast::{self, Declaration};
use crate::types::{ConstructorInfo, ExternalFunctionInfo, InterfaceEntry, ModuleInterface, Type};
use crate::{labels::FunctionLabelMap, resolve::Namespace};
use crate::{source::Span, stdlib::StdlibRegistry};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveInterfaceRegistry {
    modules: HashMap<String, ResolveModuleInterface>,
    prelude: ResolveModuleInterface,
}

impl ResolveInterfaceRegistry {
    pub fn for_single_file() -> Self {
        Self::default().with_prelude_interface().with_stdlib_interfaces()
    }

    pub fn for_project<'a>(
        root_package: &str, dependency_interfaces: &HashMap<String, InterfaceEntry>,
        modules: impl IntoIterator<Item = (&'a String, &'a ast::Module)>,
    ) -> Self {
        Self::default()
            .with_prelude_interface()
            .with_stdlib_interfaces()
            .with_dependency_interfaces(dependency_interfaces)
            .with_project_interfaces(root_package, modules)
    }

    fn with_prelude_interface(mut self) -> Self {
        self.prelude = ResolveModuleInterface::prelude();
        self
    }

    fn with_stdlib_interfaces(mut self) -> Self {
        self.modules.extend(stdlib_resolve_interfaces());
        self
    }

    fn with_dependency_interfaces(mut self, interfaces: &HashMap<String, InterfaceEntry>) -> Self {
        self.modules.extend(dependency_resolve_interfaces(interfaces));
        self
    }

    fn with_project_interfaces<'a>(
        mut self, root_package: &str, modules: impl IntoIterator<Item = (&'a String, &'a ast::Module)>,
    ) -> Self {
        self.modules.extend(modules.into_iter().map(|(name, module)| {
            (
                name.clone(),
                ResolveModuleInterface::from_project(root_package, name, module),
            )
        }));
        self
    }

    pub fn get(&self, module: &str) -> Option<&ResolveModuleInterface> {
        self.modules.get(module)
    }

    pub fn member(&self, module: &str, namespace: Namespace, name: &str) -> Option<&ResolveModuleMember> {
        self.get(module)?.members.get(&(namespace, name.to_string()))
    }

    pub fn has_public_member(&self, namespace: Namespace, name: &str) -> bool {
        self.modules
            .values()
            .chain(std::iter::once(&self.prelude))
            .any(|interface| {
                interface
                    .members
                    .get(&(namespace, name.to_string()))
                    .is_some_and(|member| member.public)
            })
    }

    pub fn prelude_members(&self) -> impl Iterator<Item = (&(Namespace, String), &ResolveModuleMember)> {
        self.prelude.members.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveModuleMember {
    pub public: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolveModuleInterface {
    pub package: Option<String>,
    pub module: String,
    pub members: HashMap<(Namespace, String), ResolveModuleMember>,
    pub externals: HashMap<String, ExternalFunctionInfo>,
}

impl ResolveModuleInterface {
    fn prelude() -> Self {
        let members = ModuleInterface::prelude_type_names()
            .into_iter()
            .map(|name| {
                (
                    (Namespace::Type, name.to_string()),
                    ResolveModuleMember { public: true, span: ModuleInterface::module_span() },
                )
            })
            .collect();
        Self { package: None, module: "prelude".to_string(), members, externals: HashMap::new() }
    }

    fn from_project(package: &str, name: &str, module: &ast::Module) -> Self {
        let mut interface = Self::from(module);
        interface.package = Some(package.to_string());
        interface.module = name.to_string();
        interface
    }
}

impl From<&ast::Module> for ResolveModuleInterface {
    fn from(value: &ast::Module) -> Self {
        // TODO: can this be constructed from an iterator?
        let mut members = HashMap::new();
        let mut externals = HashMap::new();
        for function in &value.functions {
            members.insert(
                (Namespace::Value, function.name.text.clone()),
                ResolveModuleMember { public: function.public, span: function.name.span },
            );
        }

        for declaration in &value.declarations {
            match declaration {
                Declaration::Constant(constant) => {
                    members.insert(
                        (Namespace::Value, constant.name.text.clone()),
                        ResolveModuleMember { public: constant.public, span: constant.span },
                    );
                }
                Declaration::ExternalFunction(function) => {
                    members.insert(
                        (Namespace::Value, function.name.text.clone()),
                        ResolveModuleMember { public: function.public, span: function.span },
                    );
                    externals.insert(function.name.text.clone(), function.into());
                }
                Declaration::ExternalType(type_) => {
                    members.insert(
                        (Namespace::Type, type_.name.text.clone()),
                        ResolveModuleMember { public: type_.public, span: type_.span },
                    );
                }
                Declaration::TypeDefinition(type_) => {
                    members.insert(
                        (Namespace::Type, type_.name.text.clone()),
                        ResolveModuleMember { public: type_.public, span: type_.span },
                    );
                    let exported_details = type_.public && !type_.opaque;
                    for constructor in &type_.constructors {
                        members.insert(
                            (Namespace::Constructor, constructor.name.text.clone()),
                            ResolveModuleMember { public: exported_details, span: constructor.span },
                        );
                        for argument in &constructor.arguments {
                            if let Some(label) = &argument.label {
                                members.insert(
                                    (Namespace::Field, label.text.clone()),
                                    ResolveModuleMember { public: exported_details, span: label.span },
                                );
                            }
                        }
                    }
                }
                Declaration::TypeAlias(alias) => {
                    members.insert(
                        (Namespace::Type, alias.name.text.clone()),
                        ResolveModuleMember { public: alias.public, span: alias.span },
                    );
                }
                Declaration::TargetGroup(group) => {
                    let nested = Self::from(&ast::Module {
                        span: group.span,
                        declarations: group.declarations.clone(),
                        imports: Vec::new(),
                        functions: Vec::new(),
                    });
                    members.extend(nested.members);
                    externals.extend(nested.externals);
                }
                _ => {}
            }
        }

        Self { package: None, module: String::new(), members, externals }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeInterfaceRegistry {
    pub modules: HashMap<String, InterfaceEntry>,
    prelude: ModuleInterface,
}

impl TypeInterfaceRegistry {
    pub fn for_single_file() -> Self {
        Self::default().with_prelude_interface().with_stdlib_interfaces()
    }

    pub fn for_project<'a>(
        root_package: &str, dependency_interfaces: &HashMap<String, InterfaceEntry>,
        project_modules: impl IntoIterator<Item = (&'a str, &'a ast::Module)>,
    ) -> Self {
        Self::default()
            .with_prelude_interface()
            .with_stdlib_interfaces()
            .with_dependency_interfaces(dependency_interfaces)
            .with_project_interfaces(root_package, project_modules)
    }

    fn with_prelude_interface(mut self) -> Self {
        self.prelude = ModuleInterface::prelude();
        self
    }

    fn with_stdlib_interfaces(mut self) -> Self {
        self.modules.extend(StdlibRegistry::new().modules().map(|module| {
            (
                module.name.to_string(),
                InterfaceEntry::new("gleam_stdlib", module.name, module.interface.clone()),
            )
        }));
        self
    }

    fn with_dependency_interfaces(mut self, interfaces: &HashMap<String, InterfaceEntry>) -> Self {
        self.modules.extend(interfaces.clone());
        self
    }

    fn with_project_interfaces<'a>(
        mut self, root_package: &str, modules: impl IntoIterator<Item = (&'a str, &'a ast::Module)>,
    ) -> Self {
        self.modules.extend(modules.into_iter().map(|(name, module)| {
            (
                name.to_string(),
                InterfaceEntry::new(root_package, name, ModuleInterface::from(module)),
            )
        }));
        self
    }

    pub fn get(&self, module: &str) -> Option<&ModuleInterface> {
        self.modules.get(module).map(|entry| &entry.interface)
    }

    pub fn constructors(&self) -> HashMap<String, ConstructorInfo> {
        self.modules
            .values()
            .map(|entry| &entry.interface)
            .chain(std::iter::once(&self.prelude))
            .flat_map(|i| i.interface_constructors())
            .collect()
    }

    pub fn values(&self) -> HashMap<String, Type> {
        self.modules
            .iter()
            .flat_map(|(module, entry)| entry.interface.qualified_values(module))
            .collect()
    }

    pub fn function_labels(&self) -> FunctionLabelMap {
        self.modules
            .iter()
            .flat_map(|(module, entry)| entry.interface.qualified_function_labels(module))
            .collect()
    }
}

fn stdlib_resolve_interfaces() -> HashMap<String, ResolveModuleInterface> {
    StdlibRegistry::new()
        .modules()
        .map(|module| {
            let mut members = HashMap::new();
            for name in module.interface.functions.keys() {
                members.insert(
                    (Namespace::Value, name.clone()),
                    ResolveModuleMember { public: true, span: ModuleInterface::module_span() },
                );
            }
            for name in module.interface.types.keys() {
                members.insert(
                    (Namespace::Type, name.clone()),
                    ResolveModuleMember { public: true, span: ModuleInterface::module_span() },
                );
            }
            for name in module.interface.constructors.keys() {
                members.insert(
                    (Namespace::Constructor, name.clone()),
                    ResolveModuleMember { public: true, span: ModuleInterface::module_span() },
                );
            }
            (
                module.name.to_string(),
                ResolveModuleInterface {
                    package: Some("gleam_stdlib".to_string()),
                    module: module.name.to_string(),
                    members,
                    externals: module.interface.externals.clone(),
                },
            )
        })
        .collect()
}

fn dependency_resolve_interfaces(
    interfaces: &HashMap<String, InterfaceEntry>,
) -> HashMap<String, ResolveModuleInterface> {
    interfaces
        .iter()
        .map(|(module, entry)| {
            let interface = &entry.interface;
            let mut members = HashMap::new();
            for name in interface.functions.keys() {
                members.insert(
                    (Namespace::Value, name.clone()),
                    ResolveModuleMember { public: true, span: ModuleInterface::module_span() },
                );
            }
            for (name, declaration) in &interface.types {
                members.insert(
                    (Namespace::Type, name.clone()),
                    ResolveModuleMember { public: true, span: declaration.span },
                );
            }
            for (name, constructor) in &interface.constructors {
                members.insert(
                    (Namespace::Constructor, name.clone()),
                    ResolveModuleMember { public: true, span: constructor.span },
                );
                for field in &constructor.fields {
                    members.insert(
                        (Namespace::Field, field.name.clone()),
                        ResolveModuleMember { public: true, span: constructor.span },
                    );
                }
            }
            (
                module.clone(),
                ResolveModuleInterface {
                    package: Some(entry.package.clone()),
                    module: entry.module.clone(),
                    members,
                    externals: interface.externals.clone(),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ast, parse,
        source::{SourceFile, SourceFileId},
    };

    fn parse_module(source: &str) -> ast::Module {
        let source = SourceFile::new(SourceFileId(0), source);
        let cst = parse::parse(source).expect("parse source");
        ast::build(&cst).expect("build ast")
    }

    #[test]
    fn resolver_interfaces_preserve_bodyless_external_metadata() {
        let module =
            parse_module("@external(javascript, \"regulus/js\", \"read\")\npub fn read(key: String) -> String\n");

        let interface = ResolveModuleInterface::from(&module);

        let external = interface.externals.get("read").expect("external metadata");
        assert_eq!(external.target.as_deref(), Some("javascript"));
        assert_eq!(external.module, "regulus/js");
        assert_eq!(external.function, "read");
        assert!(interface.members.contains_key(&(Namespace::Value, "read".to_string())));
    }
}
