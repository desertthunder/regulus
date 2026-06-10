/// Shared helpers for preserving function parameter labels and mapping labelled
/// call arguments onto parameter positions across compiler phases.
use std::collections::HashMap;

use crate::{
    ast::{self, Declaration},
    source::Span,
};

pub type ParameterLabels = Vec<Option<String>>;
pub type FunctionLabelMap = HashMap<String, ParameterLabels>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgumentOrder {
    pub indices: Vec<usize>,
    pub has_labels: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseCallbackPlacement {
    pub argument_indices: Vec<usize>,
    pub callback_index: usize,
    pub has_labels: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgumentLabelError {
    UnknownLabel { label: String, span: Span },
    DuplicateLabel { label: String, span: Span },
    TooManyArguments { span: Span },
}

pub fn function_label_map(module: &ast::Module) -> FunctionLabelMap {
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

pub fn call_argument_order(
    labels: Option<&[Option<String>]>, arguments: &[ast::Argument], param_count: usize,
) -> Result<CallArgumentOrder, ArgumentLabelError> {
    let Some(labels) = labels else {
        return Ok(CallArgumentOrder { indices: (0..arguments.len()).collect(), has_labels: false });
    };
    if arguments.iter().all(|argument| argument.label.is_none()) {
        return Ok(CallArgumentOrder { indices: (0..arguments.len()).collect(), has_labels: false });
    }

    let mut occupied = vec![false; param_count];
    let mut indices = Vec::with_capacity(arguments.len());
    for (fallback, argument) in arguments.iter().enumerate() {
        let index = match &argument.label {
            Some(label) => labelled_argument_index(labels, &occupied, label, argument.span)?,
            None => occupied
                .iter()
                .position(|occupied| !occupied)
                .or_else(|| (fallback < param_count).then_some(fallback))
                .ok_or(ArgumentLabelError::TooManyArguments { span: argument.span })?,
        };
        occupied[index] = true;
        indices.push(index);
    }
    Ok(CallArgumentOrder { indices, has_labels: true })
}

pub fn use_callback_placement(
    labels: Option<&[Option<String>]>, arguments: &[ast::Argument], param_count: usize,
) -> Result<UseCallbackPlacement, ArgumentLabelError> {
    let order = call_argument_order(labels, arguments, param_count)?;
    let mut occupied = vec![false; param_count];
    for index in &order.indices {
        if let Some(slot) = occupied.get_mut(*index) {
            *slot = true;
        }
    }
    let callback_index = occupied
        .iter()
        .position(|occupied| !occupied)
        .unwrap_or(arguments.len());
    Ok(UseCallbackPlacement { argument_indices: order.indices, callback_index, has_labels: order.has_labels })
}

fn labelled_argument_index(
    labels: &[Option<String>], occupied: &[bool], label: &ast::Name, span: Span,
) -> Result<usize, ArgumentLabelError> {
    if let Some(index) = labels
        .iter()
        .enumerate()
        .find_map(|(index, param_label)| (param_label.as_deref() == Some(label.text.as_str())).then_some(index))
    {
        if occupied.get(index).copied().unwrap_or(true) {
            Err(ArgumentLabelError::DuplicateLabel { label: label.text.clone(), span })
        } else {
            Ok(index)
        }
    } else {
        Err(ArgumentLabelError::UnknownLabel { label: label.text.clone(), span })
    }
}
