use std::collections::{BTreeSet, HashMap};

use crate::types::{FieldInfo, Type};

/// A solver-created unknown type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InferenceVariable(pub u64);

/// A field in an inferred record shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub type_: TypeTerm,
}

/// Type syntax used by the inference solver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeTerm {
    Anything,
    Int,
    Float,
    String,
    BitArray,
    Bool,
    Nil,
    Tuple(Vec<TypeTerm>),
    List(Box<TypeTerm>),
    Record {
        name: String,
        fields: Vec<Field>,
    },
    Custom {
        name: String,
        args: Vec<TypeTerm>,
    },
    Generic(String),
    Opaque {
        name: String,
        args: Vec<TypeTerm>,
    },
    Function {
        params: Vec<TypeTerm>,
        return_type: Box<TypeTerm>,
    },
    Variable(InferenceVariable),
}

/// Solved inference variables.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Substitutions {
    variables: HashMap<InferenceVariable, TypeTerm>,
}

impl Substitutions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, variable: InferenceVariable, type_: TypeTerm) {
        self.variables.insert(variable, type_);
    }

    pub fn get(&self, variable: InferenceVariable) -> Option<&TypeTerm> {
        self.variables.get(&variable)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&InferenceVariable, &TypeTerm)> {
        self.variables.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn walk(&self, type_: &TypeTerm) -> TypeTerm {
        match type_ {
            TypeTerm::Variable(variable) => self
                .variables
                .get(variable)
                .map(|type_| self.walk(type_))
                .unwrap_or_else(|| type_.clone()),
            TypeTerm::Tuple(items) => TypeTerm::Tuple(items.iter().map(|item| self.walk(item)).collect()),
            TypeTerm::List(item) => TypeTerm::List(Box::new(self.walk(item))),
            TypeTerm::Record { name, fields } => TypeTerm::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| Field { name: field.name.clone(), type_: self.walk(&field.type_) })
                    .collect(),
            },
            TypeTerm::Custom { name, args } => {
                TypeTerm::Custom { name: name.clone(), args: args.iter().map(|arg| self.walk(arg)).collect() }
            }
            TypeTerm::Opaque { name, args } => {
                TypeTerm::Opaque { name: name.clone(), args: args.iter().map(|arg| self.walk(arg)).collect() }
            }
            TypeTerm::Function { params, return_type } => TypeTerm::Function {
                params: params.iter().map(|param| self.walk(param)).collect(),
                return_type: Box::new(self.walk(return_type)),
            },
            _ => type_.clone(),
        }
    }

    pub fn apply(&self, type_: &mut TypeTerm) {
        *type_ = self.walk(type_);
    }
}

impl TypeTerm {
    pub fn free_variables(&self) -> BTreeSet<InferenceVariable> {
        let mut variables = BTreeSet::new();
        self.collect_free_variables(&mut variables);
        variables
    }

    pub fn contains_variable(&self, variable: InferenceVariable) -> bool {
        self.free_variables().contains(&variable)
    }

    pub fn from_type(type_: &Type) -> Self {
        match type_ {
            Type::Anything => Self::Anything,
            Type::Int => Self::Int,
            Type::Float => Self::Float,
            Type::String => Self::String,
            Type::BitArray => Self::BitArray,
            Type::Bool => Self::Bool,
            Type::Nil => Self::Nil,
            Type::Tuple(items) => Self::Tuple(items.iter().map(Self::from_type).collect()),
            Type::List(item) => Self::List(Box::new(Self::from_type(item))),
            Type::Record { name, fields } => Self::Record {
                name: name.clone(),
                fields: fields
                    .iter()
                    .map(|field| Field { name: field.name.clone(), type_: Self::from_type(&field.type_) })
                    .collect(),
            },
            Type::Custom { name, args } => {
                Self::Custom { name: name.clone(), args: args.iter().map(Self::from_type).collect() }
            }
            Type::Generic(name) => Self::Generic(name.clone()),
            Type::Opaque { name, args } => {
                Self::Opaque { name: name.clone(), args: args.iter().map(Self::from_type).collect() }
            }
            Type::Function { params, return_type } => Self::Function {
                params: params.iter().map(Self::from_type).collect(),
                return_type: Box::new(Self::from_type(return_type)),
            },
        }
    }

    pub fn into_type(self) -> Option<Type> {
        match self {
            Self::Anything => Some(Type::Anything),
            Self::Int => Some(Type::Int),
            Self::Float => Some(Type::Float),
            Self::String => Some(Type::String),
            Self::BitArray => Some(Type::BitArray),
            Self::Bool => Some(Type::Bool),
            Self::Nil => Some(Type::Nil),
            Self::Tuple(items) => Some(Type::Tuple(
                items.into_iter().map(Self::into_type).collect::<Option<Vec<_>>>()?,
            )),
            Self::List(item) => Some(Type::List(Box::new(item.into_type()?))),
            Self::Record { name, fields } => Some(Type::Record {
                name,
                fields: fields
                    .into_iter()
                    .map(|field| Some(FieldInfo::new(field.name, field.type_.into_type()?)))
                    .collect::<Option<Vec<_>>>()?,
            }),
            Self::Custom { name, args } => {
                Some(Type::Custom { name, args: args.into_iter().map(Self::into_type).collect::<Option<Vec<_>>>()? })
            }
            Self::Generic(name) => Some(Type::Generic(name)),
            Self::Opaque { name, args } => {
                Some(Type::Opaque { name, args: args.into_iter().map(Self::into_type).collect::<Option<Vec<_>>>()? })
            }
            Self::Function { params, return_type } => Some(Type::Function {
                params: params.into_iter().map(Self::into_type).collect::<Option<Vec<_>>>()?,
                return_type: Box::new(return_type.into_type()?),
            }),
            Self::Variable(_) => None,
        }
    }

    fn collect_free_variables(&self, variables: &mut BTreeSet<InferenceVariable>) {
        match self {
            Self::Variable(variable) => {
                variables.insert(*variable);
            }
            Self::Tuple(items) => items.iter().for_each(|item| item.collect_free_variables(variables)),
            Self::List(item) => item.collect_free_variables(variables),
            Self::Record { fields, .. } => fields
                .iter()
                .for_each(|field| field.type_.collect_free_variables(variables)),
            Self::Custom { args, .. } | Self::Opaque { args, .. } => {
                args.iter().for_each(|arg| arg.collect_free_variables(variables));
            }
            Self::Function { params, return_type } => {
                params.iter().for_each(|param| param.collect_free_variables(variables));
                return_type.collect_free_variables(variables);
            }
            _ => {}
        }
    }
}

impl From<&Type> for TypeTerm {
    fn from(type_: &Type) -> Self {
        Self::from_type(type_)
    }
}
