/// Shared helpers for preserving function parameter labels and mapping labelled
/// call arguments onto parameter positions across compiler phases.
use std::collections::HashMap;

use crate::ast::{self, Declaration};

pub(crate) type ParameterLabels = Vec<Option<String>>;
pub(crate) type FunctionLabelMap = HashMap<String, ParameterLabels>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UseCallbackPlacement {
    pub argument_indices: Vec<usize>,
    pub callback_index: usize,
    pub has_labels: bool,
}

pub(crate) fn function_label_map(module: &ast::Module) -> FunctionLabelMap {
    let mut labels = FunctionLabelMap::new();
    collect_function_labels(&module.declarations, &mut labels);
    labels
}

fn collect_function_labels(declarations: &[Declaration], labels: &mut FunctionLabelMap) {
    for declaration in declarations {
        match declaration {
            Declaration::Function(function) => {
                labels.insert(function.name.text.clone(), parameter_labels(&function.parameters));
            }
            Declaration::ExternalFunction(function) => {
                labels.insert(function.name.text.clone(), parameter_labels(&function.parameters));
            }
            Declaration::TargetGroup(group) => collect_function_labels(&group.declarations, labels),
            _ => {}
        }
    }
}

fn parameter_labels(parameters: &[ast::Parameter]) -> ParameterLabels {
    parameters
        .iter()
        .map(|parameter| parameter.label.as_ref().map(|label| label.text.clone()))
        .collect()
}

pub(crate) fn use_callback_placement(
    labels: Option<&[Option<String>]>, arguments: &[ast::Argument], param_count: usize,
) -> Option<UseCallbackPlacement> {
    let Some(labels) = labels else {
        return Some(UseCallbackPlacement {
            argument_indices: (0..arguments.len()).collect(),
            callback_index: arguments.len(),
            has_labels: false,
        });
    };

    let mut occupied = vec![false; param_count];
    let mut argument_indices = Vec::with_capacity(arguments.len());
    for (fallback, argument) in arguments.iter().enumerate() {
        let index = call_argument_index(labels, &occupied, argument)
            .or_else(|| (fallback < param_count).then_some(fallback))?;
        occupied[index] = true;
        argument_indices.push(index);
    }
    let callback_index = occupied.iter().position(|occupied| !occupied)?;
    Some(UseCallbackPlacement { argument_indices, callback_index, has_labels: true })
}

fn call_argument_index(labels: &[Option<String>], occupied: &[bool], argument: &ast::Argument) -> Option<usize> {
    if let Some(label) = &argument.label {
        labels
            .iter()
            .enumerate()
            .find(|(index, param_label)| {
                !occupied.get(*index).copied().unwrap_or(true) && param_label.as_deref() == Some(label.text.as_str())
            })
            .map(|(index, _)| index)
    } else {
        occupied.iter().position(|occupied| !occupied)
    }
}
