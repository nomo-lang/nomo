use nomo_ir::{
    BinaryOp, DeferredCall, LoopKind, MathBinaryFunction, MathUnaryFunction, NumBinaryFunction,
    Program, Statement, UnaryOp, ValueExpr, ValueType,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct ExecutionLimits {
    pub max_steps: u64,
    pub max_output_bytes: usize,
    pub max_call_depth: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_steps: 100_000,
            max_output_bytes: 64 * 1024,
            max_call_depth: 64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub code: &'static str,
    pub message: String,
}

impl RuntimeError {
    fn fuel() -> Self {
        Self {
            code: "NOMO-WASM-001",
            message: "execution step limit exceeded".to_string(),
        }
    }

    fn output() -> Self {
        Self {
            code: "NOMO-WASM-002",
            message: "program output limit exceeded".to_string(),
        }
    }

    fn capability(capability: &str) -> Self {
        Self {
            code: "NOMO-WASM-003",
            message: format!(
                "`{capability}` is unavailable in the browser sandbox because it requires host access"
            ),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            code: "NOMO-WASM-004",
            message: message.into(),
        }
    }
}

type RuntimeResult<T> = Result<T, RuntimeError>;

#[derive(Debug, Clone, PartialEq)]
enum Value {
    String(String),
    I64(i64),
    I32(i32),
    U32(u32),
    U64(u64),
    F64(f64),
    Char(char),
    Bool(bool),
    Array(Vec<Value>),
    Struct {
        name: String,
        fields: HashMap<String, Value>,
    },
    Enum {
        name: String,
        variant: String,
        payload: Option<Box<Value>>,
    },
    Void,
}

impl Value {
    fn display(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::I64(value) => value.to_string(),
            Self::I32(value) => value.to_string(),
            Self::U32(value) => value.to_string(),
            Self::U64(value) => value.to_string(),
            Self::F64(value) => value.to_string(),
            Self::Char(value) => value.to_string(),
            Self::Bool(value) => value.to_string(),
            Self::Array(values) => {
                let rendered = values
                    .iter()
                    .map(Self::display)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{rendered}]")
            }
            Self::Struct { name, fields } => {
                let mut fields = fields
                    .iter()
                    .map(|(key, value)| format!("{key}: {}", value.display()))
                    .collect::<Vec<_>>();
                fields.sort();
                format!("{name} {{ {} }}", fields.join(", "))
            }
            Self::Enum {
                name,
                variant,
                payload,
            } => payload.as_ref().map_or_else(
                || format!("{name}.{variant}"),
                |payload| format!("{name}.{variant}({})", payload.display()),
            ),
            Self::Void => String::new(),
        }
    }

    fn as_bool(&self) -> RuntimeResult<bool> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(RuntimeError::runtime("expected a bool value")),
        }
    }

    fn as_index(&self) -> RuntimeResult<usize> {
        match self {
            Self::U64(value) => usize::try_from(*value)
                .map_err(|_| RuntimeError::runtime("array index is too large")),
            Self::U32(value) => Ok(*value as usize),
            Self::I32(value) if *value >= 0 => Ok(*value as usize),
            Self::I64(value) if *value >= 0 => usize::try_from(*value)
                .map_err(|_| RuntimeError::runtime("array index is too large")),
            _ => Err(RuntimeError::runtime(
                "array index must be a non-negative integer",
            )),
        }
    }

    fn as_string(&self) -> RuntimeResult<&str> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(RuntimeError::runtime("expected a string value")),
        }
    }

    fn coerce(self, value_type: &ValueType) -> RuntimeResult<Self> {
        match (self, value_type) {
            (value @ Self::String(_), ValueType::String) => Ok(value),
            (value @ Self::Char(_), ValueType::Char) => Ok(value),
            (value @ Self::Bool(_), ValueType::Bool) => Ok(value),
            (value @ Self::Array(_), ValueType::Array(_)) => Ok(value),
            (value @ Self::Struct { .. }, ValueType::Struct(_, _)) => Ok(value),
            (value @ Self::Enum { .. }, ValueType::Enum(_, _)) => Ok(value),
            (Self::Void, ValueType::Void | ValueType::Never) => Ok(Self::Void),
            (Self::I64(value), ValueType::Int) => Ok(Self::I64(value)),
            (Self::I64(value), ValueType::I32) => i32::try_from(value)
                .map(Self::I32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in i32")),
            (Self::I64(value), ValueType::U32) => u32::try_from(value)
                .map(Self::U32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in u32")),
            (Self::I64(value), ValueType::U64) => u64::try_from(value)
                .map(Self::U64)
                .map_err(|_| RuntimeError::runtime("integer does not fit in u64")),
            (Self::I32(value), ValueType::Int) => Ok(Self::I64(value.into())),
            (Self::I32(value), ValueType::I32) => Ok(Self::I32(value)),
            (Self::I32(value), ValueType::U32) => u32::try_from(value)
                .map(Self::U32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in u32")),
            (Self::I32(value), ValueType::U64) => u64::try_from(value)
                .map(Self::U64)
                .map_err(|_| RuntimeError::runtime("integer does not fit in u64")),
            (Self::U32(value), ValueType::Int) => Ok(Self::I64(value.into())),
            (Self::U32(value), ValueType::I32) => i32::try_from(value)
                .map(Self::I32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in i32")),
            (Self::U32(value), ValueType::U32) => Ok(Self::U32(value)),
            (Self::U32(value), ValueType::U64) => Ok(Self::U64(value.into())),
            (Self::U64(value), ValueType::Int) => i64::try_from(value)
                .map(Self::I64)
                .map_err(|_| RuntimeError::runtime("integer does not fit in i64")),
            (Self::U64(value), ValueType::I32) => i32::try_from(value)
                .map(Self::I32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in i32")),
            (Self::U64(value), ValueType::U32) => u32::try_from(value)
                .map(Self::U32)
                .map_err(|_| RuntimeError::runtime("integer does not fit in u32")),
            (Self::U64(value), ValueType::U64) => Ok(Self::U64(value)),
            (Self::F64(value), ValueType::Float) => Ok(Self::F64(value)),
            (Self::I64(value), ValueType::Float) => Ok(Self::F64(value as f64)),
            (Self::I32(value), ValueType::Float) => Ok(Self::F64(value.into())),
            (Self::U32(value), ValueType::Float) => Ok(Self::F64(value.into())),
            (Self::U64(value), ValueType::Float) => Ok(Self::F64(value as f64)),
            (value, target) => Err(RuntimeError::runtime(format!(
                "cannot coerce runtime value `{}` to `{}`",
                value.display(),
                target.name()
            ))),
        }
    }
}

#[derive(Debug, Clone)]
enum Signal {
    Next,
    Return(Value),
    Break,
    Continue,
}

pub struct Interpreter<'a> {
    program: &'a Program,
    limits: ExecutionLimits,
    steps: u64,
    stdout: String,
    stderr: String,
    globals: HashMap<String, Value>,
    frames: Vec<HashMap<String, Value>>,
}

impl<'a> Interpreter<'a> {
    pub fn new(program: &'a Program, limits: ExecutionLimits) -> Self {
        Self {
            program,
            limits,
            steps: 0,
            stdout: String::new(),
            stderr: String::new(),
            globals: HashMap::new(),
            frames: Vec::new(),
        }
    }

    pub fn run_main(&mut self) -> RuntimeResult<()> {
        for constant in &self.program.consts {
            let value = self
                .eval_expr(&constant.initializer)?
                .coerce(&constant.value_type)?;
            self.globals.insert(constant.name.clone(), value);
        }
        let result = self.call_function("main", &[])?;
        if result != Value::Void {
            return Err(RuntimeError::runtime("`main` returned a non-void value"));
        }
        Ok(())
    }

    pub fn steps(&self) -> u64 {
        self.steps
    }

    pub fn output_bytes(&self) -> usize {
        self.stdout.len() + self.stderr.len()
    }

    pub fn into_output(self) -> (String, String) {
        (self.stdout, self.stderr)
    }

    fn tick(&mut self) -> RuntimeResult<()> {
        self.steps = self.steps.saturating_add(1);
        if self.steps > self.limits.max_steps {
            return Err(RuntimeError::fuel());
        }
        Ok(())
    }

    fn write_stdout(&mut self, value: &Value, newline: bool) -> RuntimeResult<()> {
        self.write_output(value, newline, false)
    }

    fn write_stderr(&mut self, value: &Value, newline: bool) -> RuntimeResult<()> {
        self.write_output(value, newline, true)
    }

    fn write_output(&mut self, value: &Value, newline: bool, stderr: bool) -> RuntimeResult<()> {
        let rendered = value.display();
        let added = rendered.len() + usize::from(newline);
        if self.output_bytes().saturating_add(added) > self.limits.max_output_bytes {
            return Err(RuntimeError::output());
        }
        let target = if stderr {
            &mut self.stderr
        } else {
            &mut self.stdout
        };
        target.push_str(&rendered);
        if newline {
            target.push('\n');
        }
        Ok(())
    }

    fn current_frame(&self) -> RuntimeResult<&HashMap<String, Value>> {
        self.frames
            .last()
            .ok_or_else(|| RuntimeError::runtime("no active function frame"))
    }

    fn current_frame_mut(&mut self) -> RuntimeResult<&mut HashMap<String, Value>> {
        self.frames
            .last_mut()
            .ok_or_else(|| RuntimeError::runtime("no active function frame"))
    }

    fn get_variable(&self, name: &str) -> RuntimeResult<Value> {
        self.current_frame()?
            .get(name)
            .or_else(|| self.globals.get(name))
            .cloned()
            .ok_or_else(|| RuntimeError::runtime(format!("unknown runtime variable `{name}`")))
    }

    fn set_variable(&mut self, name: &str, value: Value) -> RuntimeResult<()> {
        let frame = self.current_frame_mut()?;
        let target = frame
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::runtime(format!("unknown runtime variable `{name}`")))?;
        let value = coerce_like(value, &target)?;
        frame.insert(name.to_string(), value);
        Ok(())
    }

    fn get_path(&self, path: &[String]) -> RuntimeResult<Value> {
        let Some(name) = path.first() else {
            return Err(RuntimeError::runtime("empty runtime path"));
        };
        let mut value = self.get_variable(name)?;
        for field in &path[1..] {
            value = match value {
                Value::Struct { fields, .. } => fields.get(field).cloned().ok_or_else(|| {
                    RuntimeError::runtime(format!("unknown runtime field `{field}`"))
                })?,
                _ => {
                    return Err(RuntimeError::runtime(format!(
                        "`{field}` is not a field on this runtime value"
                    )));
                }
            };
        }
        Ok(value)
    }

    fn set_path(&mut self, path: &[String], value: Value) -> RuntimeResult<()> {
        match path {
            [name] => self.set_variable(name, value),
            [name, field] => {
                let mut base = self.get_variable(name)?;
                let Value::Struct { fields, .. } = &mut base else {
                    return Err(RuntimeError::runtime(format!(
                        "`{name}` is not a struct value"
                    )));
                };
                let target = fields.get(field).cloned().ok_or_else(|| {
                    RuntimeError::runtime(format!("unknown runtime field `{field}`"))
                })?;
                fields.insert(field.clone(), coerce_like(value, &target)?);
                self.current_frame_mut()?.insert(name.clone(), base);
                Ok(())
            }
            _ => Err(RuntimeError::runtime(
                "nested runtime paths deeper than one field are unsupported",
            )),
        }
    }

    fn call_function(&mut self, name: &str, args: &[ValueExpr]) -> RuntimeResult<Value> {
        self.tick()?;
        if self.frames.len() >= self.limits.max_call_depth {
            return Err(RuntimeError::runtime("maximum call depth exceeded"));
        }
        let function = self
            .program
            .functions
            .iter()
            .find(|function| function.name == name)
            .cloned()
            .ok_or_else(|| RuntimeError::runtime(format!("unknown function `{name}`")))?;
        if function.params.len() != args.len() {
            return Err(RuntimeError::runtime(format!(
                "function `{name}` expected {} arguments, found {}",
                function.params.len(),
                args.len()
            )));
        }

        let caller_index = self.frames.len().checked_sub(1);
        let mut writebacks: Vec<(String, Vec<String>)> = Vec::new();
        let mut frame = HashMap::new();
        for (parameter, argument) in function.params.iter().zip(args) {
            let (value, source_path) = match argument {
                ValueExpr::MutBorrow(path) => (self.get_path(path)?, Some(path.clone())),
                argument => (self.eval_expr(argument)?, None),
            };
            frame.insert(parameter.name.clone(), value.coerce(&parameter.value_type)?);
            if parameter.mutable
                && let Some(path) = source_path
            {
                writebacks.push((parameter.name.clone(), path));
            }
        }

        self.frames.push(frame);
        let signal = self.exec_block(&function.body)?;
        let final_frame = self
            .frames
            .pop()
            .ok_or_else(|| RuntimeError::runtime("function frame disappeared"))?;
        if let Some(caller_index) = caller_index {
            for (parameter, path) in writebacks {
                let value = final_frame.get(&parameter).cloned().ok_or_else(|| {
                    RuntimeError::runtime(format!("missing mutable parameter `{parameter}`"))
                })?;
                set_path_in_frame(&mut self.frames[caller_index], &path, value)?;
            }
        }

        let result = match signal {
            Signal::Return(value) => value,
            Signal::Next => Value::Void,
            Signal::Break | Signal::Continue => {
                return Err(RuntimeError::runtime(
                    "loop control escaped a function body",
                ));
            }
        };
        result.coerce(&function.return_type)
    }

    fn exec_block(&mut self, statements: &[Statement]) -> RuntimeResult<Signal> {
        let mut deferred = Vec::new();
        for statement in statements {
            self.tick()?;
            if let Statement::Defer { call } = statement {
                deferred.push(call.clone());
                continue;
            }
            let signal = self.exec_statement(statement)?;
            if !matches!(signal, Signal::Next) {
                self.run_deferred(&deferred)?;
                return Ok(signal);
            }
        }
        self.run_deferred(&deferred)?;
        Ok(Signal::Next)
    }

    fn run_deferred(&mut self, deferred: &[DeferredCall]) -> RuntimeResult<()> {
        for call in deferred.iter().rev() {
            self.tick()?;
            match call {
                DeferredCall::Expr(expr) => {
                    self.eval_expr(expr)?;
                }
                DeferredCall::Println(expr) => {
                    let value = self.eval_expr(expr)?;
                    self.write_stdout(&value, true)?;
                }
                DeferredCall::Print(expr) => {
                    let value = self.eval_expr(expr)?;
                    self.write_stdout(&value, false)?;
                }
                DeferredCall::Eprintln(expr) => {
                    let value = self.eval_expr(expr)?;
                    self.write_stderr(&value, true)?;
                }
                DeferredCall::Eprint(expr) => {
                    let value = self.eval_expr(expr)?;
                    self.write_stderr(&value, false)?;
                }
            }
        }
        Ok(())
    }

    fn exec_statement(&mut self, statement: &Statement) -> RuntimeResult<Signal> {
        match statement {
            Statement::Let {
                name,
                value_type,
                initializer,
            } => {
                let value = self.eval_expr(initializer)?.coerce(value_type)?;
                self.current_frame_mut()?.insert(name.clone(), value);
                Ok(Signal::Next)
            }
            Statement::Assign { name, value } => {
                let value = self.eval_expr(value)?;
                self.set_variable(name, value)?;
                Ok(Signal::Next)
            }
            Statement::AssignField {
                base, field, value, ..
            } => {
                let value = self.eval_expr(value)?;
                self.set_path(&[base.clone(), field.clone()], value)?;
                Ok(Signal::Next)
            }
            Statement::Println(expr) => {
                let value = self.eval_expr(expr)?;
                self.write_stdout(&value, true)?;
                Ok(Signal::Next)
            }
            Statement::Print(expr) => {
                let value = self.eval_expr(expr)?;
                self.write_stdout(&value, false)?;
                Ok(Signal::Next)
            }
            Statement::Eprintln(expr) => {
                let value = self.eval_expr(expr)?;
                self.write_stderr(&value, true)?;
                Ok(Signal::Next)
            }
            Statement::Eprint(expr) => {
                let value = self.eval_expr(expr)?;
                self.write_stderr(&value, false)?;
                Ok(Signal::Next)
            }
            Statement::Expr(expr) => {
                self.eval_expr(expr)?;
                Ok(Signal::Next)
            }
            Statement::Return(value) => Ok(Signal::Return(
                value
                    .as_ref()
                    .map(|value| self.eval_expr(value))
                    .transpose()?
                    .unwrap_or(Value::Void),
            )),
            Statement::If {
                condition,
                body,
                else_body,
            } => {
                if self.eval_expr(condition)?.as_bool()? {
                    self.exec_block(body)
                } else {
                    self.exec_block(else_body)
                }
            }
            Statement::Loop { kind, body } => self.exec_loop(kind, body),
            Statement::Break => Ok(Signal::Break),
            Statement::Continue => Ok(Signal::Continue),
            Statement::Match { value, arms, .. } => {
                let value = self.eval_expr(value)?;
                let Value::Enum {
                    variant, payload, ..
                } = value
                else {
                    return Err(RuntimeError::runtime("match value is not an enum"));
                };
                let arm = arms
                    .iter()
                    .find(|arm| arm.variant == variant)
                    .ok_or_else(|| RuntimeError::runtime("no matching enum arm"))?;
                if let (Some(binding), Some(payload)) = (&arm.binding, payload) {
                    self.current_frame_mut()?.insert(binding.clone(), *payload);
                }
                self.exec_block(&arm.body)
            }
            Statement::LetElse {
                binding,
                value,
                variant,
                else_body,
                ..
            } => {
                let value = self.eval_expr(value)?;
                let Value::Enum {
                    variant: actual,
                    payload,
                    ..
                } = value
                else {
                    return Err(RuntimeError::runtime("let-else value is not an enum"));
                };
                if actual == *variant {
                    let payload = payload.ok_or_else(|| {
                        RuntimeError::runtime("let-else matched variant has no payload")
                    })?;
                    self.current_frame_mut()?.insert(binding.clone(), *payload);
                    Ok(Signal::Next)
                } else {
                    self.exec_block(else_body)
                }
            }
            Statement::IfLet {
                binding,
                value,
                variant,
                body,
                else_body,
                ..
            } => {
                let value = self.eval_expr(value)?;
                let matches =
                    matches!(&value, Value::Enum { variant: actual, .. } if actual == variant);
                if matches {
                    if let (
                        Some(binding),
                        Value::Enum {
                            payload: Some(payload),
                            ..
                        },
                    ) = (binding, value)
                    {
                        self.current_frame_mut()?.insert(binding.clone(), *payload);
                    }
                    self.exec_block(body)
                } else if let Some(else_body) = else_body {
                    self.exec_block(else_body)
                } else {
                    Ok(Signal::Next)
                }
            }
            Statement::Defer { .. } => {
                unreachable!("defer is collected by exec_block")
            }
            Statement::Panic(expr) => {
                let message = self.eval_expr(expr)?.display();
                Err(RuntimeError::runtime(format!("panic: {message}")))
            }
            Statement::LetIf { .. }
            | Statement::LetMatch { .. }
            | Statement::QuestionLet { .. }
            | Statement::QuestionReturn { .. } => Err(RuntimeError::runtime(
                "this control-flow form is not implemented by the browser interpreter yet",
            )),
        }
    }

    fn exec_loop(&mut self, kind: &LoopKind, body: &[Statement]) -> RuntimeResult<Signal> {
        match kind {
            LoopKind::Infinite => loop {
                self.tick()?;
                match self.exec_block(body)? {
                    Signal::Next | Signal::Continue => {}
                    Signal::Break => return Ok(Signal::Next),
                    signal @ Signal::Return(_) => return Ok(signal),
                }
            },
            LoopKind::While(condition) => {
                while self.eval_expr(condition)?.as_bool()? {
                    self.tick()?;
                    match self.exec_block(body)? {
                        Signal::Next | Signal::Continue => {}
                        Signal::Break => return Ok(Signal::Next),
                        signal @ Signal::Return(_) => return Ok(signal),
                    }
                }
                Ok(Signal::Next)
            }
            LoopKind::CStyle {
                binding,
                value_type,
                initializer,
                condition,
                update,
            } => {
                let value = self.eval_expr(initializer)?.coerce(value_type)?;
                self.current_frame_mut()?.insert(binding.clone(), value);
                while self.eval_expr(condition)?.as_bool()? {
                    self.tick()?;
                    match self.exec_block(body)? {
                        Signal::Next | Signal::Continue => {
                            let value = self.eval_expr(update)?.coerce(value_type)?;
                            self.set_variable(binding, value)?;
                        }
                        Signal::Break => return Ok(Signal::Next),
                        signal @ Signal::Return(_) => return Ok(signal),
                    }
                }
                Ok(Signal::Next)
            }
            LoopKind::Iterate {
                binding, iterable, ..
            } => {
                let Value::Array(values) = self.eval_expr(iterable)? else {
                    return Err(RuntimeError::runtime("loop iterable is not an array"));
                };
                for value in values {
                    self.tick()?;
                    self.current_frame_mut()?.insert(binding.clone(), value);
                    match self.exec_block(body)? {
                        Signal::Next | Signal::Continue => {}
                        Signal::Break => return Ok(Signal::Next),
                        signal @ Signal::Return(_) => return Ok(signal),
                    }
                }
                Ok(Signal::Next)
            }
        }
    }

    fn eval_expr(&mut self, expression: &ValueExpr) -> RuntimeResult<Value> {
        self.tick()?;
        match expression {
            ValueExpr::StringLiteral(value) => Ok(Value::String(value.clone())),
            ValueExpr::IntLiteral(value) => Ok(Value::I64(*value)),
            ValueExpr::FloatLiteral(value) => value
                .parse::<f64>()
                .map(Value::F64)
                .map_err(|_| RuntimeError::runtime("invalid floating-point literal")),
            ValueExpr::CharLiteral(value) => Ok(Value::Char(*value)),
            ValueExpr::BoolLiteral(value) => Ok(Value::Bool(*value)),
            ValueExpr::VoidLiteral => Ok(Value::Void),
            ValueExpr::Variable(name) => self.get_variable(name),
            ValueExpr::FunctionRef(name) => Ok(Value::String(name.clone())),
            ValueExpr::Call { name, args } => self.call_function(name, args),
            ValueExpr::MutBorrow(path) => self.get_path(path),
            ValueExpr::Binary {
                left, op, right, ..
            } => self.eval_binary(left, *op, right),
            ValueExpr::StringCompare { left, op, right } => {
                let left = self.eval_expr(left)?.as_string()?.to_string();
                let right = self.eval_expr(right)?.as_string()?.to_string();
                Ok(Value::Bool(match op {
                    BinaryOp::Equal => left == right,
                    BinaryOp::NotEqual => left != right,
                    _ => {
                        return Err(RuntimeError::runtime("invalid string comparison operator"));
                    }
                }))
            }
            ValueExpr::Unary { op, expr } => {
                let value = self.eval_expr(expr)?;
                match (op, value) {
                    (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                    (UnaryOp::Negate, Value::I64(value)) => value
                        .checked_neg()
                        .map(Value::I64)
                        .ok_or_else(|| RuntimeError::runtime("integer overflow")),
                    (UnaryOp::Negate, Value::I32(value)) => value
                        .checked_neg()
                        .map(Value::I32)
                        .ok_or_else(|| RuntimeError::runtime("integer overflow")),
                    (UnaryOp::Negate, Value::F64(value)) => Ok(Value::F64(-value)),
                    _ => Err(RuntimeError::runtime("invalid unary operand")),
                }
            }
            ValueExpr::Cast { expr, target_type } => self.eval_expr(expr)?.coerce(target_type),
            ValueExpr::StringLen { value } => {
                Ok(Value::U64(self.eval_expr(value)?.as_string()?.len() as u64))
            }
            ValueExpr::StringConcat { left, right } => Ok(Value::String(format!(
                "{}{}",
                self.eval_expr(left)?.as_string()?,
                self.eval_expr(right)?.as_string()?
            ))),
            ValueExpr::StringIsEmpty { value } => {
                Ok(Value::Bool(self.eval_expr(value)?.as_string()?.is_empty()))
            }
            ValueExpr::StringContains { value, needle } => {
                let value = self.eval_expr(value)?.as_string()?.to_string();
                let needle = self.eval_expr(needle)?.as_string()?.to_string();
                Ok(Value::Bool(value.contains(&needle)))
            }
            ValueExpr::StringStartsWith { value, prefix } => {
                let value = self.eval_expr(value)?.as_string()?.to_string();
                let prefix = self.eval_expr(prefix)?.as_string()?.to_string();
                Ok(Value::Bool(value.starts_with(&prefix)))
            }
            ValueExpr::StringEndsWith { value, suffix } => {
                let value = self.eval_expr(value)?.as_string()?.to_string();
                let suffix = self.eval_expr(suffix)?.as_string()?.to_string();
                Ok(Value::Bool(value.ends_with(&suffix)))
            }
            ValueExpr::StringSplit { value, separator } => {
                let value = self.eval_expr(value)?.as_string()?.to_string();
                let separator = self.eval_expr(separator)?.as_string()?.to_string();
                Ok(Value::Array(
                    value
                        .split(&separator)
                        .map(|part| Value::String(part.to_string()))
                        .collect(),
                ))
            }
            ValueExpr::StringTrim { value } => Ok(Value::String(
                self.eval_expr(value)?.as_string()?.trim().to_string(),
            )),
            ValueExpr::StringToLower { value } => Ok(Value::String(
                self.eval_expr(value)?.as_string()?.to_lowercase(),
            )),
            ValueExpr::StringToUpper { value } => Ok(Value::String(
                self.eval_expr(value)?.as_string()?.to_uppercase(),
            )),
            ValueExpr::CharIsDigit { value } => match self.eval_expr(value)? {
                Value::Char(value) => Ok(Value::Bool(value.is_numeric())),
                _ => Err(RuntimeError::runtime("expected char")),
            },
            ValueExpr::CharIsAlpha { value } => match self.eval_expr(value)? {
                Value::Char(value) => Ok(Value::Bool(value.is_alphabetic())),
                _ => Err(RuntimeError::runtime("expected char")),
            },
            ValueExpr::CharIsWhitespace { value } => match self.eval_expr(value)? {
                Value::Char(value) => Ok(Value::Bool(value.is_whitespace())),
                _ => Err(RuntimeError::runtime("expected char")),
            },
            ValueExpr::CharToString { value } => match self.eval_expr(value)? {
                Value::Char(value) => Ok(Value::String(value.to_string())),
                _ => Err(RuntimeError::runtime("expected char")),
            },
            ValueExpr::NumToString { value, .. } => {
                Ok(Value::String(self.eval_expr(value)?.display()))
            }
            ValueExpr::NumParseI64 { value } => {
                let value = self.eval_expr(value)?.as_string()?.parse::<i64>();
                Ok(result_value(
                    "Result",
                    value
                        .map(Value::I64)
                        .map_err(|_| Value::String("invalid i64".into())),
                ))
            }
            ValueExpr::NumParseU64 { value } => {
                let value = self.eval_expr(value)?.as_string()?.parse::<u64>();
                Ok(result_value(
                    "Result",
                    value
                        .map(Value::U64)
                        .map_err(|_| Value::String("invalid u64".into())),
                ))
            }
            ValueExpr::NumParseF64 { value } => {
                let value = self.eval_expr(value)?.as_string()?.parse::<f64>();
                Ok(result_value(
                    "Result",
                    value
                        .map(Value::F64)
                        .map_err(|_| Value::String("invalid f64".into())),
                ))
            }
            ValueExpr::ArrayNew { .. } => Ok(Value::Array(Vec::new())),
            ValueExpr::ArrayLen { array } => match self.eval_expr(array)? {
                Value::Array(values) => Ok(Value::U64(values.len() as u64)),
                _ => Err(RuntimeError::runtime("expected array")),
            },
            ValueExpr::ArrayIter { array, .. } => self.eval_expr(array),
            ValueExpr::ArrayGet { array, index, .. } => {
                let Value::Array(values) = self.eval_expr(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                let index = self.eval_expr(index)?.as_index()?;
                Ok(values
                    .get(index)
                    .cloned()
                    .map_or_else(|| option_value(None), |value| option_value(Some(value))))
            }
            ValueExpr::ArrayPush { array, value, .. } => {
                let value = self.eval_expr(value)?;
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                values.push(value);
                Ok(Value::Array(values))
            }
            ValueExpr::ArrayPop { array, .. } => {
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                let result = option_value(values.pop());
                self.current_frame_mut()?
                    .insert(array.clone(), Value::Array(values));
                Ok(result)
            }
            ValueExpr::ArraySet {
                array,
                index,
                value,
                ..
            } => {
                let index = self.eval_expr(index)?.as_index()?;
                let value = self.eval_expr(value)?;
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                let slot = values
                    .get_mut(index)
                    .ok_or_else(|| RuntimeError::runtime("array index out of bounds"))?;
                *slot = coerce_like(value, slot)?;
                Ok(Value::Array(values))
            }
            ValueExpr::ArrayInsert {
                array,
                index,
                value,
                ..
            } => {
                let index = self.eval_expr(index)?.as_index()?;
                let value = self.eval_expr(value)?;
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                if index > values.len() {
                    return Err(RuntimeError::runtime("array index out of bounds"));
                }
                values.insert(index, value);
                Ok(Value::Array(values))
            }
            ValueExpr::ArrayRemove { array, index, .. } => {
                let index = self.eval_expr(index)?.as_index()?;
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                let result = if index < values.len() {
                    option_value(Some(values.remove(index)))
                } else {
                    option_value(None)
                };
                self.current_frame_mut()?
                    .insert(array.clone(), Value::Array(values));
                Ok(result)
            }
            ValueExpr::ArrayClear { array, .. } => {
                let Value::Array(mut values) = self.get_variable(array)? else {
                    return Err(RuntimeError::runtime("expected array"));
                };
                values.clear();
                Ok(Value::Array(values))
            }
            ValueExpr::StructLiteral {
                type_name, fields, ..
            } => {
                let fields = fields
                    .iter()
                    .map(|(name, value)| Ok((name.clone(), self.eval_expr(value)?)))
                    .collect::<RuntimeResult<HashMap<_, _>>>()?;
                Ok(Value::Struct {
                    name: type_name.clone(),
                    fields,
                })
            }
            ValueExpr::FieldAccess { base, field } => {
                let Value::Struct { fields, .. } = self.get_variable(base)? else {
                    return Err(RuntimeError::runtime("field base is not a struct"));
                };
                fields.get(field).cloned().ok_or_else(|| {
                    RuntimeError::runtime(format!("unknown runtime field `{field}`"))
                })
            }
            ValueExpr::EnumVariant {
                enum_name,
                variant,
                payload,
                ..
            } => Ok(Value::Enum {
                name: enum_name.clone(),
                variant: variant.clone(),
                payload: payload
                    .as_ref()
                    .map(|value| self.eval_expr(value).map(Box::new))
                    .transpose()?,
            }),
            ValueExpr::EnumPayload { value, variant } => {
                let Value::Enum {
                    variant: actual,
                    payload,
                    ..
                } = self.eval_expr(value)?
                else {
                    return Err(RuntimeError::runtime("enum payload source is not an enum"));
                };
                if actual != *variant {
                    return Err(RuntimeError::runtime("enum variant mismatch"));
                }
                payload
                    .map(|payload| *payload)
                    .ok_or_else(|| RuntimeError::runtime("enum variant does not carry a payload"))
            }
            ValueExpr::Match { value, arms } => {
                let Value::Enum {
                    variant, payload, ..
                } = self.eval_expr(value)?
                else {
                    return Err(RuntimeError::runtime("match value is not an enum"));
                };
                let arm = arms
                    .iter()
                    .find(|arm| arm.variant == variant)
                    .ok_or_else(|| RuntimeError::runtime("no matching enum arm"))?;
                if let (Some(binding), Some(payload)) = (&arm.binding, payload) {
                    self.current_frame_mut()?.insert(binding.clone(), *payload);
                }
                self.eval_expr(&arm.value)
            }
            ValueExpr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.eval_expr(condition)?.as_bool()? {
                    self.eval_expr(then_branch)
                } else {
                    self.eval_expr(else_branch)
                }
            }
            ValueExpr::OptionIsSome { option, .. } => Ok(Value::Bool(matches!(
                self.eval_expr(option)?,
                Value::Enum { variant, .. } if variant == "Some"
            ))),
            ValueExpr::OptionIsNone { option, .. } => Ok(Value::Bool(matches!(
                self.eval_expr(option)?,
                Value::Enum { variant, .. } if variant == "None"
            ))),
            ValueExpr::OptionUnwrapOr {
                option, default, ..
            } => match self.eval_expr(option)? {
                Value::Enum {
                    variant,
                    payload: Some(payload),
                    ..
                } if variant == "Some" => Ok(*payload),
                _ => self.eval_expr(default),
            },
            ValueExpr::ResultIsOk { result, .. } => Ok(Value::Bool(matches!(
                self.eval_expr(result)?,
                Value::Enum { variant, .. } if variant == "Ok"
            ))),
            ValueExpr::ResultIsErr { result, .. } => Ok(Value::Bool(matches!(
                self.eval_expr(result)?,
                Value::Enum { variant, .. } if variant == "Err"
            ))),
            ValueExpr::ResultUnwrapOr {
                result, default, ..
            } => match self.eval_expr(result)? {
                Value::Enum {
                    variant,
                    payload: Some(payload),
                    ..
                } if variant == "Ok" => Ok(*payload),
                _ => self.eval_expr(default),
            },
            ValueExpr::MathUnary {
                function, value, ..
            } => math_unary(*function, self.eval_expr(value)?),
            ValueExpr::MathBinary {
                function,
                left,
                right,
                ..
            } => math_binary(*function, self.eval_expr(left)?, self.eval_expr(right)?),
            ValueExpr::NumBinary {
                function,
                op,
                left,
                right,
                ..
            } => {
                let left = self.eval_expr(left)?;
                let right = self.eval_expr(right)?;
                match function {
                    NumBinaryFunction::Checked => {
                        Ok(option_value(numeric_operation(left, *op, right).ok()))
                    }
                    NumBinaryFunction::Wrapping => wrapping_numeric_operation(left, *op, right),
                }
            }
            ValueExpr::OsPlatform => Ok(Value::String("browser".to_string())),
            ValueExpr::OsArch => Ok(Value::String("wasm32".to_string())),
            ValueExpr::OsPathSeparator => Ok(Value::String("/".to_string())),
            ValueExpr::OsLineEnding => Ok(Value::String("\n".to_string())),
            ValueExpr::Panic { message, .. } => {
                let message = self.eval_expr(message)?.display();
                Err(RuntimeError::runtime(format!("panic: {message}")))
            }
            ValueExpr::FsReadToString { .. }
            | ValueExpr::FsWriteString { .. }
            | ValueExpr::FsReadBytes { .. }
            | ValueExpr::FsWriteBytes { .. }
            | ValueExpr::FsExists { .. }
            | ValueExpr::FsMetadata { .. }
            | ValueExpr::FsCreateDir { .. }
            | ValueExpr::FsRemoveDir { .. }
            | ValueExpr::FsReadDir { .. }
            | ValueExpr::FsOpen { .. }
            | ValueExpr::FileClose { .. }
            | ValueExpr::FileReadToString { .. }
            | ValueExpr::FileWriteString { .. } => Err(RuntimeError::capability("filesystem")),
            ValueExpr::NetConnect { .. }
            | ValueExpr::NetListen { .. }
            | ValueExpr::NetUdpBind { .. }
            | ValueExpr::TcpListenerAccept { .. }
            | ValueExpr::TcpListenerClose { .. }
            | ValueExpr::TcpStreamClose { .. }
            | ValueExpr::TcpStreamReadToString { .. }
            | ValueExpr::TcpStreamWriteString { .. }
            | ValueExpr::UdpSocketClose { .. }
            | ValueExpr::UdpSocketRecvFromString { .. }
            | ValueExpr::UdpSocketSendToString { .. } => Err(RuntimeError::capability("network")),
            ValueExpr::ProcessExit { .. }
            | ValueExpr::ProcessSpawn { .. }
            | ValueExpr::ProcessStatus { .. }
            | ValueExpr::ProcessExec { .. }
            | ValueExpr::ProcessOutput { .. } => Err(RuntimeError::capability("process")),
            ValueExpr::EnvGet { .. }
            | ValueExpr::EnvSet { .. }
            | ValueExpr::EnvCwd
            | ValueExpr::EnvHomeDir
            | ValueExpr::EnvTempDir
            | ValueExpr::EnvArgs => Err(RuntimeError::capability("environment")),
            ValueExpr::IoReadLine => Err(RuntimeError::capability("interactive input")),
            ValueExpr::TimeNowMillis
            | ValueExpr::TimeMonotonicMillis
            | ValueExpr::TimeSleep { .. }
            | ValueExpr::TimeSleepMillis { .. } => Err(RuntimeError::capability("clock")),
            other => Err(RuntimeError::runtime(format!(
                "typed IR operation is not implemented by the browser interpreter: {other:?}"
            ))),
        }
    }

    fn eval_binary(
        &mut self,
        left: &ValueExpr,
        op: BinaryOp,
        right: &ValueExpr,
    ) -> RuntimeResult<Value> {
        if matches!(op, BinaryOp::LogicalAnd) {
            let left = self.eval_expr(left)?.as_bool()?;
            return if left {
                Ok(Value::Bool(self.eval_expr(right)?.as_bool()?))
            } else {
                Ok(Value::Bool(false))
            };
        }
        if matches!(op, BinaryOp::LogicalOr) {
            let left = self.eval_expr(left)?.as_bool()?;
            return if left {
                Ok(Value::Bool(true))
            } else {
                Ok(Value::Bool(self.eval_expr(right)?.as_bool()?))
            };
        }

        let left = self.eval_expr(left)?;
        let right = self.eval_expr(right)?;
        match op {
            BinaryOp::Equal => return Ok(Value::Bool(values_equal(&left, &right)?)),
            BinaryOp::NotEqual => return Ok(Value::Bool(!values_equal(&left, &right)?)),
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                return compare_values(left, op, right).map(Value::Bool);
            }
            _ => {}
        }
        numeric_operation(left, op, right)
    }
}

fn set_path_in_frame(
    frame: &mut HashMap<String, Value>,
    path: &[String],
    value: Value,
) -> RuntimeResult<()> {
    match path {
        [name] => {
            let target = frame.get(name).cloned().ok_or_else(|| {
                RuntimeError::runtime(format!("unknown runtime variable `{name}`"))
            })?;
            frame.insert(name.clone(), coerce_like(value, &target)?);
            Ok(())
        }
        [name, field] => {
            let base = frame.get_mut(name).ok_or_else(|| {
                RuntimeError::runtime(format!("unknown runtime variable `{name}`"))
            })?;
            let Value::Struct { fields, .. } = base else {
                return Err(RuntimeError::runtime(format!(
                    "`{name}` is not a struct value"
                )));
            };
            let target = fields
                .get(field)
                .cloned()
                .ok_or_else(|| RuntimeError::runtime(format!("unknown runtime field `{field}`")))?;
            fields.insert(field.clone(), coerce_like(value, &target)?);
            Ok(())
        }
        _ => Err(RuntimeError::runtime(
            "nested runtime paths deeper than one field are unsupported",
        )),
    }
}

fn coerce_like(value: Value, target: &Value) -> RuntimeResult<Value> {
    let value_type = match target {
        Value::String(_) => ValueType::String,
        Value::I64(_) => ValueType::Int,
        Value::I32(_) => ValueType::I32,
        Value::U32(_) => ValueType::U32,
        Value::U64(_) => ValueType::U64,
        Value::F64(_) => ValueType::Float,
        Value::Char(_) => ValueType::Char,
        Value::Bool(_) => ValueType::Bool,
        Value::Array(_) => ValueType::Array(Box::new(ValueType::Void)),
        Value::Struct { name, .. } => ValueType::Struct(name.clone(), Vec::new()),
        Value::Enum { name, .. } => ValueType::Enum(name.clone(), Vec::new()),
        Value::Void => ValueType::Void,
    };
    value.coerce(&value_type)
}

fn values_equal(left: &Value, right: &Value) -> RuntimeResult<bool> {
    if let Ok((left, right)) = numeric_pair(left.clone(), right.clone()) {
        return Ok(left == right);
    }
    Ok(left == right)
}

fn compare_values(left: Value, op: BinaryOp, right: Value) -> RuntimeResult<bool> {
    let (left, right) = numeric_pair(left, right)?;
    match (left, right) {
        (Value::I64(left), Value::I64(right)) => compare_order(left, op, right),
        (Value::I32(left), Value::I32(right)) => compare_order(left, op, right),
        (Value::U32(left), Value::U32(right)) => compare_order(left, op, right),
        (Value::U64(left), Value::U64(right)) => compare_order(left, op, right),
        (Value::F64(left), Value::F64(right)) => compare_order(left, op, right),
        _ => Err(RuntimeError::runtime("values are not comparable")),
    }
}

fn compare_order<T: PartialOrd>(left: T, op: BinaryOp, right: T) -> RuntimeResult<bool> {
    Ok(match op {
        BinaryOp::Less => left < right,
        BinaryOp::LessEqual => left <= right,
        BinaryOp::Greater => left > right,
        BinaryOp::GreaterEqual => left >= right,
        _ => return Err(RuntimeError::runtime("invalid comparison operator")),
    })
}

fn numeric_pair(left: Value, right: Value) -> RuntimeResult<(Value, Value)> {
    match (&left, &right) {
        (Value::F64(_), _) | (_, Value::F64(_)) => Ok((
            left.coerce(&ValueType::Float)?,
            right.coerce(&ValueType::Float)?,
        )),
        (Value::U64(_), _) | (_, Value::U64(_)) => Ok((
            left.coerce(&ValueType::U64)?,
            right.coerce(&ValueType::U64)?,
        )),
        (Value::U32(_), _) | (_, Value::U32(_)) => Ok((
            left.coerce(&ValueType::U32)?,
            right.coerce(&ValueType::U32)?,
        )),
        (Value::I32(_), _) | (_, Value::I32(_)) => Ok((
            left.coerce(&ValueType::I32)?,
            right.coerce(&ValueType::I32)?,
        )),
        (Value::I64(_), Value::I64(_)) => Ok((left, right)),
        _ => Err(RuntimeError::runtime("expected matching numeric values")),
    }
}

fn numeric_operation(left: Value, op: BinaryOp, right: Value) -> RuntimeResult<Value> {
    let (left, right) = numeric_pair(left, right)?;
    match (left, right) {
        (Value::I64(left), Value::I64(right)) => signed_operation(left, op, right).map(Value::I64),
        (Value::I32(left), Value::I32(right)) => signed_operation(left, op, right).map(Value::I32),
        (Value::U32(left), Value::U32(right)) => {
            unsigned_operation(left, op, right).map(Value::U32)
        }
        (Value::U64(left), Value::U64(right)) => {
            unsigned_operation(left, op, right).map(Value::U64)
        }
        (Value::F64(left), Value::F64(right)) => Ok(Value::F64(match op {
            BinaryOp::Add => left + right,
            BinaryOp::Subtract => left - right,
            BinaryOp::Multiply => left * right,
            BinaryOp::Divide => left / right,
            BinaryOp::Remainder => left % right,
            _ => return Err(RuntimeError::runtime("invalid floating-point operator")),
        })),
        _ => Err(RuntimeError::runtime("expected matching numeric values")),
    }
}

fn wrapping_numeric_operation(left: Value, op: BinaryOp, right: Value) -> RuntimeResult<Value> {
    let (left, right) = numeric_pair(left, right)?;
    macro_rules! wrapping {
        ($left:expr, $right:expr, $variant:ident) => {
            Ok(Value::$variant(match op {
                BinaryOp::Add => $left.wrapping_add($right),
                BinaryOp::Subtract => $left.wrapping_sub($right),
                BinaryOp::Multiply => $left.wrapping_mul($right),
                _ => {
                    return Err(RuntimeError::runtime(
                        "wrapping arithmetic supports add, subtract, and multiply",
                    ));
                }
            }))
        };
    }
    match (left, right) {
        (Value::I64(left), Value::I64(right)) => wrapping!(left, right, I64),
        (Value::I32(left), Value::I32(right)) => wrapping!(left, right, I32),
        (Value::U32(left), Value::U32(right)) => wrapping!(left, right, U32),
        (Value::U64(left), Value::U64(right)) => wrapping!(left, right, U64),
        _ => Err(RuntimeError::runtime(
            "wrapping arithmetic requires matching integer values",
        )),
    }
}

fn math_unary(function: MathUnaryFunction, value: Value) -> RuntimeResult<Value> {
    match (function, value) {
        (MathUnaryFunction::Abs, Value::I64(value)) => value
            .checked_abs()
            .map(Value::I64)
            .ok_or_else(|| RuntimeError::runtime("integer overflow")),
        (MathUnaryFunction::Abs, Value::I32(value)) => value
            .checked_abs()
            .map(Value::I32)
            .ok_or_else(|| RuntimeError::runtime("integer overflow")),
        (MathUnaryFunction::Abs, value @ (Value::U32(_) | Value::U64(_))) => Ok(value),
        (MathUnaryFunction::Abs, Value::F64(value)) => Ok(Value::F64(value.abs())),
        (MathUnaryFunction::Floor, Value::F64(value)) => Ok(Value::F64(value.floor())),
        (MathUnaryFunction::Ceil, Value::F64(value)) => Ok(Value::F64(value.ceil())),
        (MathUnaryFunction::Round, Value::F64(value)) => Ok(Value::F64(value.round())),
        (MathUnaryFunction::Sqrt, Value::F64(value)) => Ok(Value::F64(value.sqrt())),
        (MathUnaryFunction::Sin, Value::F64(value)) => Ok(Value::F64(value.sin())),
        (MathUnaryFunction::Cos, Value::F64(value)) => Ok(Value::F64(value.cos())),
        _ => Err(RuntimeError::runtime("invalid math operand")),
    }
}

fn math_binary(function: MathBinaryFunction, left: Value, right: Value) -> RuntimeResult<Value> {
    macro_rules! min_max {
        ($left:expr, $right:expr, $variant:ident) => {
            Ok(Value::$variant(match function {
                MathBinaryFunction::Min => $left.min($right),
                MathBinaryFunction::Max => $left.max($right),
                MathBinaryFunction::Pow => {
                    return Err(RuntimeError::runtime("math.pow requires f64 operands"));
                }
            }))
        };
    }
    match (left, right) {
        (Value::I64(left), Value::I64(right)) => min_max!(left, right, I64),
        (Value::I32(left), Value::I32(right)) => min_max!(left, right, I32),
        (Value::U32(left), Value::U32(right)) => min_max!(left, right, U32),
        (Value::U64(left), Value::U64(right)) => min_max!(left, right, U64),
        (Value::F64(left), Value::F64(right)) => Ok(Value::F64(match function {
            MathBinaryFunction::Min => left.min(right),
            MathBinaryFunction::Max => left.max(right),
            MathBinaryFunction::Pow => left.powf(right),
        })),
        _ => Err(RuntimeError::runtime(
            "math operation requires matching numeric operands",
        )),
    }
}

trait CheckedInteger:
    Copy
    + TryFrom<u32>
    + std::ops::BitOr<Output = Self>
    + std::ops::BitXor<Output = Self>
    + std::ops::BitAnd<Output = Self>
    + std::ops::Not<Output = Self>
{
    fn checked_add(self, rhs: Self) -> Option<Self>;
    fn checked_sub(self, rhs: Self) -> Option<Self>;
    fn checked_mul(self, rhs: Self) -> Option<Self>;
    fn checked_div(self, rhs: Self) -> Option<Self>;
    fn checked_rem(self, rhs: Self) -> Option<Self>;
    fn checked_shl(self, rhs: u32) -> Option<Self>;
    fn checked_shr(self, rhs: u32) -> Option<Self>;
    fn shift(self) -> RuntimeResult<u32>;
}

macro_rules! checked_integer {
    ($($type:ty),+ $(,)?) => {
        $(
            impl CheckedInteger for $type {
                fn checked_add(self, rhs: Self) -> Option<Self> { self.checked_add(rhs) }
                fn checked_sub(self, rhs: Self) -> Option<Self> { self.checked_sub(rhs) }
                fn checked_mul(self, rhs: Self) -> Option<Self> { self.checked_mul(rhs) }
                fn checked_div(self, rhs: Self) -> Option<Self> { self.checked_div(rhs) }
                fn checked_rem(self, rhs: Self) -> Option<Self> { self.checked_rem(rhs) }
                fn checked_shl(self, rhs: u32) -> Option<Self> { self.checked_shl(rhs) }
                fn checked_shr(self, rhs: u32) -> Option<Self> { self.checked_shr(rhs) }
                fn shift(self) -> RuntimeResult<u32> {
                    u32::try_from(self).map_err(|_| RuntimeError::runtime("invalid shift count"))
                }
            }
        )+
    };
}

checked_integer!(i64, i32, u32, u64);

fn integer_operation<T: CheckedInteger>(left: T, op: BinaryOp, right: T) -> RuntimeResult<T> {
    let value = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide => left.checked_div(right),
        BinaryOp::Remainder => left.checked_rem(right),
        BinaryOp::ShiftLeft => left.checked_shl(right.shift()?),
        BinaryOp::ShiftRight => left.checked_shr(right.shift()?),
        BinaryOp::BitOr => Some(left | right),
        BinaryOp::BitXor => Some(left ^ right),
        BinaryOp::BitAnd => Some(left & right),
        BinaryOp::BitAndNot => Some(left & !right),
        _ => return Err(RuntimeError::runtime("invalid integer operator")),
    };
    value.ok_or_else(|| RuntimeError::runtime("integer overflow or division by zero"))
}

fn signed_operation<T: CheckedInteger>(left: T, op: BinaryOp, right: T) -> RuntimeResult<T> {
    integer_operation(left, op, right)
}

fn unsigned_operation<T: CheckedInteger>(left: T, op: BinaryOp, right: T) -> RuntimeResult<T> {
    integer_operation(left, op, right)
}

fn option_value(payload: Option<Value>) -> Value {
    Value::Enum {
        name: "Option".to_string(),
        variant: if payload.is_some() { "Some" } else { "None" }.to_string(),
        payload: payload.map(Box::new),
    }
}

fn result_value(name: &str, result: Result<Value, Value>) -> Value {
    let (variant, payload) = match result {
        Ok(value) => ("Ok", value),
        Err(value) => ("Err", value),
    };
    Value::Enum {
        name: name.to_string(),
        variant: variant.to_string(),
        payload: Some(Box::new(payload)),
    }
}
