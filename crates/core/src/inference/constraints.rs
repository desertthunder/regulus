use crate::source::Span;

use super::{TypeTerm, Unifier, substitutions::Substitutions, unification::UnificationError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub expected: TypeTerm,
    pub actual: TypeTerm,
    pub span: Span,
}

impl Constraint {
    pub fn new(expected: TypeTerm, actual: TypeTerm, span: Span) -> Self {
        Self { expected, actual, span }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConstraintSet {
    constraints: Vec<Constraint>,
}

impl ConstraintSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, expected: TypeTerm, actual: TypeTerm, span: Span) {
        self.constraints.push(Constraint { expected, actual, span });
    }

    pub fn iter(&self) -> impl Iterator<Item = &Constraint> {
        self.constraints.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    pub fn solve(&self) -> Result<Substitutions, UnificationError> {
        let mut unifier = Unifier::new();
        for constraint in &self.constraints {
            unifier.unify(&constraint.expected, &constraint.actual, Some(constraint.span))?;
        }
        Ok(unifier.into_substitutions())
    }
}

impl IntoIterator for ConstraintSet {
    type Item = Constraint;
    type IntoIter = std::vec::IntoIter<Constraint>;

    fn into_iter(self) -> Self::IntoIter {
        self.constraints.into_iter()
    }
}
