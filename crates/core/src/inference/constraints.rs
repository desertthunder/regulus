use super::generics::{Environment, Scheme, TypeVarSupply};
use super::interfaces::constructor_scheme;
use super::substitutions::Substitutions;
use super::unification::UnificationError;
use super::{TypeTerm, Unifier};
use crate::ast::{self, Expression, LiteralKind, Pattern, Statement};
use crate::labels::{ArgumentLabelError, FunctionLabelMap, call_argument_order, use_callback_placement};
use crate::source::Span;
use crate::types::{ConstructorInfo, Type};
use std::collections::HashMap;

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

    pub fn solve(&self) -> std::result::Result<Substitutions, UnificationError> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintGenerationError {
    UnknownValue { name: String, span: Span },
    UnknownConstructor { name: String, span: Span },
    UnsupportedAnnotation { source: String, span: Span },
    TupleIndexOutOfBounds { index: usize, span: Span },
    ArgumentLabel(ArgumentLabelError),
}

pub type Result<T> = std::result::Result<T, ConstraintGenerationError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredExpression {
    pub span: Span,
    pub type_: TypeTerm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintGeneration {
    pub type_: TypeTerm,
    pub constraints: ConstraintSet,
    pub expressions: Vec<InferredExpression>,
    pub environment: Environment,
}

/// Generates expression and pattern constraints without solving them.
#[derive(Debug, Clone)]
pub struct ConstraintGenerator {
    supply: TypeVarSupply,
    environment: Environment,
    constructors: HashMap<String, ConstructorInfo>,
    function_labels: FunctionLabelMap,
    constraints: ConstraintSet,
    expressions: Vec<InferredExpression>,
    scopes: Vec<HashMap<String, Scheme>>,
}

impl ConstraintGenerator {
    pub fn new(environment: Environment) -> Self {
        Self {
            supply: TypeVarSupply::new(),
            environment,
            constructors: HashMap::new(),
            function_labels: HashMap::new(),
            constraints: ConstraintSet::new(),
            expressions: Vec::new(),
            scopes: vec![HashMap::new()],
        }
    }

    pub fn with_constructors(mut self, constructors: HashMap<String, ConstructorInfo>) -> Self {
        self.constructors = constructors;
        self
    }

    pub fn with_function_labels(mut self, function_labels: FunctionLabelMap) -> Self {
        self.function_labels = function_labels;
        self
    }

    pub fn finish(self, type_: TypeTerm) -> ConstraintGeneration {
        ConstraintGeneration {
            type_,
            constraints: self.constraints,
            expressions: self.expressions,
            environment: self.environment,
        }
    }

    pub fn infer_expression(&mut self, expression: &Expression) -> Result<TypeTerm> {
        let type_ = self.infer_expression_inner(expression)?;
        self.expressions
            .push(InferredExpression { span: expression_span(expression), type_: type_.clone() });
        Ok(type_)
    }

    fn infer_expression_inner(&mut self, expression: &Expression) -> Result<TypeTerm> {
        match expression {
            Expression::Literal(literal) => Ok(literal_type(&literal.kind)),
            Expression::Variable(name) => self.lookup_name(name),
            Expression::Call(call) => self.infer_call(call),
            Expression::FieldAccess(access) => self.infer_field_access(access),
            Expression::Block(block) => self.infer_block(block),
            Expression::Case(case) => self.infer_case(case),
            Expression::BinaryOperation(operation) => self.infer_binary_operation(operation),
            Expression::Pipeline(pipeline) => self.infer_pipeline(pipeline),
            Expression::UnaryOperation(operation) => self.infer_unary_operation(operation),
            Expression::Use(use_) => self.infer_use(use_, &[]),
            Expression::AnonymousFunction(function) => self.infer_anonymous_function(function),
            Expression::Capture(capture) => self.infer_capture(capture),
            Expression::Record(record) => self.infer_record(record),
            Expression::RecordUpdate(update) => self.infer_record_update(update),
            Expression::Tuple(tuple) => tuple
                .elements
                .iter()
                .map(|element| self.infer_expression(element))
                .collect::<Result<Vec<_>>>()
                .map(TypeTerm::Tuple),
            Expression::TupleAccess(access) => self.infer_tuple_access(access),
            Expression::List(list) => self.infer_list(list),
            Expression::BitArray(bit_array) => {
                for segment in &bit_array.segments {
                    self.infer_expression(&segment.value)?;
                    for option in &segment.options {
                        if let Some(value) = &option.value {
                            self.infer_expression(value)?;
                        }
                    }
                }
                Ok(TypeTerm::BitArray)
            }
            Expression::Panic(failure) | Expression::Todo(failure) => {
                if let Some(message) = &failure.message {
                    self.infer_expression(message)?;
                }
                Ok(self.supply.fresh_type())
            }
            Expression::Assert(assert) => {
                let value = self.infer_expression(&assert.value)?;
                let expected = self.annotation_or_fresh(assert.type_annotation.as_ref())?;
                self.constraints.push(expected.clone(), value, assert.span);
                self.bind_pattern(&assert.pattern, &expected)?;
                Ok(TypeTerm::Nil)
            }
            Expression::Echo(echo) => self.infer_expression(&echo.value),
            Expression::Raw(raw) => Ok(raw_type(raw.kind.as_str()).unwrap_or_else(|| self.supply.fresh_type())),
        }
    }

    pub fn infer_function(&mut self, function: &ast::Function) -> Result<TypeTerm> {
        self.push_scope();
        let mut params = Vec::new();
        for parameter in &function.parameters {
            let type_ = self.annotation_or_fresh(parameter.type_annotation.as_ref())?;
            if let Some(name) = &parameter.name {
                self.define(name.text.clone(), Scheme::monomorphic(type_.clone()));
            }
            params.push(type_);
        }
        let body = self.infer_block(&function.body)?;
        let return_type = match &function.return_type {
            Some(annotation) => self.annotation_or_fresh(Some(annotation))?,
            None => body.clone(),
        };
        self.constraints.push(return_type.clone(), body, function.body.span);
        self.pop_scope();
        Ok(TypeTerm::Function { params, return_type: Box::new(return_type) })
    }

    pub fn infer_block(&mut self, block: &ast::Block) -> Result<TypeTerm> {
        self.push_scope();
        let last = self.infer_statements(&block.statements)?;
        self.pop_scope();
        Ok(last)
    }

    fn infer_statements(&mut self, statements: &[Statement]) -> Result<TypeTerm> {
        let mut last = TypeTerm::Nil;
        for (index, statement) in statements.iter().enumerate() {
            last = match statement {
                Statement::Let(let_) => {
                    let value = self.infer_expression(&let_.value)?;
                    if let ast::Pattern::Name(name) = &let_.pattern
                        && let_.type_annotation.is_none()
                        && eligible_for_local_generalization(&let_.value)
                    {
                        self.define(
                            name.text.clone(),
                            Scheme::generalize_top_level(&value, &Substitutions::new()),
                        );
                    } else {
                        let expected = self.annotation_or_fresh(let_.type_annotation.as_ref())?;
                        self.constraints.push(expected.clone(), value, let_.span);
                        self.bind_pattern(&let_.pattern, &expected)?;
                    }
                    TypeTerm::Nil
                }
                Statement::LetAssert(let_assert) => {
                    let value = self.infer_expression(&let_assert.value)?;
                    let expected = self.annotation_or_fresh(let_assert.type_annotation.as_ref())?;
                    self.constraints.push(expected.clone(), value, let_assert.span);
                    if let Some(message) = &let_assert.message {
                        self.infer_expression(message)?;
                    }
                    self.bind_pattern(&let_assert.pattern, &expected)?;
                    TypeTerm::Nil
                }
                Statement::Expression(Expression::Use(use_)) => return self.infer_use(use_, &statements[index + 1..]),
                Statement::Expression(expression) => self.infer_expression(expression)?,
            };
        }
        Ok(last)
    }

    pub fn bind_pattern(&mut self, pattern: &Pattern, expected: &TypeTerm) -> Result<()> {
        match pattern {
            Pattern::Name(name) => self.define(name.text.clone(), Scheme::monomorphic(expected.clone())),
            Pattern::Discard(_) => {}
            Pattern::Integer(literal) => self.constraints.push(TypeTerm::Int, expected.clone(), literal.span),
            Pattern::Float(literal) => self.constraints.push(TypeTerm::Float, expected.clone(), literal.span),
            Pattern::String(literal) => self.constraints.push(TypeTerm::String, expected.clone(), literal.span),
            Pattern::Bool(literal) => self.constraints.push(TypeTerm::Bool, expected.clone(), literal.span),
            Pattern::Nil(literal) => self.constraints.push(TypeTerm::Nil, expected.clone(), literal.span),
            Pattern::Tuple(tuple) => {
                let elements = (0..tuple.elements.len())
                    .map(|_| self.supply.fresh_type())
                    .collect::<Vec<_>>();
                self.constraints
                    .push(TypeTerm::Tuple(elements.clone()), expected.clone(), tuple.span);
                for (pattern, element) in tuple.elements.iter().zip(elements.iter()) {
                    self.bind_pattern(pattern, element)?;
                }
            }
            Pattern::List(list) => {
                let element = self.supply.fresh_type();
                self.constraints
                    .push(TypeTerm::List(Box::new(element.clone())), expected.clone(), list.span);
                for pattern in &list.elements {
                    self.bind_pattern(pattern, &element)?;
                }
                if let Some(ast::ListPatternTail::Name(name)) = &list.tail {
                    self.define(
                        name.text.clone(),
                        Scheme::monomorphic(TypeTerm::List(Box::new(element))),
                    );
                }
            }
            Pattern::Constructor(pattern) => self.bind_constructor_pattern(pattern, expected)?,
            Pattern::Alias(alias) => {
                self.bind_pattern(&alias.pattern, expected)?;
                self.define(alias.alias.text.clone(), Scheme::monomorphic(expected.clone()));
            }
            Pattern::BitString(raw) => {
                self.constraints.push(TypeTerm::BitArray, expected.clone(), raw.span);
                for binding in bit_string_pattern_bindings(raw) {
                    self.define(binding.name.text, Scheme::monomorphic(binding.type_));
                }
            }
            Pattern::Raw(_) => {}
        }
        Ok(())
    }

    fn infer_call(&mut self, call: &ast::Call) -> Result<TypeTerm> {
        let function = self.infer_expression(&call.function)?;
        let params = self.infer_call_params(call)?;
        let return_type = self.supply.fresh_type();
        self.constraints.push(
            function,
            TypeTerm::Function { params, return_type: Box::new(return_type.clone()) },
            call.span,
        );
        Ok(return_type)
    }

    fn infer_call_params(&mut self, call: &ast::Call) -> Result<Vec<TypeTerm>> {
        let argument_types = call
            .arguments
            .iter()
            .map(|argument| self.infer_expression(&argument.value))
            .collect::<Result<Vec<_>>>()?;
        let param_count = self.call_function_labels(call).map_or(argument_types.len(), <[_]>::len);
        let order = call_argument_order(self.call_function_labels(call), &call.arguments, param_count)
            .map_err(ConstraintGenerationError::ArgumentLabel)?;
        if !order.has_labels {
            return Ok(argument_types);
        }
        let mut ordered = vec![None; param_count];
        for (index, type_) in order.indices.into_iter().zip(argument_types) {
            ordered[index] = Some(type_);
        }
        Ok(ordered.into_iter().flatten().collect())
    }

    fn infer_field_access(&mut self, access: &ast::FieldAccess) -> Result<TypeTerm> {
        if let Expression::Variable(module) = access.record.as_ref()
            && let Some(scheme) = self.environment.get(&format!("{}.{}", module.text, access.field.text))
        {
            return Ok(scheme.instantiate(&mut self.supply));
        }

        let record = self.infer_expression(&access.record)?;
        let field_type = self.supply.fresh_type();
        self.constraints.push(
            TypeTerm::Record {
                name: String::new(),
                fields: vec![super::Field { name: access.field.text.clone(), type_: field_type.clone() }],
            },
            record,
            access.span,
        );
        Ok(field_type)
    }

    fn infer_case(&mut self, case: &ast::Case) -> Result<TypeTerm> {
        let subjects = case
            .subjects
            .iter()
            .map(|subject| self.infer_expression(subject))
            .collect::<Result<Vec<_>>>()?;
        let result = self.supply.fresh_type();
        for clause in &case.clauses {
            self.push_scope();
            for (pattern, subject) in clause.patterns.iter().zip(subjects.iter()) {
                self.bind_pattern(pattern, subject)?;
            }
            if let Some(guard) = &clause.guard {
                let guard_type = self.infer_expression(guard)?;
                self.constraints
                    .push(TypeTerm::Bool, guard_type, expression_span(guard));
            }
            let branch = self.infer_expression(&clause.value)?;
            self.constraints
                .push(result.clone(), branch, expression_span(&clause.value));
            self.pop_scope();
        }
        Ok(result)
    }

    fn infer_binary_operation(&mut self, operation: &ast::BinaryOperation) -> Result<TypeTerm> {
        let left = self.infer_expression(&operation.left)?;
        let right = self.infer_expression(&operation.right)?;
        match operation.operator {
            ast::BinaryOperator::Add
            | ast::BinaryOperator::Subtract
            | ast::BinaryOperator::Multiply
            | ast::BinaryOperator::Divide
            | ast::BinaryOperator::Remainder => {
                self.constraints
                    .push(TypeTerm::Int, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::Int, right, expression_span(&operation.right));
                Ok(TypeTerm::Int)
            }
            ast::BinaryOperator::FloatAdd
            | ast::BinaryOperator::FloatSubtract
            | ast::BinaryOperator::FloatMultiply
            | ast::BinaryOperator::FloatDivide => {
                self.constraints
                    .push(TypeTerm::Float, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::Float, right, expression_span(&operation.right));
                Ok(TypeTerm::Float)
            }
            ast::BinaryOperator::And | ast::BinaryOperator::Or => {
                self.constraints
                    .push(TypeTerm::Bool, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::Bool, right, expression_span(&operation.right));
                Ok(TypeTerm::Bool)
            }
            ast::BinaryOperator::StringConcat => {
                self.constraints
                    .push(TypeTerm::String, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::String, right, expression_span(&operation.right));
                Ok(TypeTerm::String)
            }
            ast::BinaryOperator::Equal | ast::BinaryOperator::NotEqual => {
                self.constraints.push(left, right, expression_span(&operation.right));
                Ok(TypeTerm::Bool)
            }
            ast::BinaryOperator::LessThan
            | ast::BinaryOperator::LessThanEqual
            | ast::BinaryOperator::GreaterThan
            | ast::BinaryOperator::GreaterThanEqual => {
                self.constraints
                    .push(TypeTerm::Int, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::Int, right, expression_span(&operation.right));
                Ok(TypeTerm::Bool)
            }
            ast::BinaryOperator::FloatLessThan
            | ast::BinaryOperator::FloatLessThanEqual
            | ast::BinaryOperator::FloatGreaterThan
            | ast::BinaryOperator::FloatGreaterThanEqual => {
                self.constraints
                    .push(TypeTerm::Float, left, expression_span(&operation.left));
                self.constraints
                    .push(TypeTerm::Float, right, expression_span(&operation.right));
                Ok(TypeTerm::Bool)
            }
        }
    }

    fn infer_pipeline(&mut self, pipeline: &ast::Pipeline) -> Result<TypeTerm> {
        let input = self.infer_expression(&pipeline.value)?;
        let into = self.infer_expression(&pipeline.into)?;
        let return_type = self.supply.fresh_type();
        self.constraints.push(
            TypeTerm::Function { params: vec![input], return_type: Box::new(return_type.clone()) },
            into,
            pipeline.span,
        );
        Ok(return_type)
    }

    fn infer_unary_operation(&mut self, operation: &ast::UnaryOperation) -> Result<TypeTerm> {
        let value = self.infer_expression(&operation.value)?;
        match operation.operator {
            ast::UnaryOperator::BooleanNot => {
                self.constraints
                    .push(TypeTerm::Bool, value, expression_span(&operation.value));
                Ok(TypeTerm::Bool)
            }
            ast::UnaryOperator::IntegerNegate => {
                self.constraints
                    .push(TypeTerm::Int, value, expression_span(&operation.value));
                Ok(TypeTerm::Int)
            }
        }
    }

    fn infer_use(&mut self, use_: &ast::Use, continuation: &[Statement]) -> Result<TypeTerm> {
        let callback_params = use_
            .assignments
            .iter()
            .map(|assignment| self.annotation_or_fresh(assignment.type_annotation.as_ref()))
            .collect::<Result<Vec<_>>>()?;

        self.push_scope();
        for (assignment, type_) in use_.assignments.iter().zip(callback_params.iter()) {
            self.bind_pattern(&assignment.pattern, type_)?;
        }
        let callback_return = self.infer_statements(continuation)?;
        self.pop_scope();

        let callback = TypeTerm::Function { params: callback_params, return_type: Box::new(callback_return) };
        let return_type = self.supply.fresh_type();
        match use_.value.as_ref() {
            Expression::Call(call) => {
                let function = self.infer_expression(&call.function)?;
                let params = self.use_call_params(call, callback)?;
                self.constraints.push(
                    function,
                    TypeTerm::Function { params, return_type: Box::new(return_type.clone()) },
                    use_.span,
                );
            }
            value => {
                let function = self.infer_expression(value)?;
                self.constraints.push(
                    function,
                    TypeTerm::Function { params: vec![callback], return_type: Box::new(return_type.clone()) },
                    use_.span,
                );
            }
        }
        Ok(return_type)
    }

    fn use_call_params(&mut self, call: &ast::Call, callback: TypeTerm) -> Result<Vec<TypeTerm>> {
        let argument_types = call
            .arguments
            .iter()
            .map(|argument| self.infer_expression(&argument.value))
            .collect::<Result<Vec<_>>>()?;
        let param_count = self
            .call_function_labels(call)
            .map_or(argument_types.len() + 1, <[_]>::len);
        let placement = use_callback_placement(self.call_function_labels(call), &call.arguments, param_count)
            .map_err(ConstraintGenerationError::ArgumentLabel)?;
        if !placement.has_labels {
            let mut params = argument_types;
            params.push(callback);
            return Ok(params);
        }
        let mut ordered = vec![None; param_count];
        for (index, type_) in placement.argument_indices.into_iter().zip(argument_types) {
            ordered[index] = Some(type_);
        }
        if placement.callback_index >= ordered.len() {
            ordered.resize_with(placement.callback_index + 1, || None);
        }
        ordered[placement.callback_index] = Some(callback);
        Ok(ordered.into_iter().flatten().collect())
    }

    fn call_function_labels(&self, call: &ast::Call) -> Option<&[Option<String>]> {
        match call.function.as_ref() {
            Expression::Variable(name) => self.function_labels.get(&name.text).map(Vec::as_slice),
            Expression::FieldAccess(access) => match access.record.as_ref() {
                Expression::Variable(module) => self
                    .function_labels
                    .get(&format!("{}.{}", module.text, access.field.text))
                    .map(Vec::as_slice),
                _ => None,
            },
            _ => None,
        }
    }

    fn infer_anonymous_function(&mut self, function: &ast::AnonymousFunction) -> Result<TypeTerm> {
        self.push_scope();
        let mut params = Vec::new();
        for parameter in &function.parameters {
            let type_ = self.annotation_or_fresh(parameter.type_annotation.as_ref())?;
            if let Some(name) = &parameter.name {
                self.define(name.text.clone(), Scheme::monomorphic(type_.clone()));
            }
            params.push(type_);
        }
        let body = self.infer_block(&function.body)?;
        let return_type = match &function.return_type {
            Some(annotation) => {
                let return_type = self.annotation_or_fresh(Some(annotation))?;
                self.constraints.push(return_type.clone(), body, function.body.span);
                return_type
            }
            None => body,
        };
        self.pop_scope();
        Ok(TypeTerm::Function { params, return_type: Box::new(return_type) })
    }

    fn infer_capture(&mut self, capture: &ast::Capture) -> Result<TypeTerm> {
        let function = self.infer_expression(&capture.function)?;
        let all_params = capture
            .arguments
            .iter()
            .map(|argument| match argument {
                Some(argument) => self.infer_expression(&argument.value),
                None => Ok(self.supply.fresh_type()),
            })
            .collect::<Result<Vec<_>>>()?;
        let remaining = capture
            .arguments
            .iter()
            .zip(all_params.iter())
            .filter_map(|(argument, param)| argument.is_none().then_some(param.clone()))
            .collect::<Vec<_>>();
        let return_type = self.supply.fresh_type();
        self.constraints.push(
            TypeTerm::Function { params: all_params, return_type: Box::new(return_type.clone()) },
            function,
            capture.span,
        );
        Ok(TypeTerm::Function { params: remaining, return_type: Box::new(return_type) })
    }

    fn infer_record(&mut self, record: &ast::Record) -> Result<TypeTerm> {
        let name = constructor_name_text(&record.constructor);
        let constructor = self.constructor_function(&name, record.span)?;
        let result = self.supply.fresh_type();
        let args = record
            .arguments
            .iter()
            .map(|argument| self.infer_expression(&argument.value))
            .collect::<Result<Vec<_>>>()?;
        self.constraints.push(
            TypeTerm::Function { params: args, return_type: Box::new(result.clone()) },
            constructor,
            record.span,
        );
        Ok(result)
    }

    fn infer_record_update(&mut self, update: &ast::RecordUpdate) -> Result<TypeTerm> {
        let spread = self.infer_expression(&update.spread)?;
        let name = constructor_name_text(&update.constructor);
        let constructor = self.constructor_function(&name, update.span)?;
        let args = update
            .updates
            .iter()
            .map(|argument| self.infer_expression(&argument.value))
            .collect::<Result<Vec<_>>>()?;
        self.constraints.push(
            TypeTerm::Function { params: args, return_type: Box::new(spread.clone()) },
            constructor,
            update.span,
        );
        Ok(spread)
    }

    fn infer_tuple_access(&mut self, access: &ast::TupleAccess) -> Result<TypeTerm> {
        let tuple = self.infer_expression(&access.tuple)?;
        let index = access.index.text.parse::<usize>().unwrap_or(0);
        let elements = (0..=index).map(|_| self.supply.fresh_type()).collect::<Vec<_>>();
        let result = elements
            .get(index)
            .cloned()
            .ok_or(ConstraintGenerationError::TupleIndexOutOfBounds { index, span: access.index.span })?;
        self.constraints.push(TypeTerm::Tuple(elements), tuple, access.span);
        Ok(result)
    }

    fn infer_list(&mut self, list: &ast::List) -> Result<TypeTerm> {
        let element = self.supply.fresh_type();
        for expression in &list.elements {
            let actual = self.infer_expression(expression)?;
            self.constraints
                .push(element.clone(), actual, expression_span(expression));
        }
        if let Some(spread) = &list.spread {
            let spread_type = self.infer_expression(spread)?;
            self.constraints.push(
                TypeTerm::List(Box::new(element.clone())),
                spread_type,
                expression_span(spread),
            );
        }
        Ok(TypeTerm::List(Box::new(element)))
    }

    fn bind_constructor_pattern(&mut self, pattern: &ast::ConstructorPattern, expected: &TypeTerm) -> Result<()> {
        let name = constructor_pattern_name(pattern);
        let constructor = self.constructor_function(name, pattern.span)?;
        let params = pattern
            .arguments
            .iter()
            .map(|argument| {
                let type_ = self.supply.fresh_type();
                if let Some(pattern) = &argument.pattern {
                    self.bind_pattern(pattern, &type_)?;
                } else if let Some(label) = &argument.label {
                    self.define(label.text.clone(), Scheme::monomorphic(type_.clone()));
                }
                Ok(type_)
            })
            .collect::<Result<Vec<_>>>()?;
        self.constraints.push(
            TypeTerm::Function { params, return_type: Box::new(expected.clone()) },
            constructor,
            pattern.span,
        );
        Ok(())
    }

    fn constructor_function(&mut self, name: &str, span: Span) -> Result<TypeTerm> {
        if let Some(scheme) = self.environment.get_constructor(name) {
            return Ok(scheme.instantiate(&mut self.supply));
        }

        let constructor = self
            .constructors
            .get(name)
            .ok_or_else(|| ConstraintGenerationError::UnknownConstructor { name: name.to_string(), span })?;
        Ok(constructor_scheme(constructor).instantiate(&mut self.supply))
    }

    fn annotation_or_fresh(&mut self, annotation: Option<&ast::TypeAnnotation>) -> Result<TypeTerm> {
        let Some(annotation) = annotation else {
            return Ok(self.supply.fresh_type());
        };
        Type::from_source(&annotation.source)
            .map(|type_| Scheme::instantiate_named_generics(&TypeTerm::from_type(&type_), &mut self.supply))
            .ok_or_else(|| ConstraintGenerationError::UnsupportedAnnotation {
                source: annotation.source.clone(),
                span: annotation.span,
            })
    }

    fn lookup_name(&mut self, name: &ast::Name) -> Result<TypeTerm> {
        for scope in self.scopes.iter().rev() {
            if let Some(scheme) = scope.get(&name.text) {
                return Ok(scheme.instantiate(&mut self.supply));
            }
        }
        self.environment
            .get(&name.text)
            .map(|scheme| scheme.instantiate(&mut self.supply))
            .ok_or_else(|| ConstraintGenerationError::UnknownValue { name: name.text.clone(), span: name.span })
    }

    fn define(&mut self, name: String, scheme: Scheme) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, scheme);
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

fn eligible_for_local_generalization(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Literal(_)
            | Expression::AnonymousFunction(_)
            | Expression::Tuple(_)
            | Expression::List(_)
            | Expression::Record(_)
            | Expression::Block(_)
    )
}

fn literal_type(kind: &LiteralKind) -> TypeTerm {
    match kind {
        LiteralKind::Int => TypeTerm::Int,
        LiteralKind::Float => TypeTerm::Float,
        LiteralKind::String => TypeTerm::String,
        LiteralKind::Bool => TypeTerm::Bool,
        LiteralKind::Nil => TypeTerm::Nil,
    }
}

fn raw_type(kind: &str) -> Option<TypeTerm> {
    match kind {
        "bit_string" => Some(TypeTerm::BitArray),
        "tuple" => Some(TypeTerm::Tuple(Vec::new())),
        "list" => Some(TypeTerm::List(Box::new(TypeTerm::Nil))),
        _ => None,
    }
}

struct BitStringPatternBinding {
    name: ast::Name,
    type_: TypeTerm,
}

fn bit_string_pattern_bindings(raw: &ast::RawSyntax) -> Vec<BitStringPatternBinding> {
    raw.source
        .trim()
        .strip_prefix("<<")
        .and_then(|source| source.strip_suffix(">>"))
        .into_iter()
        .flat_map(|inner| inner.split(','))
        .filter_map(|segment| {
            let (name, options) = segment.split_once(':').unwrap_or((segment, ""));
            let name = name.trim();
            name.chars()
                .next()
                .is_some_and(char::is_lowercase)
                .then(|| BitStringPatternBinding {
                    name: ast::Name { span: raw.span, text: name.into() },
                    type_: if bit_string_segment_is_binary(options) { TypeTerm::BitArray } else { TypeTerm::Int },
                })
        })
        .collect()
}

fn bit_string_segment_is_binary(options: &str) -> bool {
    options
        .split('-')
        .any(|option| matches!(option.trim(), "binary" | "bytes" | "bits" | "bit_string"))
}

fn expression_span(expression: &Expression) -> Span {
    Span::from(expression)
}

fn constructor_name_text(name: &ast::ConstructorName) -> String {
    match name {
        ast::ConstructorName::Local(name) => name.text.clone(),
        ast::ConstructorName::Remote { name, .. } => name.text.clone(),
    }
}

fn constructor_pattern_name(pattern: &ast::ConstructorPattern) -> &str {
    match &pattern.constructor {
        ast::ConstructorName::Local(name) => &name.text,
        ast::ConstructorName::Remote { name, .. } => &name.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{SourceFileId, Span};

    fn span() -> Span {
        Span::new(SourceFileId(0), 0, 0)
    }

    #[test]
    fn generates_literal_constraints() {
        let literal = Expression::Literal(ast::Literal { span: span(), kind: LiteralKind::Int, source: "1".into() });
        let mut generator = ConstraintGenerator::new(Environment::new());

        let type_ = generator.infer_expression(&literal).expect("literal type");

        assert_eq!(type_, TypeTerm::Int);
        assert!(generator.constraints.is_empty());
    }

    #[test]
    fn generates_call_constraint() {
        let mut environment = Environment::new();
        environment.insert(
            "id",
            Scheme::monomorphic(TypeTerm::Function {
                params: vec![TypeTerm::Int],
                return_type: Box::new(TypeTerm::Int),
            }),
        );
        let call = Expression::Call(ast::Call {
            span: span(),
            function: Box::new(Expression::Variable(ast::Name { span: span(), text: "id".into() })),
            arguments: vec![ast::Argument {
                span: span(),
                label: None,
                value: Expression::Literal(ast::Literal { span: span(), kind: LiteralKind::Int, source: "1".into() }),
            }],
        });
        let mut generator = ConstraintGenerator::new(environment);

        generator.infer_expression(&call).expect("call type");

        assert_eq!(generator.constraints.iter().count(), 1);
    }

    #[test]
    fn generates_nested_pattern_constraints_and_bindings() {
        let mut generator = ConstraintGenerator::new(Environment::new());
        let expected = TypeTerm::Tuple(vec![TypeTerm::Int]);
        let pattern = Pattern::Tuple(ast::TuplePattern {
            span: span(),
            elements: vec![Pattern::Name(ast::Name { span: span(), text: "x".into() })],
        });

        generator
            .bind_pattern(&pattern, &expected)
            .expect("pattern constraints");

        assert_eq!(generator.constraints.iter().count(), 1);
        assert!(
            generator
                .lookup_name(&ast::Name { span: span(), text: "x".into() })
                .is_ok()
        );
    }
}
