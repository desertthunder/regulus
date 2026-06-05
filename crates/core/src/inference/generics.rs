use std::collections::{BTreeSet, HashMap};

use super::{InferenceVariable, Substitutions, TypeTerm};
use crate::types::Type;

#[derive(Debug, Clone, Default)]
pub struct TypeVarSupply {
    next: u64,
}

impl TypeVarSupply {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fresh(&mut self) -> InferenceVariable {
        let variable = InferenceVariable(self.next);
        self.next += 1;
        variable
    }

    pub fn fresh_type(&mut self) -> TypeTerm {
        TypeTerm::Variable(self.fresh())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub variables: Vec<InferenceVariable>,
    pub type_: TypeTerm,
}

impl Scheme {
    pub fn monomorphic(type_: TypeTerm) -> Self {
        Self { variables: Vec::new(), type_ }
    }

    pub fn from_type(type_: &Type) -> Self {
        Self::monomorphic(TypeTerm::from_type(type_))
    }

    pub fn constructor(params: Vec<TypeTerm>, return_type: TypeTerm) -> Self {
        Self::monomorphic(TypeTerm::Function { params, return_type: Box::new(return_type) })
    }

    pub fn generalize(type_: &TypeTerm, environment: &Environment, substitutions: &Substitutions) -> Self {
        let type_ = substitutions.walk(type_);
        let environment_variables = environment.free_variables(substitutions);
        let variables = type_
            .free_variables()
            .difference(&environment_variables)
            .copied()
            .collect();
        Self { variables, type_ }
    }

    pub fn generalize_top_level(type_: &TypeTerm, substitutions: &Substitutions) -> Self {
        let type_ = substitutions.walk(type_);
        Self { variables: type_.free_variables().into_iter().collect(), type_ }
    }

    pub fn instantiate(&self, supply: &mut TypeVarSupply) -> TypeTerm {
        let replacements = self
            .variables
            .iter()
            .map(|variable| (*variable, supply.fresh_type()))
            .collect::<HashMap<_, _>>();
        let type_ = replace_variables(&self.type_, &replacements);
        Self::instantiate_named_generics(&type_, supply)
    }

    /// Replace source-level named generics with fresh inference variables.
    pub fn instantiate_named_generics(type_: &TypeTerm, supply: &mut TypeVarSupply) -> TypeTerm {
        let mut replacements = HashMap::new();
        replace_named_generics(type_, supply, &mut replacements)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    values: HashMap<String, Scheme>,
    constructors: HashMap<String, Scheme>,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, scheme: Scheme) {
        self.values.insert(name.into(), scheme);
    }

    pub fn insert_constructor(&mut self, name: impl Into<String>, scheme: Scheme) {
        self.constructors.insert(name.into(), scheme);
    }

    pub fn get(&self, name: &str) -> Option<&Scheme> {
        self.values.get(name)
    }

    pub fn get_constructor(&self, name: &str) -> Option<&Scheme> {
        self.constructors.get(name)
    }

    pub fn generalize_value(&self, type_: &TypeTerm, substitutions: &Substitutions) -> Scheme {
        Scheme::generalize(type_, self, substitutions)
    }

    pub fn insert_generalized(&mut self, name: impl Into<String>, type_: &TypeTerm, substitutions: &Substitutions) {
        let scheme = self.generalize_value(type_, substitutions);
        self.insert(name, scheme);
    }

    pub fn free_variables(&self, substitutions: &Substitutions) -> BTreeSet<InferenceVariable> {
        let mut variables = BTreeSet::new();
        for scheme in self.values.values().chain(self.constructors.values()) {
            let quantified = scheme.variables.iter().copied().collect::<BTreeSet<_>>();
            for variable in substitutions.walk(&scheme.type_).free_variables() {
                if !quantified.contains(&variable) {
                    variables.insert(variable);
                }
            }
        }
        variables
    }
}

fn replace_variables(type_: &TypeTerm, replacements: &HashMap<InferenceVariable, TypeTerm>) -> TypeTerm {
    match type_ {
        TypeTerm::Variable(variable) => replacements.get(variable).cloned().unwrap_or_else(|| type_.clone()),
        TypeTerm::Tuple(items) => {
            TypeTerm::Tuple(items.iter().map(|item| replace_variables(item, replacements)).collect())
        }
        TypeTerm::List(item) => TypeTerm::List(Box::new(replace_variables(item, replacements))),
        TypeTerm::Record { name, fields } => TypeTerm::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| super::Field {
                    name: field.name.clone(),
                    type_: replace_variables(&field.type_, replacements),
                })
                .collect(),
        },
        TypeTerm::Custom { name, args } => TypeTerm::Custom {
            name: name.clone(),
            args: args.iter().map(|arg| replace_variables(arg, replacements)).collect(),
        },
        TypeTerm::Opaque { name, args } => TypeTerm::Opaque {
            name: name.clone(),
            args: args.iter().map(|arg| replace_variables(arg, replacements)).collect(),
        },
        TypeTerm::Function { params, return_type } => TypeTerm::Function {
            params: params
                .iter()
                .map(|param| replace_variables(param, replacements))
                .collect(),
            return_type: Box::new(replace_variables(return_type, replacements)),
        },
        _ => type_.clone(),
    }
}

fn replace_named_generics(
    type_: &TypeTerm, supply: &mut TypeVarSupply, replacements: &mut HashMap<String, TypeTerm>,
) -> TypeTerm {
    match type_ {
        TypeTerm::Generic(name) => replacements
            .entry(name.clone())
            .or_insert_with(|| supply.fresh_type())
            .clone(),
        TypeTerm::Tuple(items) => TypeTerm::Tuple(
            items
                .iter()
                .map(|item| replace_named_generics(item, supply, replacements))
                .collect(),
        ),
        TypeTerm::List(item) => TypeTerm::List(Box::new(replace_named_generics(item, supply, replacements))),
        TypeTerm::Record { name, fields } => TypeTerm::Record {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|field| super::Field {
                    name: field.name.clone(),
                    type_: replace_named_generics(&field.type_, supply, replacements),
                })
                .collect(),
        },
        TypeTerm::Custom { name, args } => TypeTerm::Custom {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| replace_named_generics(arg, supply, replacements))
                .collect(),
        },
        TypeTerm::Opaque { name, args } => TypeTerm::Opaque {
            name: name.clone(),
            args: args
                .iter()
                .map(|arg| replace_named_generics(arg, supply, replacements))
                .collect(),
        },
        TypeTerm::Function { params, return_type } => TypeTerm::Function {
            params: params
                .iter()
                .map(|param| replace_named_generics(param, supply, replacements))
                .collect(),
            return_type: Box::new(replace_named_generics(return_type, supply, replacements)),
        },
        _ => type_.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiates_named_generics_on_each_lookup() {
        let scheme = Scheme::from_type(&Type::Function {
            params: vec![Type::Generic("a".into())],
            return_type: Box::new(Type::Generic("a".into())),
        });
        let mut supply = TypeVarSupply::new();

        let first = scheme.instantiate(&mut supply);
        let second = scheme.instantiate(&mut supply);

        assert_ne!(first, second);
        let TypeTerm::Function { params, return_type } = first else { panic!("function") };
        assert_eq!(params[0], *return_type);
    }

    #[test]
    fn generalizes_top_level_free_variables() {
        let variable = InferenceVariable(0);
        let scheme = Scheme::generalize_top_level(
            &TypeTerm::Function {
                params: vec![TypeTerm::Variable(variable)],
                return_type: Box::new(TypeTerm::Variable(variable)),
            },
            &Substitutions::new(),
        );

        assert_eq!(scheme.variables, vec![variable]);
    }
}
