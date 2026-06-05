use std::collections::HashMap;

use super::{Environment, Scheme, TypeTerm, TypeVarSupply};
use crate::types::{ConstructorInfo, ModuleInterface, TypeDeclaration};

/// Public inferred schemes exported by a module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InferenceInterface {
    pub values: HashMap<String, Scheme>,
    pub types: HashMap<String, TypeDeclaration>,
    pub constructors: HashMap<String, Scheme>,
}

impl InferenceInterface {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_module_interface(interface: &ModuleInterface) -> Self {
        let mut inferred = Self::new();
        inferred.types = interface.types.clone();
        for (name, type_) in &interface.functions {
            inferred.insert_value(name.clone(), Scheme::from_type(type_));
        }
        for (name, constructor) in &interface.constructors {
            inferred.insert_constructor(name.clone(), constructor_scheme(constructor));
        }
        inferred
    }

    pub fn insert_value(&mut self, name: impl Into<String>, scheme: Scheme) {
        self.values.insert(name.into(), scheme);
    }

    pub fn insert_constructor(&mut self, name: impl Into<String>, scheme: Scheme) {
        self.constructors.insert(name.into(), scheme);
    }

    pub fn value(&self, name: &str) -> Option<&Scheme> {
        self.values.get(name)
    }

    pub fn constructor(&self, name: &str) -> Option<&Scheme> {
        self.constructors.get(name)
    }

    pub fn to_environment(&self) -> Environment {
        let mut environment = Environment::new();
        self.add_to_environment(&mut environment);
        environment
    }

    pub fn add_to_environment(&self, environment: &mut Environment) {
        for (name, scheme) in &self.values {
            environment.insert(name.clone(), scheme.clone());
        }
        for (name, scheme) in &self.constructors {
            environment.insert_constructor(name.clone(), scheme.clone());
        }
    }

    pub fn add_imported_module_to_environment(&self, module_name: &str, environment: &mut Environment) {
        for (name, scheme) in &self.values {
            environment.insert(format!("{module_name}.{name}"), scheme.clone());
        }
        for (name, scheme) in &self.constructors {
            environment.insert_constructor(format!("{module_name}.{name}"), scheme.clone());
        }
    }

    pub fn instantiate_value(&self, name: &str, supply: &mut TypeVarSupply) -> Option<TypeTerm> {
        self.value(name).map(|scheme| scheme.instantiate(supply))
    }

    pub fn instantiate_constructor(&self, name: &str, supply: &mut TypeVarSupply) -> Option<TypeTerm> {
        self.constructor(name).map(|scheme| scheme.instantiate(supply))
    }
}

pub fn constructor_scheme(constructor: &ConstructorInfo) -> Scheme {
    Scheme::constructor(
        constructor
            .fields
            .iter()
            .map(|field| TypeTerm::from_type(&field.type_))
            .collect(),
        TypeTerm::from_type(&constructor.return_type),
    )
}

pub fn environment_from_interfaces(interfaces: &HashMap<String, InferenceInterface>) -> Environment {
    let mut environment = Environment::new();
    for (module_name, interface) in interfaces {
        interface.add_imported_module_to_environment(module_name, &mut environment);
    }
    environment
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        source::{SourceFileId, Span},
        types::{FieldInfo, Type},
    };

    fn span() -> Span {
        Span::new(SourceFileId(0), 0, 0)
    }

    #[test]
    fn stores_public_value_and_constructor_schemes() {
        let mut interface = ModuleInterface::default();
        interface.functions.insert(
            "identity".into(),
            Type::Function {
                params: vec![Type::Generic("a".into())],
                return_type: Box::new(Type::Generic("a".into())),
            },
        );
        interface.constructors.insert(
            "Box".into(),
            ConstructorInfo {
                name: "Box".into(),
                fields: vec![FieldInfo { name: "value".into(), type_: Type::Generic("a".into()) }],
                return_type: Type::Custom { name: "Box".into(), args: vec![Type::Generic("a".into())] },
                span: span(),
            },
        );

        let inferred = InferenceInterface::from_module_interface(&interface);

        assert!(inferred.value("identity").is_some());
        assert!(inferred.constructor("Box").is_some());
    }

    #[test]
    fn instantiates_imported_interface_schemes() {
        let mut interface = InferenceInterface::new();
        interface.insert_value(
            "identity",
            Scheme::from_type(&Type::Function {
                params: vec![Type::Generic("a".into())],
                return_type: Box::new(Type::Generic("a".into())),
            }),
        );
        let mut environment = Environment::new();
        interface.add_imported_module_to_environment("one", &mut environment);
        interface.add_imported_module_to_environment("two", &mut environment);
        let mut supply = TypeVarSupply::new();

        let one = environment.get("one.identity").expect("one").instantiate(&mut supply);
        let two = environment.get("two.identity").expect("two").instantiate(&mut supply);

        assert_ne!(one, two);
    }
}
