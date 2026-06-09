use std::collections::HashMap;

use crate::source::{SourceFileId, Span};
use crate::types::{ConstructorInfo, FieldInfo, ModuleInterface, Type, TypeDeclaration};

pub const STDLIB_IO_HOST_MODULE: &str = "__regulus_stdlib_io";

const STDLIB_SPAN: Span = Span { file_id: SourceFileId(u32::MAX), start: 0, end: 0 };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleStrategy {
    Hybrid,
    PreferCompiledSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberStrategy {
    Intrinsic,
    HostImport,
    ManagedConstructor,
    InterfaceOnly,
    PreferCompiledSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibMember {
    pub name: &'static str,
    pub strategy: MemberStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibModule {
    pub name: &'static str,
    pub strategy: ModuleStrategy,
    pub interface: ModuleInterface,
    pub members: Vec<StdlibMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdlibRegistry {
    modules: HashMap<&'static str, StdlibModule>,
}

impl StdlibRegistry {
    pub fn new() -> Self {
        Self {
            modules: stdlib_modules()
                .into_iter()
                .map(|module| (module.name, module))
                .collect(),
        }
    }

    pub fn module(&self, name: &str) -> Option<&StdlibModule> {
        self.modules.get(name)
    }

    pub fn interface(&self, name: &str) -> Option<&ModuleInterface> {
        self.module(name).map(|module| &module.interface)
    }

    pub fn member_strategy(&self, module: &str, member: &str) -> Option<MemberStrategy> {
        self.module(module)?
            .members
            .iter()
            .find(|entry| entry.name == member)
            .map(|entry| entry.strategy)
    }

    pub fn modules(&self) -> impl Iterator<Item = &StdlibModule> {
        self.modules.values()
    }
}

impl Default for StdlibRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn stdlib_modules() -> Vec<StdlibModule> {
    vec![
        StdlibModule::gleam_io(),
        StdlibModule::gleam_int(),
        StdlibModule::gleam_string(),
        StdlibModule::gleam_list(),
        StdlibModule::gleam_result(),
        StdlibModule::gleam_option(),
        StdlibModule::gleam_order(),
        StdlibModule::remaining("gleam/bit_array"),
        StdlibModule::remaining("gleam/bool"),
        StdlibModule::remaining("gleam/bytes_tree"),
        StdlibModule::remaining("gleam/dict"),
        StdlibModule::remaining("gleam/dynamic"),
        StdlibModule::remaining("gleam/dynamic/decode"),
        StdlibModule::remaining("gleam/float"),
        StdlibModule::remaining("gleam/function"),
        StdlibModule::remaining("gleam/pair"),
        StdlibModule::remaining("gleam/set"),
        StdlibModule::remaining("gleam/string_tree"),
        StdlibModule::remaining("gleam/uri"),
    ]
}

impl StdlibModule {
    fn gleam_io() -> Self {
        Self::new(
            "gleam/io",
            ModuleStrategy::Hybrid,
            &[
                function("println", vec![Type::String], Type::Nil, MemberStrategy::HostImport),
                function("print", vec![Type::String], Type::Nil, MemberStrategy::HostImport),
                function(
                    "debug",
                    vec![Type::generic("a")],
                    Type::generic("a"),
                    MemberStrategy::Intrinsic,
                ),
            ],
            &[],
        )
    }

    fn gleam_int() -> Self {
        Self::new(
            "gleam/int",
            ModuleStrategy::Hybrid,
            &[
                function("to_string", vec![Type::Int], Type::String, MemberStrategy::Intrinsic),
                function(
                    "parse",
                    vec![Type::String],
                    result(Type::Int, Type::Nil),
                    MemberStrategy::InterfaceOnly,
                ),
            ],
            &[],
        )
    }

    fn gleam_string() -> Self {
        Self::new(
            "gleam/string",
            ModuleStrategy::Hybrid,
            &[
                function(
                    "append",
                    vec![Type::String, Type::String],
                    Type::String,
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "concat",
                    vec![Type::List(Box::new(Type::String))],
                    Type::String,
                    MemberStrategy::Intrinsic,
                ),
                function("length", vec![Type::String], Type::Int, MemberStrategy::Intrinsic),
                function("is_empty", vec![Type::String], Type::Bool, MemberStrategy::Intrinsic),
            ],
            &[],
        )
    }

    fn gleam_list() -> Self {
        Self::new(
            "gleam/list",
            ModuleStrategy::Hybrid,
            &[
                function(
                    "length",
                    vec![Type::List(Box::new(Type::generic("a")))],
                    Type::Int,
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "reverse",
                    vec![Type::List(Box::new(Type::generic("a")))],
                    list(Type::generic("a")),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "map",
                    vec![
                        list(Type::generic("a")),
                        fn_type(vec![Type::generic("a")], Type::generic("b")),
                    ],
                    list(Type::generic("b")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "fold",
                    vec![
                        list(Type::generic("a")),
                        Type::generic("b"),
                        fn_type(vec![Type::generic("b"), Type::generic("a")], Type::generic("b")),
                    ],
                    Type::generic("b"),
                    MemberStrategy::InterfaceOnly,
                ),
            ],
            &[],
        )
    }

    fn gleam_result() -> Self {
        let result_type = type_decl(
            "Result",
            vec!["a", "e"],
            false,
            vec![
                constructor(
                    "Ok",
                    vec![field("value", Type::generic("a"))],
                    result(Type::generic("a"), Type::generic("e")),
                ),
                constructor(
                    "Error",
                    vec![field("reason", Type::generic("e"))],
                    result(Type::generic("a"), Type::generic("e")),
                ),
            ],
        );
        Self::new(
            "gleam/result",
            ModuleStrategy::Hybrid,
            &[
                constructor_member("Ok"),
                constructor_member("Error"),
                function(
                    "map",
                    vec![
                        result(Type::generic("a"), Type::generic("e")),
                        fn_type(vec![Type::generic("a")], Type::generic("b")),
                    ],
                    result(Type::generic("b"), Type::generic("e")),
                    MemberStrategy::InterfaceOnly,
                ),
            ],
            &[result_type],
        )
    }

    fn gleam_option() -> Self {
        let option_type = type_decl(
            "Option",
            vec!["a"],
            false,
            vec![
                constructor(
                    "Some",
                    vec![field("value", Type::generic("a"))],
                    option(Type::generic("a")),
                ),
                constructor("None", vec![], option(Type::generic("a"))),
            ],
        );
        Self::new(
            "gleam/option",
            ModuleStrategy::Hybrid,
            &[
                constructor_member("Some"),
                constructor_member("None"),
                function(
                    "map",
                    vec![
                        option(Type::generic("a")),
                        fn_type(vec![Type::generic("a")], Type::generic("b")),
                    ],
                    option(Type::generic("b")),
                    MemberStrategy::InterfaceOnly,
                ),
            ],
            &[option_type],
        )
    }

    fn gleam_order() -> Self {
        let order_type = type_decl(
            "Order",
            vec![],
            false,
            vec![
                constructor("Lt", vec![], Type::custom("Order", vec![])),
                constructor("Eq", vec![], Type::custom("Order", vec![])),
                constructor("Gt", vec![], Type::custom("Order", vec![])),
            ],
        );
        Self::new(
            "gleam/order",
            ModuleStrategy::Hybrid,
            &[
                constructor_member("Lt"),
                constructor_member("Eq"),
                constructor_member("Gt"),
            ],
            &[order_type],
        )
    }

    fn remaining(name: &'static str) -> Self {
        Self::new(name, ModuleStrategy::PreferCompiledSource, &[], &[])
    }

    fn new(
        name: &'static str, strategy: ModuleStrategy, members: &[StdlibMemberSpec], types: &[TypeDeclaration],
    ) -> Self {
        let mut interface = ModuleInterface::default();
        let mut member_entries = Vec::new();

        for member in members {
            member_entries.push(StdlibMember { name: member.name, strategy: member.strategy });
            if let Some(type_) = &member.type_ {
                interface.functions.insert(member.name.into(), type_.clone());
            }
        }

        for type_ in types {
            interface.types.insert(type_.name.clone(), type_.clone());
            for constructor in &type_.constructors {
                interface
                    .constructors
                    .insert(constructor.name.clone(), constructor.clone());
            }
        }

        Self { name, strategy, interface, members: member_entries }
    }
}

#[derive(Debug, Clone)]
struct StdlibMemberSpec {
    name: &'static str,
    strategy: MemberStrategy,
    type_: Option<Type>,
}

fn function(name: &'static str, params: Vec<Type>, return_type: Type, strategy: MemberStrategy) -> StdlibMemberSpec {
    StdlibMemberSpec { name, strategy, type_: Some(fn_type(params, return_type)) }
}

fn constructor_member(name: &'static str) -> StdlibMemberSpec {
    StdlibMemberSpec { name, strategy: MemberStrategy::ManagedConstructor, type_: None }
}

fn type_decl(name: &str, parameters: Vec<&str>, opaque: bool, constructors: Vec<ConstructorInfo>) -> TypeDeclaration {
    TypeDeclaration {
        name: name.into(),
        parameters: parameters.into_iter().map(str::to_string).collect(),
        opaque,
        constructors,
        span: STDLIB_SPAN,
    }
}

fn constructor(name: &str, fields: Vec<FieldInfo>, return_type: Type) -> ConstructorInfo {
    ConstructorInfo { name: name.into(), fields, return_type, span: STDLIB_SPAN }
}

fn field(name: &str, type_: Type) -> FieldInfo {
    FieldInfo { name: name.into(), type_ }
}

fn fn_type(params: Vec<Type>, return_type: Type) -> Type {
    Type::Function { params, return_type: Box::new(return_type) }
}

fn list(item: Type) -> Type {
    Type::List(Box::new(item))
}

fn result(ok: Type, error: Type) -> Type {
    Type::custom("Result", vec![ok, error])
}

fn option(item: Type) -> Type {
    Type::custom("Option", vec![item])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_initial_modules_and_member_strategies() {
        let registry = StdlibRegistry::new();

        assert_eq!(
            registry.member_strategy("gleam/io", "println"),
            Some(MemberStrategy::HostImport),
        );
        assert_eq!(
            registry.member_strategy("gleam/int", "to_string"),
            Some(MemberStrategy::Intrinsic),
        );
        assert_eq!(
            registry.member_strategy("gleam/result", "Ok"),
            Some(MemberStrategy::ManagedConstructor),
        );
    }

    #[test]
    fn exposes_initial_interfaces() {
        let registry = StdlibRegistry::new();
        let interface = registry.interface("gleam/string").expect("string interface");

        assert_eq!(
            interface.functions.get("append"),
            Some(&fn_type(vec![Type::String, Type::String], Type::String)),
        );
    }

    #[test]
    fn records_remaining_stdlib_strategy() {
        let registry = StdlibRegistry::new();
        let module = registry.module("gleam/dict").expect("dict module");

        assert_eq!(module.strategy, ModuleStrategy::PreferCompiledSource);
    }
}
