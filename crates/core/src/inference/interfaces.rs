use std::collections::HashMap;

use super::Scheme;
use crate::types::TypeDeclaration;

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
}
