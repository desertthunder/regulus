use crate::source::Span;

use super::substitutions::{Field, InferenceVariable, Substitutions, TypeTerm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnificationError {
    Mismatch {
        expected: Box<TypeTerm>,
        actual: Box<TypeTerm>,
        span: Option<Span>,
    },
    ArityMismatch {
        expected: usize,
        actual: usize,
        span: Option<Span>,
    },
    FieldMismatch {
        field: String,
        span: Option<Span>,
    },
    OccursCheck {
        variable: InferenceVariable,
        type_: Box<TypeTerm>,
        span: Option<Span>,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Unifier {
    substitutions: Substitutions,
}

impl Unifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_substitutions(substitutions: Substitutions) -> Self {
        Self { substitutions }
    }

    pub fn substitutions(&self) -> &Substitutions {
        &self.substitutions
    }

    pub fn into_substitutions(self) -> Substitutions {
        self.substitutions
    }

    pub fn unify(
        &mut self, expected: &TypeTerm, actual: &TypeTerm, span: Option<Span>,
    ) -> Result<TypeTerm, UnificationError> {
        let expected = self.substitutions.walk(expected);
        let actual = self.substitutions.walk(actual);
        self.unify_walked(expected, actual, span)
    }

    fn unify_walked(
        &mut self, expected: TypeTerm, actual: TypeTerm, span: Option<Span>,
    ) -> Result<TypeTerm, UnificationError> {
        match (expected, actual) {
            (TypeTerm::Variable(variable), type_) | (type_, TypeTerm::Variable(variable)) => {
                self.bind_variable(variable, &type_, span)
            }
            (TypeTerm::Int, TypeTerm::Int) => Ok(TypeTerm::Int),
            (TypeTerm::Float, TypeTerm::Float) => Ok(TypeTerm::Float),
            (TypeTerm::String, TypeTerm::String) => Ok(TypeTerm::String),
            (TypeTerm::BitArray, TypeTerm::BitArray) => Ok(TypeTerm::BitArray),
            (TypeTerm::Bool, TypeTerm::Bool) => Ok(TypeTerm::Bool),
            (TypeTerm::Nil, TypeTerm::Nil) => Ok(TypeTerm::Nil),
            (TypeTerm::Generic(left), TypeTerm::Generic(right)) if left == right => Ok(TypeTerm::Generic(left)),
            (TypeTerm::Tuple(expected), TypeTerm::Tuple(actual)) => {
                Ok(TypeTerm::Tuple(self.unify_many(expected, actual, span)?))
            }
            (TypeTerm::List(expected), TypeTerm::List(actual)) => {
                Ok(TypeTerm::List(Box::new(self.unify(&expected, &actual, span)?)))
            }
            (
                TypeTerm::Custom { name: expected_name, args: expected_args },
                TypeTerm::Custom { name: actual_name, args: actual_args },
            ) if expected_name == actual_name => {
                Ok(TypeTerm::Custom { name: expected_name, args: self.unify_many(expected_args, actual_args, span)? })
            }
            (
                TypeTerm::Opaque { name: expected_name, args: expected_args },
                TypeTerm::Opaque { name: actual_name, args: actual_args },
            ) if expected_name == actual_name => {
                Ok(TypeTerm::Opaque { name: expected_name, args: self.unify_many(expected_args, actual_args, span)? })
            }
            (
                TypeTerm::Function { params: expected_params, return_type: expected_return },
                TypeTerm::Function { params: actual_params, return_type: actual_return },
            ) => Ok(TypeTerm::Function {
                params: self.unify_many(expected_params, actual_params, span)?,
                return_type: Box::new(self.unify(&expected_return, &actual_return, span)?),
            }),
            (
                TypeTerm::Record { name: expected_name, fields: expected_fields },
                TypeTerm::Record { name: actual_name, fields: actual_fields },
            ) if expected_name == actual_name => Ok(TypeTerm::Record {
                name: expected_name,
                fields: self.unify_fields(expected_fields, &actual_fields, span)?,
            }),
            (expected, actual) => {
                Err(UnificationError::Mismatch { expected: Box::new(expected), actual: Box::new(actual), span })
            }
        }
    }

    fn bind_variable(
        &mut self, variable: InferenceVariable, type_: &TypeTerm, span: Option<Span>,
    ) -> Result<TypeTerm, UnificationError> {
        let type_ = self.substitutions.walk(type_);
        if type_ == TypeTerm::Variable(variable) {
            return Ok(type_);
        }
        if type_.contains_variable(variable) {
            return Err(UnificationError::OccursCheck { variable, type_: Box::new(type_), span });
        }
        self.substitutions.insert(variable, type_.clone());
        Ok(type_)
    }

    fn unify_many(
        &mut self, expected: Vec<TypeTerm>, actual: Vec<TypeTerm>, span: Option<Span>,
    ) -> Result<Vec<TypeTerm>, UnificationError> {
        if expected.len() != actual.len() {
            return Err(UnificationError::ArityMismatch { expected: expected.len(), actual: actual.len(), span });
        }
        expected
            .into_iter()
            .zip(actual)
            .map(|(expected, actual)| self.unify(&expected, &actual, span))
            .collect()
    }

    fn unify_fields(
        &mut self, expected: Vec<Field>, actual: &[Field], span: Option<Span>,
    ) -> Result<Vec<Field>, UnificationError> {
        if expected.len() != actual.len() {
            return Err(UnificationError::ArityMismatch { expected: expected.len(), actual: actual.len(), span });
        }

        let mut unified = Vec::new();
        for expected_field in expected {
            let Some(actual_field) = actual.iter().find(|field| field.name == expected_field.name) else {
                return Err(UnificationError::FieldMismatch { field: expected_field.name, span });
            };
            unified.push(Field {
                name: expected_field.name,
                type_: self.unify(&expected_field.type_, &actual_field.type_, span)?,
            });
        }
        Ok(unified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unifies_inference_variable_with_scalar() {
        let variable = InferenceVariable(0);
        let mut unifier = Unifier::new();

        unifier
            .unify(&TypeTerm::Variable(variable), &TypeTerm::Int, None)
            .expect("unifies");

        assert_eq!(
            unifier.substitutions().walk(&TypeTerm::Variable(variable)),
            TypeTerm::Int
        );
    }

    #[test]
    fn rejects_infinite_types() {
        let variable = InferenceVariable(0);
        let mut unifier = Unifier::new();
        let recursive = TypeTerm::List(Box::new(TypeTerm::Variable(variable)));

        let error = unifier
            .unify(&TypeTerm::Variable(variable), &recursive, None)
            .expect_err("occurs check fails");

        assert!(matches!(
            error,
            UnificationError::OccursCheck { variable: InferenceVariable(0), .. }
        ));
    }

    #[test]
    fn unifies_function_types_recursively() {
        let variable = InferenceVariable(0);
        let mut unifier = Unifier::new();
        let expected =
            TypeTerm::Function { params: vec![TypeTerm::Variable(variable)], return_type: Box::new(TypeTerm::Bool) };
        let actual = TypeTerm::Function { params: vec![TypeTerm::String], return_type: Box::new(TypeTerm::Bool) };

        unifier.unify(&expected, &actual, None).expect("unifies");

        assert_eq!(
            unifier.substitutions().walk(&TypeTerm::Variable(variable)),
            TypeTerm::String
        );
    }
}
