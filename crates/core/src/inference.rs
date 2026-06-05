//! Hindley-Milner style inference core.
//!
//! This module contains the reusable pieces needed by the type checker to move
//! from annotation checking to constraint-based inference. It deliberately keeps
//! inference variables separate from named generic parameters: named generics are
//! source-level `a`/`value` parameters, while inference variables are solver
//! placeholders created by the checker.

pub mod constraints;
pub mod generics;
pub mod interfaces;
pub mod substitutions;
pub mod unification;

pub use constraints::{
    Constraint, ConstraintGeneration, ConstraintGenerationError, ConstraintGenerator, ConstraintSet,
};
pub use generics::{Environment, Scheme, TypeVarSupply};
pub use interfaces::InferenceInterface;
pub use substitutions::{Field, InferenceVariable, Substitutions, TypeTerm};
pub use unification::{UnificationError, Unifier};
