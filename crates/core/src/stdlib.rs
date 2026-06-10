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
        StdlibModule::gleam_bit_array(),
        StdlibModule::gleam_bool(),
        StdlibModule::remaining("gleam/bytes_tree"),
        StdlibModule::gleam_dict(),
        StdlibModule::gleam_dynamic(),
        StdlibModule::gleam_dynamic_decode(),
        StdlibModule::gleam_float(),
        StdlibModule::gleam_function(),
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
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "fold",
                    vec![
                        list(Type::generic("a")),
                        Type::generic("b"),
                        fn_type(vec![Type::generic("b"), Type::generic("a")], Type::generic("b")),
                    ],
                    Type::generic("b"),
                    MemberStrategy::Intrinsic,
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
                    MemberStrategy::Intrinsic,
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
                    MemberStrategy::Intrinsic,
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

    fn gleam_bit_array() -> Self {
        Self::new(
            "gleam/bit_array",
            ModuleStrategy::Hybrid,
            &[
                function(
                    "append",
                    vec![Type::BitArray, Type::BitArray],
                    Type::BitArray,
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "concat",
                    vec![Type::List(Box::new(Type::BitArray))],
                    Type::BitArray,
                    MemberStrategy::Intrinsic,
                ),
                function("bit_size", vec![Type::BitArray], Type::Int, MemberStrategy::Intrinsic),
                function("byte_size", vec![Type::BitArray], Type::Int, MemberStrategy::Intrinsic),
                function("is_empty", vec![Type::BitArray], Type::Bool, MemberStrategy::Intrinsic),
                function(
                    "starts_with",
                    vec![Type::BitArray, Type::BitArray],
                    Type::Bool,
                    MemberStrategy::Intrinsic,
                ),
            ],
            &[],
        )
    }

    fn gleam_bool() -> Self {
        Self::new(
            "gleam/bool",
            ModuleStrategy::Hybrid,
            &[
                function("to_string", vec![Type::Bool], Type::String, MemberStrategy::Intrinsic),
                function("negate", vec![Type::Bool], Type::Bool, MemberStrategy::Intrinsic),
                function(
                    "compare",
                    vec![Type::Bool, Type::Bool],
                    Type::custom("Order", vec![]),
                    MemberStrategy::Intrinsic,
                ),
            ],
            &[],
        )
    }

    fn gleam_dict() -> Self {
        let dict_type = type_decl("Dict", vec!["k", "v"], true, vec![]);
        let dict_kv = dict(Type::generic("k"), Type::generic("v"));
        Self::new(
            "gleam/dict",
            ModuleStrategy::Hybrid,
            &[
                function("new", vec![], dict_kv.clone(), MemberStrategy::Intrinsic),
                function("size", vec![dict_kv.clone()], Type::Int, MemberStrategy::Intrinsic),
                function("is_empty", vec![dict_kv.clone()], Type::Bool, MemberStrategy::Intrinsic),
                function(
                    "insert",
                    vec![dict_kv.clone(), Type::generic("k"), Type::generic("v")],
                    dict_kv.clone(),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "get",
                    vec![dict_kv.clone(), Type::generic("k")],
                    option(Type::generic("v")),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "has_key",
                    vec![dict_kv.clone(), Type::generic("k")],
                    Type::Bool,
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "delete",
                    vec![dict_kv.clone(), Type::generic("k")],
                    dict_kv,
                    MemberStrategy::Intrinsic,
                ),
            ],
            &[dict_type],
        )
    }

    fn gleam_dynamic() -> Self {
        let dynamic_type = type_decl("Dynamic", vec![], true, vec![]);
        Self::new(
            "gleam/dynamic",
            ModuleStrategy::Hybrid,
            &[
                function("array", vec![list(dynamic())], dynamic(), MemberStrategy::Intrinsic),
                function("bit_array", vec![Type::BitArray], dynamic(), MemberStrategy::Intrinsic),
                function("bool", vec![Type::Bool], dynamic(), MemberStrategy::Intrinsic),
                function("classify", vec![dynamic()], Type::String, MemberStrategy::Intrinsic),
                function("float", vec![Type::Float], dynamic(), MemberStrategy::Intrinsic),
                function("int", vec![Type::Int], dynamic(), MemberStrategy::Intrinsic),
                function("list", vec![list(dynamic())], dynamic(), MemberStrategy::Intrinsic),
                function("nil", vec![], dynamic(), MemberStrategy::Intrinsic),
                function(
                    "properties",
                    vec![list(Type::Tuple(vec![dynamic(), dynamic()]))],
                    dynamic(),
                    MemberStrategy::Intrinsic,
                ),
                function("string", vec![Type::String], dynamic(), MemberStrategy::Intrinsic),
            ],
            &[dynamic_type],
        )
    }

    fn gleam_dynamic_decode() -> Self {
        let decoder_type = type_decl("Decoder", vec!["t"], true, vec![]);
        let decode_error_type = type_decl(
            "DecodeError",
            vec![],
            false,
            vec![constructor(
                "DecodeError",
                vec![
                    field("expected", Type::String),
                    field("found", Type::String),
                    field("path", list(Type::String)),
                ],
                decode_error(),
            )],
        );
        Self::new(
            "gleam/dynamic/decode",
            ModuleStrategy::Hybrid,
            &[
                value("bit_array", decoder(Type::BitArray), MemberStrategy::Intrinsic),
                value("bool", decoder(Type::Bool), MemberStrategy::Intrinsic),
                value("dynamic", decoder(dynamic()), MemberStrategy::Intrinsic),
                value("float", decoder(Type::Float), MemberStrategy::Intrinsic),
                value("int", decoder(Type::Int), MemberStrategy::Intrinsic),
                value("string", decoder(Type::String), MemberStrategy::Intrinsic),
                function(
                    "run",
                    vec![dynamic(), decoder(Type::generic("t"))],
                    result(Type::generic("t"), list(decode_error())),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "list",
                    vec![decoder(Type::generic("a"))],
                    decoder(list(Type::generic("a"))),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "optional",
                    vec![decoder(Type::generic("a"))],
                    decoder(option(Type::generic("a"))),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "dict",
                    vec![decoder(Type::generic("k")), decoder(Type::generic("v"))],
                    decoder(dict(Type::generic("k"), Type::generic("v"))),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "at",
                    vec![list(Type::generic("segment")), decoder(Type::generic("a"))],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "optionally_at",
                    vec![
                        list(Type::generic("segment")),
                        Type::generic("a"),
                        decoder(Type::generic("a")),
                    ],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "field",
                    vec![
                        Type::generic("name"),
                        decoder(Type::generic("t")),
                        fn_type(vec![Type::generic("t")], decoder(Type::generic("final"))),
                    ],
                    decoder(Type::generic("final")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "optional_field",
                    vec![
                        Type::generic("name"),
                        Type::generic("t"),
                        decoder(Type::generic("t")),
                        fn_type(vec![Type::generic("t")], decoder(Type::generic("final"))),
                    ],
                    decoder(Type::generic("final")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "subfield",
                    vec![
                        list(Type::generic("name")),
                        decoder(Type::generic("t")),
                        fn_type(vec![Type::generic("t")], decoder(Type::generic("final"))),
                    ],
                    decoder(Type::generic("final")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "success",
                    vec![Type::generic("t")],
                    decoder(Type::generic("t")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "failure",
                    vec![Type::generic("a"), Type::String],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "map",
                    vec![
                        decoder(Type::generic("a")),
                        fn_type(vec![Type::generic("a")], Type::generic("b")),
                    ],
                    decoder(Type::generic("b")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "then",
                    vec![
                        decoder(Type::generic("a")),
                        fn_type(vec![Type::generic("a")], decoder(Type::generic("b"))),
                    ],
                    decoder(Type::generic("b")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "one_of",
                    vec![decoder(Type::generic("a")), list(decoder(Type::generic("a")))],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "collapse_errors",
                    vec![decoder(Type::generic("a")), Type::String],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "map_errors",
                    vec![
                        decoder(Type::generic("a")),
                        fn_type(vec![list(decode_error())], list(decode_error())),
                    ],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "recursive",
                    vec![fn_type(vec![], decoder(Type::generic("a")))],
                    decoder(Type::generic("a")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "new_primitive_decoder",
                    vec![
                        Type::String,
                        fn_type(vec![dynamic()], result(Type::generic("t"), Type::generic("t"))),
                    ],
                    decoder(Type::generic("t")),
                    MemberStrategy::InterfaceOnly,
                ),
                function(
                    "decode_error",
                    vec![Type::String, dynamic()],
                    list(decode_error()),
                    MemberStrategy::InterfaceOnly,
                ),
            ],
            &[decoder_type, decode_error_type],
        )
    }

    fn gleam_float() -> Self {
        Self::new(
            "gleam/float",
            ModuleStrategy::Hybrid,
            &[
                function(
                    "compare",
                    vec![Type::Float, Type::Float],
                    Type::custom("Order", vec![]),
                    MemberStrategy::Intrinsic,
                ),
                function("to_string", vec![Type::Float], Type::String, MemberStrategy::Intrinsic),
                function(
                    "max",
                    vec![Type::Float, Type::Float],
                    Type::Float,
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "min",
                    vec![Type::Float, Type::Float],
                    Type::Float,
                    MemberStrategy::Intrinsic,
                ),
                function("negate", vec![Type::Float], Type::Float, MemberStrategy::Intrinsic),
            ],
            &[],
        )
    }

    fn gleam_function() -> Self {
        Self::new(
            "gleam/function",
            ModuleStrategy::Hybrid,
            &[
                function(
                    "identity",
                    vec![Type::generic("a")],
                    Type::generic("a"),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "constant",
                    vec![Type::generic("a"), Type::generic("b")],
                    Type::generic("a"),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "compose",
                    vec![
                        fn_type(vec![Type::generic("b")], Type::generic("c")),
                        fn_type(vec![Type::generic("a")], Type::generic("b")),
                    ],
                    fn_type(vec![Type::generic("a")], Type::generic("c")),
                    MemberStrategy::Intrinsic,
                ),
                function(
                    "flip",
                    vec![fn_type(
                        vec![Type::generic("a"), Type::generic("b")],
                        Type::generic("c"),
                    )],
                    fn_type(vec![Type::generic("b"), Type::generic("a")], Type::generic("c")),
                    MemberStrategy::Intrinsic,
                ),
            ],
            &[],
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

fn value(name: &'static str, type_: Type, strategy: MemberStrategy) -> StdlibMemberSpec {
    StdlibMemberSpec { name, strategy, type_: Some(type_) }
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

fn dict(key: Type, value: Type) -> Type {
    Type::custom("Dict", vec![key, value])
}

fn dynamic() -> Type {
    Type::custom("Dynamic", vec![])
}

fn decoder(item: Type) -> Type {
    Type::custom("Decoder", vec![item])
}

fn decode_error() -> Type {
    Type::custom("DecodeError", vec![])
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
        let module = registry.module("gleam/bytes_tree").expect("bytes_tree module");

        assert_eq!(module.strategy, ModuleStrategy::PreferCompiledSource);
    }
}
