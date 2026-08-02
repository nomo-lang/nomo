use nomo_ir::{
    ArrayMutationMode, DeferredCall, Function, LoopKind, Program, Statement, TaskSelectOperation,
    ValueExpr, ValueType,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type BlockId = usize;
pub type StatementId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OwnershipState {
    Unique,
    Shared,
    Unknown,
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PanicKind {
    Bounds,
    Overflow,
    DivisionByZero,
    Explicit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallEffectSummary {
    pub may_escape_arguments: bool,
    pub may_suspend: bool,
    pub invalidates_ownership_proofs: bool,
}

impl CallEffectSummary {
    const SYNC_UNKNOWN: Self = Self {
        may_escape_arguments: true,
        may_suspend: false,
        invalidates_ownership_proofs: true,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    FreshArray {
        binding: String,
    },
    Retain {
        binding: String,
    },
    Release {
        binding: String,
    },
    AliasArray {
        target: String,
        source: String,
    },
    AssignUnknown {
        binding: String,
    },
    Call {
        effect: CallEffectSummary,
        array_arguments: Vec<String>,
    },
    Escape {
        bindings: Vec<String>,
    },
    ArrayMutation {
        binding: String,
    },
    CheckedArrayBounds {
        statement: StatementId,
        binding: String,
    },
    CowDetach {
        statement: StatementId,
        binding: String,
        flat_array: bool,
        elided: bool,
    },
    ArrayStore {
        statement: StatementId,
        binding: String,
    },
    Move {
        binding: String,
    },
    Read {
        binding: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Goto(BlockId),
    Branch {
        then_block: BlockId,
        else_block: BlockId,
    },
    Switch(Vec<BlockId>),
    Return,
    Panic,
    Unreachable,
}

impl Terminator {
    fn successors(&self) -> Vec<BlockId> {
        match self {
            Self::Goto(target) => vec![*target],
            Self::Branch {
                then_block,
                else_block,
            } => vec![*then_block, *else_block],
            Self::Switch(targets) => targets.clone(),
            Self::Return | Self::Panic | Self::Unreachable => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanicEdge {
    pub from: BlockId,
    pub to: BlockId,
    pub kind: PanicKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCfg {
    pub function: String,
    pub entry: BlockId,
    pub exit: BlockId,
    pub panic_exit: BlockId,
    pub blocks: Vec<BasicBlock>,
    pub panic_edges: Vec<PanicEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionOptimization {
    pub function: String,
    pub skipped_suspend: bool,
    pub cfg: Option<FunctionCfg>,
    pub checked_unique_stores: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationReport {
    pub functions: Vec<FunctionOptimization>,
    pub checked_unique_stores: usize,
}

/// Applies release-only ownership proofs to the typed IR.
///
/// This pass never removes a checked array access. It only changes the COW
/// detachment mode on a flat-array store after the CFG fixed point proves the
/// receiver unique on every normal predecessor. Suspend functions and any
/// unmodelled ownership effect retain the checked COW path.
pub fn optimize_release_program(program: &mut Program) -> OptimizationReport {
    reset_mutation_modes(program);
    let mut report = OptimizationReport::default();
    for function in &mut program.functions {
        if function.is_suspend {
            report.functions.push(FunctionOptimization {
                function: qualified_function_name(function),
                skipped_suspend: true,
                cfg: None,
                checked_unique_stores: 0,
            });
            continue;
        }

        let eligible = eligible_flat_array_bindings(function);
        let mut builder = CfgBuilder::new(function, eligible.clone());
        let mut cfg = builder.build();
        let proven = prove_unique_detaches(&mut cfg, &eligible);
        annotate_function(function, &proven);
        let checked_unique_stores = proven.len();
        report.checked_unique_stores += checked_unique_stores;
        report.functions.push(FunctionOptimization {
            function: qualified_function_name(function),
            skipped_suspend: false,
            cfg: Some(cfg),
            checked_unique_stores,
        });
    }
    report
}

fn qualified_function_name(function: &Function) -> String {
    if function.package.is_empty() {
        function.name.clone()
    } else {
        format!("{}.{}", function.package, function.name)
    }
}

fn reset_mutation_modes(program: &mut Program) {
    for function in &mut program.functions {
        reset_body_mutation_modes(&mut function.body);
    }
}

fn reset_body_mutation_modes(body: &mut [Statement]) {
    for statement in body {
        match statement {
            Statement::ArrayIndexAssign { mutation_mode, .. } => {
                *mutation_mode = ArrayMutationMode::CheckedCow;
            }
            Statement::Let { initializer, .. } => reset_expr_mutation_modes(initializer),
            Statement::LetIf {
                condition,
                body,
                else_body,
                ..
            } => {
                reset_expr_mutation_modes(condition);
                reset_body_mutation_modes(body);
                reset_body_mutation_modes(else_body);
            }
            Statement::LetMatch { value, arms, .. } | Statement::Match { value, arms, .. } => {
                reset_expr_mutation_modes(value);
                for arm in arms {
                    reset_body_mutation_modes(&mut arm.body);
                }
            }
            Statement::QuestionLet {
                result_expr,
                early_exit_actions,
                ..
            }
            | Statement::QuestionReturn {
                result_expr,
                early_exit_actions,
                ..
            } => {
                reset_expr_mutation_modes(result_expr);
                for action in early_exit_actions {
                    reset_expr_mutation_modes(action);
                }
            }
            Statement::LetElse {
                value, else_body, ..
            } => {
                reset_expr_mutation_modes(value);
                reset_body_mutation_modes(else_body);
            }
            Statement::IfLet {
                value,
                body,
                else_body,
                ..
            } => {
                reset_expr_mutation_modes(value);
                reset_body_mutation_modes(body);
                if let Some(else_body) = else_body {
                    reset_body_mutation_modes(else_body);
                }
            }
            Statement::If {
                condition,
                body,
                else_body,
            } => {
                reset_expr_mutation_modes(condition);
                reset_body_mutation_modes(body);
                reset_body_mutation_modes(else_body);
            }
            Statement::Assign { value, .. }
            | Statement::AssignField { value, .. }
            | Statement::Eprintln(value)
            | Statement::Eprint(value)
            | Statement::Println(value)
            | Statement::Print(value)
            | Statement::Panic(value)
            | Statement::Expr(value)
            | Statement::Return(Some(value)) => reset_expr_mutation_modes(value),
            Statement::TaskSelect { arms } => {
                for arm in arms {
                    match &mut arm.operation {
                        TaskSelectOperation::Receive { channel, .. } => {
                            reset_expr_mutation_modes(channel);
                        }
                        TaskSelectOperation::Send { channel, value, .. } => {
                            reset_expr_mutation_modes(channel);
                            reset_expr_mutation_modes(value);
                        }
                        TaskSelectOperation::Sleep { duration } => {
                            reset_expr_mutation_modes(duration);
                        }
                        TaskSelectOperation::Join { .. } => {}
                    }
                    reset_body_mutation_modes(&mut arm.body);
                }
            }
            Statement::Loop { kind, body } => {
                match kind {
                    LoopKind::Infinite => {}
                    LoopKind::While(condition) => reset_expr_mutation_modes(condition),
                    LoopKind::CStyle {
                        initializer,
                        condition,
                        update,
                        ..
                    } => {
                        reset_expr_mutation_modes(initializer);
                        reset_expr_mutation_modes(condition);
                        reset_expr_mutation_modes(update);
                    }
                    LoopKind::Iterate { iterable, .. } => reset_expr_mutation_modes(iterable),
                }
                reset_body_mutation_modes(body);
            }
            Statement::Defer { call } => match call {
                DeferredCall::Expr(expr)
                | DeferredCall::Println(expr)
                | DeferredCall::Print(expr)
                | DeferredCall::Eprintln(expr)
                | DeferredCall::Eprint(expr) => reset_expr_mutation_modes(expr),
            },
            Statement::Break | Statement::Continue | Statement::Return(None) => {}
        }
    }
}

fn reset_expr_mutation_modes(expr: &mut ValueExpr) {
    if let ValueExpr::ArraySet { mutation_mode, .. } = expr {
        *mutation_mode = ArrayMutationMode::CheckedCow;
    }
    match expr {
        ValueExpr::Binary { left, right, .. }
        | ValueExpr::StringCompare { left, right, .. }
        | ValueExpr::StringConcat { left, right }
        | ValueExpr::StringContains {
            value: left,
            needle: right,
        }
        | ValueExpr::StringStartsWith {
            value: left,
            prefix: right,
        }
        | ValueExpr::StringEndsWith {
            value: left,
            suffix: right,
        }
        | ValueExpr::StringSplit {
            value: left,
            separator: right,
        }
        | ValueExpr::PathJoin { left, right }
        | ValueExpr::NumBinary { left, right, .. }
        | ValueExpr::MathBinary { left, right, .. }
        | ValueExpr::CollectionsStringMapGet {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapContains {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringMapRemove {
            map: left,
            key: right,
        }
        | ValueExpr::CollectionsStringSetContains {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetInsert {
            set: left,
            value: right,
        }
        | ValueExpr::CollectionsStringSetRemove {
            set: left,
            value: right,
        }
        | ValueExpr::RegexIsMatch {
            regex: left,
            value: right,
        }
        | ValueExpr::RegexCaptures {
            regex: left,
            value: right,
        }
        | ValueExpr::NetConnect {
            host: left,
            port: right,
        }
        | ValueExpr::NetListen {
            host: left,
            port: right,
        }
        | ValueExpr::NetUdpBind {
            host: left,
            port: right,
        }
        | ValueExpr::UdpSocketRecvFromString {
            socket: left,
            max_bytes: right,
        }
        | ValueExpr::TcpStreamWriteString {
            stream: left,
            content: right,
        }
        | ValueExpr::FsWriteString {
            path: left,
            content: right,
        }
        | ValueExpr::FsWriteBytes {
            path: left,
            bytes: right,
        }
        | ValueExpr::EnvSet {
            name: left,
            value: right,
        }
        | ValueExpr::HashWriteString {
            state: left,
            value: right,
        }
        | ValueExpr::HashWriteBytes {
            state: left,
            value: right,
        }
        | ValueExpr::FileWriteString {
            file: left,
            content: right,
        }
        | ValueExpr::ArrayGet {
            array: left,
            index: right,
            ..
        }
        | ValueExpr::ArrayIndex {
            array: left,
            index: right,
            ..
        }
        | ValueExpr::ResultUnwrapOr {
            result: left,
            default: right,
            ..
        }
        | ValueExpr::OptionUnwrapOr {
            option: left,
            default: right,
            ..
        } => {
            reset_expr_mutation_modes(left);
            reset_expr_mutation_modes(right);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            reset_expr_mutation_modes(map);
            reset_expr_mutation_modes(key);
            reset_expr_mutation_modes(value);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            reset_expr_mutation_modes(socket);
            reset_expr_mutation_modes(content);
            reset_expr_mutation_modes(host);
            reset_expr_mutation_modes(port);
        }
        ValueExpr::Call { args, .. }
        | ValueExpr::JsonStructured { args, .. }
        | ValueExpr::JsonRpc { args, .. }
        | ValueExpr::Cron { args, .. } => {
            for arg in args {
                reset_expr_mutation_modes(arg);
            }
        }
        ValueExpr::ArrayLiteral { elements, .. } => {
            for element in elements {
                reset_expr_mutation_modes(element);
            }
        }
        ValueExpr::ArraySet { index, value, .. } | ValueExpr::ArrayInsert { index, value, .. } => {
            reset_expr_mutation_modes(index);
            reset_expr_mutation_modes(value);
        }
        ValueExpr::ArrayRemove { index, .. } | ValueExpr::ArrayPush { value: index, .. } => {
            reset_expr_mutation_modes(index);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsOpen { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FileClose { file: path }
        | ValueExpr::FileReadToString { file: path }
        | ValueExpr::TcpListenerAccept { listener: path }
        | ValueExpr::TcpListenerClose { listener: path }
        | ValueExpr::TcpStreamClose { stream: path }
        | ValueExpr::TcpStreamReadToString { stream: path }
        | ValueExpr::UdpSocketClose { socket: path }
        | ValueExpr::EnvGet { name: path }
        | ValueExpr::StringLen { value: path }
        | ValueExpr::StringIsEmpty { value: path }
        | ValueExpr::StringTrim { value: path }
        | ValueExpr::StringToLower { value: path }
        | ValueExpr::StringToUpper { value: path }
        | ValueExpr::CharIsDigit { value: path }
        | ValueExpr::CharIsAlpha { value: path }
        | ValueExpr::CharIsWhitespace { value: path }
        | ValueExpr::CharToString { value: path }
        | ValueExpr::PathBasename { path }
        | ValueExpr::PathDirname { path }
        | ValueExpr::PathExtension { path }
        | ValueExpr::PathNormalize { path }
        | ValueExpr::PathIsAbsolute { path }
        | ValueExpr::MathUnary { value: path, .. }
        | ValueExpr::TimeDurationMillis { millis: path }
        | ValueExpr::TimeDurationSeconds { seconds: path }
        | ValueExpr::TimeDurationAsMillis { duration: path }
        | ValueExpr::TimeFormatDuration { duration: path }
        | ValueExpr::TimeSleep { duration: path }
        | ValueExpr::TimeSleepMillis { duration: path }
        | ValueExpr::LogEnabled { level: path }
        | ValueExpr::HashString { value: path }
        | ValueExpr::HashBytes { value: path }
        | ValueExpr::HashFinish { state: path }
        | ValueExpr::CryptoSha256 { value: path }
        | ValueExpr::CryptoSha512 { value: path }
        | ValueExpr::CryptoRandomBytes { count: path }
        | ValueExpr::JsonParse { value: path }
        | ValueExpr::JsonStringify { value: path }
        | ValueExpr::RegexCompile { pattern: path }
        | ValueExpr::CollectionsStringMapLen { map: path }
        | ValueExpr::CollectionsStringSetLen { set: path }
        | ValueExpr::ProcessExit { code: path }
        | ValueExpr::ProcessSpawn { command: path }
        | ValueExpr::ProcessStatus { command: path }
        | ValueExpr::ProcessExec { command: path }
        | ValueExpr::ProcessOutput { command: path }
        | ValueExpr::NumParseI64 { value: path }
        | ValueExpr::NumParseU64 { value: path }
        | ValueExpr::NumParseF64 { value: path }
        | ValueExpr::NumToString { value: path, .. }
        | ValueExpr::Unary { expr: path, .. }
        | ValueExpr::Cast { expr: path, .. }
        | ValueExpr::ResultMapErr { result: path, .. }
        | ValueExpr::ResultIsOk { result: path, .. }
        | ValueExpr::ResultIsErr { result: path, .. }
        | ValueExpr::ResultMap { result: path, .. }
        | ValueExpr::ResultAndThen { result: path, .. }
        | ValueExpr::OptionIsSome { option: path, .. }
        | ValueExpr::OptionIsNone { option: path, .. }
        | ValueExpr::OptionMap { option: path, .. }
        | ValueExpr::OptionAndThen { option: path, .. }
        | ValueExpr::EnumPayload { value: path, .. }
        | ValueExpr::EnumPayloadFieldAccess { value: path, .. }
        | ValueExpr::ArrayIter { array: path, .. }
        | ValueExpr::ArrayLen { array: path }
        | ValueExpr::Panic { message: path, .. } => reset_expr_mutation_modes(path),
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                reset_expr_mutation_modes(value);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                reset_expr_mutation_modes(payload);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            reset_expr_mutation_modes(condition);
            reset_expr_mutation_modes(then_branch);
            reset_expr_mutation_modes(else_branch);
        }
        ValueExpr::Match { value, arms } => {
            reset_expr_mutation_modes(value);
            for arm in arms {
                reset_expr_mutation_modes(&mut arm.value);
            }
        }
        ValueExpr::StringLiteral(_)
        | ValueExpr::IntLiteral(_)
        | ValueExpr::FloatLiteral(_)
        | ValueExpr::CharLiteral(_)
        | ValueExpr::BoolLiteral(_)
        | ValueExpr::VoidLiteral
        | ValueExpr::HashNew
        | ValueExpr::CollectionsStringMapNew
        | ValueExpr::CollectionsStringSetNew
        | ValueExpr::Variable(_)
        | ValueExpr::FunctionRef(_)
        | ValueExpr::MutBorrow(_)
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::ArrayPop { .. }
        | ValueExpr::ArrayClear { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn is_primitive_pod(value_type: &ValueType) -> bool {
    matches!(
        value_type,
        ValueType::Bool
            | ValueType::Char
            | ValueType::Int
            | ValueType::I32
            | ValueType::U32
            | ValueType::U64
            | ValueType::Float
    )
}

fn is_flat_pod_array(value_type: &ValueType) -> bool {
    matches!(
        value_type,
        ValueType::Array(element) if is_primitive_pod(element)
    )
}

fn eligible_flat_array_bindings(function: &Function) -> BTreeSet<String> {
    if body_requires_safe_fallback(&function.body) {
        return BTreeSet::new();
    }
    let mut counts = BTreeMap::<String, usize>::new();
    let mut flat = BTreeSet::new();
    for parameter in &function.params {
        *counts.entry(parameter.name.clone()).or_default() += 1;
        if is_flat_pod_array(&parameter.value_type) {
            flat.insert(parameter.name.clone());
        }
    }
    collect_binding_names(&function.body, &mut counts, &mut flat);
    flat.retain(|name| counts.get(name) == Some(&1));
    flat
}

fn body_requires_safe_fallback(body: &[Statement]) -> bool {
    body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Defer { .. } | Statement::TaskSelect { .. }
        ) || statement_bodies(statement)
            .into_iter()
            .any(body_requires_safe_fallback)
    })
}

fn record_binding(
    name: &str,
    value_type: Option<&ValueType>,
    counts: &mut BTreeMap<String, usize>,
    flat: &mut BTreeSet<String>,
) {
    *counts.entry(name.to_string()).or_default() += 1;
    if value_type.is_some_and(is_flat_pod_array) {
        flat.insert(name.to_string());
    }
}

fn collect_binding_names(
    body: &[Statement],
    counts: &mut BTreeMap<String, usize>,
    flat: &mut BTreeSet<String>,
) {
    for statement in body {
        match statement {
            Statement::Let {
                name, value_type, ..
            }
            | Statement::LetIf {
                name, value_type, ..
            }
            | Statement::QuestionLet {
                name, value_type, ..
            } => record_binding(name, Some(value_type), counts, flat),
            Statement::LetMatch {
                name,
                value_type,
                arms,
                ..
            } => {
                record_binding(name, Some(value_type), counts, flat);
                for arm in arms {
                    if let Some(binding) = &arm.binding {
                        record_binding(binding, None, counts, flat);
                    }
                }
            }
            Statement::LetElse {
                binding,
                value_type,
                ..
            } => record_binding(binding, Some(value_type), counts, flat),
            Statement::IfLet {
                binding: Some(binding),
                value_type,
                ..
            } => record_binding(binding, value_type.as_ref(), counts, flat),
            Statement::Loop { kind, .. } => match kind {
                LoopKind::CStyle {
                    binding,
                    value_type,
                    ..
                } => record_binding(binding, Some(value_type), counts, flat),
                LoopKind::Iterate {
                    binding,
                    element_type,
                    ..
                } => record_binding(binding, Some(element_type), counts, flat),
                LoopKind::Infinite | LoopKind::While(_) => {}
            },
            Statement::Match { arms, .. } => {
                for arm in arms {
                    if let Some(binding) = &arm.binding {
                        record_binding(binding, None, counts, flat);
                    }
                }
            }
            Statement::TaskSelect { arms } => {
                for arm in arms {
                    record_binding(&arm.binding, Some(&arm.binding_type), counts, flat);
                }
            }
            _ => {}
        }

        match statement {
            Statement::LetIf {
                body, else_body, ..
            }
            | Statement::If {
                body, else_body, ..
            } => {
                collect_binding_names(body, counts, flat);
                collect_binding_names(else_body, counts, flat);
            }
            Statement::LetMatch { arms, .. } | Statement::Match { arms, .. } => {
                for arm in arms {
                    collect_binding_names(&arm.body, counts, flat);
                }
            }
            Statement::LetElse { else_body, .. } => {
                collect_binding_names(else_body, counts, flat);
            }
            Statement::IfLet {
                body, else_body, ..
            } => {
                collect_binding_names(body, counts, flat);
                if let Some(else_body) = else_body {
                    collect_binding_names(else_body, counts, flat);
                }
            }
            Statement::TaskSelect { arms } => {
                for arm in arms {
                    collect_binding_names(&arm.body, counts, flat);
                }
            }
            Statement::Loop { body, .. } => collect_binding_names(body, counts, flat),
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum PathStep {
    Statement(usize),
    Branch(usize),
}

type StatementPath = Vec<PathStep>;

fn assign_statement_ids(
    body: &[Statement],
    prefix: &[PathStep],
    next: &mut StatementId,
    sites: &mut BTreeMap<StatementPath, StatementId>,
) {
    for (index, statement) in body.iter().enumerate() {
        let mut path = prefix.to_vec();
        path.push(PathStep::Statement(index));
        sites.insert(path.clone(), *next);
        *next += 1;
        for (branch, child) in statement_bodies(statement).into_iter().enumerate() {
            let mut child_prefix = path.clone();
            child_prefix.push(PathStep::Branch(branch));
            assign_statement_ids(child, &child_prefix, next, sites);
        }
    }
}

fn statement_bodies(statement: &Statement) -> Vec<&[Statement]> {
    match statement {
        Statement::LetIf {
            body, else_body, ..
        }
        | Statement::If {
            body, else_body, ..
        } => vec![body, else_body],
        Statement::LetMatch { arms, .. } | Statement::Match { arms, .. } => {
            arms.iter().map(|arm| arm.body.as_slice()).collect()
        }
        Statement::LetElse { else_body, .. } => vec![else_body],
        Statement::IfLet {
            body, else_body, ..
        } => {
            let mut result = vec![body.as_slice()];
            if let Some(else_body) = else_body {
                result.push(else_body);
            }
            result
        }
        Statement::TaskSelect { arms } => arms.iter().map(|arm| arm.body.as_slice()).collect(),
        Statement::Loop { body, .. } => vec![body],
        _ => Vec::new(),
    }
}

struct CfgBuilder<'a> {
    function: &'a Function,
    eligible: BTreeSet<String>,
    sites: BTreeMap<StatementPath, StatementId>,
    blocks: Vec<BasicBlock>,
    panic_edges: Vec<PanicEdge>,
    exit: BlockId,
    panic_exit: BlockId,
}

impl<'a> CfgBuilder<'a> {
    fn new(function: &'a Function, eligible: BTreeSet<String>) -> Self {
        let mut next = 0;
        let mut sites = BTreeMap::new();
        assign_statement_ids(&function.body, &[], &mut next, &mut sites);
        let mut builder = Self {
            function,
            eligible,
            sites,
            blocks: Vec::new(),
            panic_edges: Vec::new(),
            exit: 0,
            panic_exit: 0,
        };
        builder.exit = builder.push_block(Vec::new(), Terminator::Return);
        builder.panic_exit = builder.push_block(Vec::new(), Terminator::Panic);
        builder
    }

    fn build(&mut self) -> FunctionCfg {
        let entry = self.build_sequence(&self.function.body, &[], self.exit, None, None);
        FunctionCfg {
            function: qualified_function_name(self.function),
            entry,
            exit: self.exit,
            panic_exit: self.panic_exit,
            blocks: std::mem::take(&mut self.blocks),
            panic_edges: std::mem::take(&mut self.panic_edges),
        }
    }

    fn push_block(&mut self, operations: Vec<Operation>, terminator: Terminator) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            operations,
            terminator,
        });
        id
    }

    fn add_panic_edges(&mut self, from: BlockId, kinds: BTreeSet<PanicKind>) {
        for kind in kinds {
            self.panic_edges.push(PanicEdge {
                from,
                to: self.panic_exit,
                kind,
            });
        }
    }

    fn build_sequence(
        &mut self,
        body: &[Statement],
        prefix: &[PathStep],
        next: BlockId,
        break_target: Option<BlockId>,
        continue_target: Option<BlockId>,
    ) -> BlockId {
        let mut successor = next;
        for (index, statement) in body.iter().enumerate().rev() {
            let mut path = prefix.to_vec();
            path.push(PathStep::Statement(index));
            successor =
                self.build_statement(statement, &path, successor, break_target, continue_target);
        }
        successor
    }

    fn build_statement(
        &mut self,
        statement: &Statement,
        path: &[PathStep],
        next: BlockId,
        break_target: Option<BlockId>,
        continue_target: Option<BlockId>,
    ) -> BlockId {
        let statement_id = self.sites[path];
        let (operations, panic_kinds) =
            statement_operations(statement, statement_id, &self.eligible);
        let block = self.push_block(operations, Terminator::Unreachable);
        let terminator = match statement {
            Statement::LetIf {
                body, else_body, ..
            }
            | Statement::If {
                body, else_body, ..
            } => {
                let then_block =
                    self.build_child_body(body, path, 0, next, break_target, continue_target);
                let else_block =
                    self.build_child_body(else_body, path, 1, next, break_target, continue_target);
                Terminator::Branch {
                    then_block,
                    else_block,
                }
            }
            Statement::LetMatch { arms, .. } | Statement::Match { arms, .. } => {
                let targets = arms
                    .iter()
                    .enumerate()
                    .map(|(branch, arm)| {
                        self.build_child_body(
                            &arm.body,
                            path,
                            branch,
                            next,
                            break_target,
                            continue_target,
                        )
                    })
                    .collect();
                Terminator::Switch(targets)
            }
            Statement::LetElse { else_body, .. } => {
                let else_block =
                    self.build_child_body(else_body, path, 0, next, break_target, continue_target);
                Terminator::Branch {
                    then_block: next,
                    else_block,
                }
            }
            Statement::IfLet {
                body, else_body, ..
            } => {
                let then_block =
                    self.build_child_body(body, path, 0, next, break_target, continue_target);
                let else_block = else_body.as_ref().map_or(next, |body| {
                    self.build_child_body(body, path, 1, next, break_target, continue_target)
                });
                Terminator::Branch {
                    then_block,
                    else_block,
                }
            }
            Statement::TaskSelect { arms } => {
                let targets = arms
                    .iter()
                    .enumerate()
                    .map(|(branch, arm)| {
                        self.build_child_body(
                            &arm.body,
                            path,
                            branch,
                            next,
                            break_target,
                            continue_target,
                        )
                    })
                    .collect();
                Terminator::Switch(targets)
            }
            Statement::Loop { kind, body } => {
                let body_entry =
                    self.build_child_body(body, path, 0, block, Some(next), Some(block));
                match kind {
                    LoopKind::Infinite => Terminator::Goto(body_entry),
                    LoopKind::While(_) | LoopKind::CStyle { .. } | LoopKind::Iterate { .. } => {
                        Terminator::Branch {
                            then_block: body_entry,
                            else_block: next,
                        }
                    }
                }
            }
            Statement::Break => break_target.map_or(Terminator::Unreachable, Terminator::Goto),
            Statement::Continue => {
                continue_target.map_or(Terminator::Unreachable, Terminator::Goto)
            }
            Statement::Return(_) | Statement::QuestionReturn { .. } => Terminator::Return,
            Statement::Panic(_) => Terminator::Panic,
            Statement::QuestionLet { .. } => Terminator::Branch {
                then_block: next,
                else_block: self.exit,
            },
            _ => Terminator::Goto(next),
        };
        self.blocks[block].terminator = terminator;
        self.add_panic_edges(block, panic_kinds);
        block
    }

    fn build_child_body(
        &mut self,
        body: &[Statement],
        path: &[PathStep],
        branch: usize,
        next: BlockId,
        break_target: Option<BlockId>,
        continue_target: Option<BlockId>,
    ) -> BlockId {
        let mut prefix = path.to_vec();
        prefix.push(PathStep::Branch(branch));
        self.build_sequence(body, &prefix, next, break_target, continue_target)
    }
}

fn statement_operations(
    statement: &Statement,
    statement_id: StatementId,
    eligible: &BTreeSet<String>,
) -> (Vec<Operation>, BTreeSet<PanicKind>) {
    let mut effects = ExprEffects::default();
    match statement {
        Statement::Let {
            name, initializer, ..
        } => assign_operations(
            name,
            initializer,
            false,
            statement_id,
            eligible,
            &mut effects,
        ),
        Statement::Assign { name, value } => {
            assign_operations(name, value, true, statement_id, eligible, &mut effects);
        }
        Statement::ArrayIndexAssign {
            root,
            indices,
            array_types,
            value,
            ..
        } => {
            for index in indices {
                collect_expr_effects(index, eligible, ExprContext::Value, &mut effects);
            }
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
            effects.flush_uses();
            if eligible.contains(root) {
                effects.operations.push(Operation::CheckedArrayBounds {
                    statement: statement_id,
                    binding: root.clone(),
                });
                effects.operations.push(Operation::CowDetach {
                    statement: statement_id,
                    binding: root.clone(),
                    flat_array: indices.len() == 1
                        && array_types.len() == 1
                        && is_primitive_pod(&array_types[0]),
                    elided: false,
                });
                effects.operations.push(Operation::ArrayStore {
                    statement: statement_id,
                    binding: root.clone(),
                });
            }
            effects.panic_kinds.insert(PanicKind::Bounds);
        }
        Statement::LetIf {
            condition, name, ..
        } => {
            collect_expr_effects(condition, eligible, ExprContext::Value, &mut effects);
            assign_unknown(name, false, eligible, &mut effects.operations);
        }
        Statement::LetMatch { value, name, .. } => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
            assign_unknown(name, false, eligible, &mut effects.operations);
        }
        Statement::QuestionLet {
            name, result_expr, ..
        } => {
            collect_expr_effects(result_expr, eligible, ExprContext::Value, &mut effects);
            assign_unknown(name, false, eligible, &mut effects.operations);
        }
        Statement::QuestionReturn { result_expr, .. } => {
            collect_expr_effects(result_expr, eligible, ExprContext::Move, &mut effects);
        }
        Statement::LetElse { binding, value, .. } => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
            assign_unknown(binding, false, eligible, &mut effects.operations);
        }
        Statement::IfLet { value, binding, .. } => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
            if let Some(binding) = binding {
                assign_unknown(binding, false, eligible, &mut effects.operations);
            }
        }
        Statement::If { condition, .. } => {
            collect_expr_effects(condition, eligible, ExprContext::Value, &mut effects);
        }
        Statement::AssignField { base, value, .. } => {
            if eligible.contains(base) {
                effects.escapes.insert(base.clone());
            }
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
        }
        Statement::Eprintln(value)
        | Statement::Eprint(value)
        | Statement::Println(value)
        | Statement::Print(value)
        | Statement::Expr(value) => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
        }
        Statement::Panic(value) => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
            effects.panic_kinds.insert(PanicKind::Explicit);
        }
        Statement::Return(Some(value)) => {
            collect_expr_effects(value, eligible, ExprContext::Move, &mut effects);
        }
        Statement::Match { value, .. } => {
            collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
        }
        Statement::TaskSelect { arms } => {
            for arm in arms {
                match &arm.operation {
                    TaskSelectOperation::Receive { channel, .. } => {
                        collect_expr_effects(channel, eligible, ExprContext::Value, &mut effects);
                    }
                    TaskSelectOperation::Send { channel, value, .. } => {
                        collect_expr_effects(channel, eligible, ExprContext::Value, &mut effects);
                        collect_expr_effects(value, eligible, ExprContext::Value, &mut effects);
                    }
                    TaskSelectOperation::Sleep { duration } => {
                        collect_expr_effects(duration, eligible, ExprContext::Value, &mut effects);
                    }
                    TaskSelectOperation::Join { .. } => {}
                }
            }
        }
        Statement::Loop { kind, .. } => match kind {
            LoopKind::Infinite => {}
            LoopKind::While(condition) => {
                collect_expr_effects(condition, eligible, ExprContext::Value, &mut effects);
            }
            LoopKind::CStyle {
                initializer,
                condition,
                update,
                ..
            } => {
                collect_expr_effects(initializer, eligible, ExprContext::Value, &mut effects);
                collect_expr_effects(condition, eligible, ExprContext::Value, &mut effects);
                collect_expr_effects(update, eligible, ExprContext::Value, &mut effects);
            }
            LoopKind::Iterate { iterable, .. } => {
                collect_expr_effects(iterable, eligible, ExprContext::Value, &mut effects);
            }
        },
        Statement::Defer { call } => {
            let expr = match call {
                DeferredCall::Expr(expr)
                | DeferredCall::Println(expr)
                | DeferredCall::Print(expr)
                | DeferredCall::Eprintln(expr)
                | DeferredCall::Eprint(expr) => expr,
            };
            collect_expr_effects(expr, eligible, ExprContext::Value, &mut effects);
        }
        Statement::Break | Statement::Continue | Statement::Return(None) => {}
    }
    effects.finish()
}

fn assign_unknown(
    name: &str,
    overwrite: bool,
    eligible: &BTreeSet<String>,
    operations: &mut Vec<Operation>,
) {
    if eligible.contains(name) {
        if overwrite {
            operations.push(Operation::Release {
                binding: name.to_string(),
            });
        }
        operations.push(Operation::AssignUnknown {
            binding: name.to_string(),
        });
    }
}

fn assign_operations(
    name: &str,
    value: &ValueExpr,
    overwrite: bool,
    statement_id: StatementId,
    eligible: &BTreeSet<String>,
    effects: &mut ExprEffects,
) {
    if eligible.contains(name) {
        match value {
            ValueExpr::ArrayNew { element_type } | ValueExpr::ArrayLiteral { element_type, .. }
                if is_primitive_pod(element_type) =>
            {
                for child in value.child_expressions() {
                    collect_expr_effects(child, eligible, ExprContext::Value, effects);
                }
                effects.flush_uses();
                if overwrite {
                    effects.operations.push(Operation::Release {
                        binding: name.to_string(),
                    });
                }
                effects.operations.push(Operation::FreshArray {
                    binding: name.to_string(),
                });
                return;
            }
            ValueExpr::Variable(source) if eligible.contains(source) => {
                if overwrite {
                    effects.operations.push(Operation::Release {
                        binding: name.to_string(),
                    });
                }
                effects.operations.push(Operation::Retain {
                    binding: source.clone(),
                });
                effects.operations.push(Operation::AliasArray {
                    target: name.to_string(),
                    source: source.clone(),
                });
                return;
            }
            ValueExpr::ArraySet {
                array,
                index,
                value,
                element_type,
                ..
            } if array == name && is_primitive_pod(element_type) => {
                collect_expr_effects(index, eligible, ExprContext::Value, effects);
                collect_expr_effects(value, eligible, ExprContext::Value, effects);
                effects.flush_uses();
                effects.operations.push(Operation::CheckedArrayBounds {
                    statement: statement_id,
                    binding: name.to_string(),
                });
                effects.operations.push(Operation::CowDetach {
                    statement: statement_id,
                    binding: name.to_string(),
                    flat_array: true,
                    elided: false,
                });
                effects.operations.push(Operation::ArrayStore {
                    statement: statement_id,
                    binding: name.to_string(),
                });
                effects.panic_kinds.insert(PanicKind::Bounds);
                return;
            }
            ValueExpr::ArrayPush { array, value, .. } if array == name => {
                collect_expr_effects(value, eligible, ExprContext::Value, effects);
                effects.flush_uses();
                effects.operations.push(Operation::ArrayMutation {
                    binding: name.to_string(),
                });
                return;
            }
            ValueExpr::ArrayInsert {
                array,
                index,
                value,
                ..
            } if array == name => {
                collect_expr_effects(index, eligible, ExprContext::Value, effects);
                collect_expr_effects(value, eligible, ExprContext::Value, effects);
                effects.flush_uses();
                effects.operations.push(Operation::ArrayMutation {
                    binding: name.to_string(),
                });
                effects.panic_kinds.insert(PanicKind::Bounds);
                return;
            }
            ValueExpr::ArrayClear { array, .. }
            | ValueExpr::ArrayPop { array, .. }
            | ValueExpr::ArrayRemove { array, .. }
                if array == name =>
            {
                for child in value.child_expressions() {
                    collect_expr_effects(child, eligible, ExprContext::Value, effects);
                }
                effects.flush_uses();
                effects.operations.push(Operation::ArrayMutation {
                    binding: name.to_string(),
                });
                return;
            }
            _ => {}
        }
    }

    collect_expr_effects(value, eligible, ExprContext::Value, effects);
    effects.flush_uses();
    assign_unknown(name, overwrite, eligible, &mut effects.operations);
}

#[derive(Debug, Clone, Copy)]
enum ExprContext {
    Value,
    Read,
    Move,
}

#[derive(Default)]
struct ExprEffects {
    operations: Vec<Operation>,
    escapes: BTreeSet<String>,
    moves: BTreeSet<String>,
    reads: BTreeSet<String>,
    panic_kinds: BTreeSet<PanicKind>,
}

impl ExprEffects {
    fn flush_uses(&mut self) {
        if !self.escapes.is_empty() {
            self.operations.push(Operation::Escape {
                bindings: std::mem::take(&mut self.escapes).into_iter().collect(),
            });
        }
        for binding in std::mem::take(&mut self.moves) {
            self.operations.push(Operation::Move { binding });
        }
        for binding in std::mem::take(&mut self.reads) {
            self.operations.push(Operation::Read { binding });
        }
    }

    fn finish(mut self) -> (Vec<Operation>, BTreeSet<PanicKind>) {
        self.flush_uses();
        (self.operations, self.panic_kinds)
    }
}

fn collect_expr_effects(
    expr: &ValueExpr,
    eligible: &BTreeSet<String>,
    context: ExprContext,
    effects: &mut ExprEffects,
) {
    match expr {
        ValueExpr::Variable(name) => record_variable_use(name, eligible, context, effects),
        ValueExpr::MutBorrow(path) => {
            if let Some(root) = path.first().filter(|root| eligible.contains(*root)) {
                effects.escapes.insert(root.clone());
            }
        }
        ValueExpr::FieldAccess { base, .. } => {
            record_variable_use(base, eligible, ExprContext::Value, effects);
        }
        ValueExpr::ArrayLen { array } => {
            collect_expr_effects(array, eligible, ExprContext::Read, effects);
        }
        ValueExpr::ArrayIndex { array, index, .. } | ValueExpr::ArrayGet { array, index, .. } => {
            collect_expr_effects(array, eligible, ExprContext::Read, effects);
            collect_expr_effects(index, eligible, ExprContext::Value, effects);
            if matches!(expr, ValueExpr::ArrayIndex { .. }) {
                effects.panic_kinds.insert(PanicKind::Bounds);
            }
        }
        ValueExpr::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            collect_expr_effects(index, eligible, ExprContext::Value, effects);
            collect_expr_effects(value, eligible, ExprContext::Value, effects);
            if eligible.contains(array) {
                effects.operations.push(Operation::ArrayMutation {
                    binding: array.clone(),
                });
            }
            effects.panic_kinds.insert(PanicKind::Bounds);
        }
        ValueExpr::ArrayPush { array, value, .. } => {
            collect_expr_effects(value, eligible, ExprContext::Value, effects);
            if eligible.contains(array) {
                effects.operations.push(Operation::ArrayMutation {
                    binding: array.clone(),
                });
            }
        }
        ValueExpr::ArrayInsert {
            array,
            index,
            value,
            ..
        } => {
            collect_expr_effects(index, eligible, ExprContext::Value, effects);
            collect_expr_effects(value, eligible, ExprContext::Value, effects);
            if eligible.contains(array) {
                effects.operations.push(Operation::ArrayMutation {
                    binding: array.clone(),
                });
            }
            effects.panic_kinds.insert(PanicKind::Bounds);
        }
        ValueExpr::ArrayRemove { array, index, .. } => {
            collect_expr_effects(index, eligible, ExprContext::Value, effects);
            if eligible.contains(array) {
                effects.operations.push(Operation::ArrayMutation {
                    binding: array.clone(),
                });
            }
        }
        ValueExpr::ArrayPop { array, .. } | ValueExpr::ArrayClear { array, .. } => {
            if eligible.contains(array) {
                effects.operations.push(Operation::ArrayMutation {
                    binding: array.clone(),
                });
            }
        }
        ValueExpr::Call { args, .. } => {
            let mut call_arrays = BTreeSet::new();
            for arg in args {
                collect_array_variables(arg, eligible, &mut call_arrays);
                collect_expr_effects(arg, eligible, ExprContext::Value, effects);
            }
            effects.operations.push(Operation::Call {
                effect: CallEffectSummary::SYNC_UNKNOWN,
                array_arguments: call_arrays.into_iter().collect(),
            });
        }
        ValueExpr::Binary { op, value_type, .. } => {
            if value_type.is_integer() {
                use nomo_ir::BinaryOp;
                match op {
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::ShiftLeft
                    | BinaryOp::ShiftRight => {
                        effects.panic_kinds.insert(PanicKind::Overflow);
                    }
                    BinaryOp::Divide | BinaryOp::Remainder => {
                        effects.panic_kinds.insert(PanicKind::DivisionByZero);
                        effects.panic_kinds.insert(PanicKind::Overflow);
                    }
                    _ => {}
                }
            }
            for child in expr.child_expressions() {
                collect_expr_effects(child, eligible, ExprContext::Value, effects);
            }
        }
        ValueExpr::Panic { message, .. } => {
            collect_expr_effects(message, eligible, ExprContext::Value, effects);
            effects.panic_kinds.insert(PanicKind::Explicit);
        }
        _ => {
            for child in expr.child_expressions() {
                collect_expr_effects(child, eligible, ExprContext::Value, effects);
            }
        }
    }
}

fn collect_array_variables(
    expr: &ValueExpr,
    eligible: &BTreeSet<String>,
    arrays: &mut BTreeSet<String>,
) {
    match expr {
        ValueExpr::Variable(name) | ValueExpr::FieldAccess { base: name, .. } => {
            if eligible.contains(name) {
                arrays.insert(name.clone());
            }
        }
        ValueExpr::MutBorrow(path) => {
            if let Some(root) = path.first().filter(|root| eligible.contains(*root)) {
                arrays.insert(root.clone());
            }
        }
        ValueExpr::ArrayPop { array, .. }
        | ValueExpr::ArrayRemove { array, .. }
        | ValueExpr::ArrayPush { array, .. }
        | ValueExpr::ArraySet { array, .. }
        | ValueExpr::ArrayInsert { array, .. }
        | ValueExpr::ArrayClear { array, .. } => {
            if eligible.contains(array) {
                arrays.insert(array.clone());
            }
            for child in expr.child_expressions() {
                collect_array_variables(child, eligible, arrays);
            }
        }
        _ => {
            for child in expr.child_expressions() {
                collect_array_variables(child, eligible, arrays);
            }
        }
    }
}

fn record_variable_use(
    name: &str,
    eligible: &BTreeSet<String>,
    context: ExprContext,
    effects: &mut ExprEffects,
) {
    if !eligible.contains(name) {
        return;
    }
    match context {
        ExprContext::Value => {
            effects.escapes.insert(name.to_string());
        }
        ExprContext::Read => {
            effects.reads.insert(name.to_string());
        }
        ExprContext::Move => {
            effects.moves.insert(name.to_string());
        }
    }
}

type StateMap = BTreeMap<String, OwnershipState>;

fn prove_unique_detaches(
    cfg: &mut FunctionCfg,
    eligible: &BTreeSet<String>,
) -> BTreeSet<StatementId> {
    let initial: StateMap = eligible
        .iter()
        .map(|name| (name.clone(), OwnershipState::Unknown))
        .collect();
    let mut in_states = vec![None::<StateMap>; cfg.blocks.len()];
    in_states[cfg.entry] = Some(initial);
    let mut queue = VecDeque::from([cfg.entry]);
    while let Some(block_id) = queue.pop_front() {
        let mut state = in_states[block_id]
            .clone()
            .expect("queued CFG block must have an input state");
        transfer_operations(&cfg.blocks[block_id].operations, &mut state);
        for successor in cfg.blocks[block_id].terminator.successors() {
            let changed = match &mut in_states[successor] {
                Some(existing) => join_states(existing, &state),
                slot @ None => {
                    *slot = Some(state.clone());
                    true
                }
            };
            if changed {
                queue.push_back(successor);
            }
        }
    }

    let mut proven = BTreeSet::new();
    for block in &mut cfg.blocks {
        let Some(mut state) = in_states[block.id].clone() else {
            continue;
        };
        for operation in &mut block.operations {
            if let Operation::CowDetach {
                statement,
                binding,
                flat_array,
                elided,
            } = operation
                && *flat_array
                && state.get(binding) == Some(&OwnershipState::Unique)
            {
                *elided = true;
                proven.insert(*statement);
            }
            transfer_operation(operation, &mut state);
        }
    }
    proven
}

fn join_states(target: &mut StateMap, incoming: &StateMap) -> bool {
    let mut changed = false;
    for (binding, current) in target.iter_mut() {
        let other = incoming
            .get(binding)
            .copied()
            .unwrap_or(OwnershipState::Unknown);
        let joined = if *current == other {
            *current
        } else {
            OwnershipState::Unknown
        };
        if *current != joined {
            *current = joined;
            changed = true;
        }
    }
    changed
}

fn transfer_operations(operations: &[Operation], state: &mut StateMap) {
    for operation in operations {
        transfer_operation(operation, state);
    }
}

fn transfer_operation(operation: &Operation, state: &mut StateMap) {
    match operation {
        Operation::FreshArray { binding } => {
            state.insert(binding.clone(), OwnershipState::Unique);
        }
        Operation::ArrayMutation { binding } => {
            state.insert(binding.clone(), OwnershipState::Unknown);
        }
        Operation::Retain { binding } => {
            state.insert(binding.clone(), OwnershipState::Shared);
        }
        Operation::Release { binding } | Operation::Move { binding } => {
            state.insert(binding.clone(), OwnershipState::Moved);
        }
        Operation::AliasArray { target, source } => {
            state.insert(source.clone(), OwnershipState::Shared);
            state.insert(target.clone(), OwnershipState::Shared);
        }
        Operation::AssignUnknown { binding } => {
            state.insert(binding.clone(), OwnershipState::Unknown);
        }
        Operation::Call {
            effect,
            array_arguments,
        } => {
            if effect.invalidates_ownership_proofs {
                for ownership in state.values_mut() {
                    *ownership = OwnershipState::Unknown;
                }
            } else if effect.may_escape_arguments {
                for binding in array_arguments {
                    state.insert(binding.clone(), OwnershipState::Unknown);
                }
            }
        }
        Operation::Escape { bindings } => {
            for binding in bindings {
                state.insert(binding.clone(), OwnershipState::Unknown);
            }
        }
        Operation::CowDetach { binding, .. } => {
            state.insert(binding.clone(), OwnershipState::Unique);
        }
        Operation::CheckedArrayBounds { .. }
        | Operation::ArrayStore { .. }
        | Operation::Read { .. } => {}
    }
}

fn annotate_function(function: &mut Function, proven: &BTreeSet<StatementId>) {
    let mut next = 0;
    annotate_body(&mut function.body, proven, &mut next);
}

fn annotate_body(body: &mut [Statement], proven: &BTreeSet<StatementId>, next: &mut StatementId) {
    for statement in body {
        let statement_id = *next;
        *next += 1;
        if proven.contains(&statement_id) {
            match statement {
                Statement::ArrayIndexAssign { mutation_mode, .. } => {
                    *mutation_mode = ArrayMutationMode::CheckedUnique;
                }
                Statement::Assign {
                    value: ValueExpr::ArraySet { mutation_mode, .. },
                    ..
                } => {
                    *mutation_mode = ArrayMutationMode::CheckedUnique;
                }
                _ => {}
            }
        }

        match statement {
            Statement::LetIf {
                body, else_body, ..
            }
            | Statement::If {
                body, else_body, ..
            } => {
                annotate_body(body, proven, next);
                annotate_body(else_body, proven, next);
            }
            Statement::LetMatch { arms, .. } | Statement::Match { arms, .. } => {
                for arm in arms {
                    annotate_body(&mut arm.body, proven, next);
                }
            }
            Statement::LetElse { else_body, .. } => annotate_body(else_body, proven, next),
            Statement::IfLet {
                body, else_body, ..
            } => {
                annotate_body(body, proven, next);
                if let Some(else_body) = else_body {
                    annotate_body(else_body, proven, next);
                }
            }
            Statement::TaskSelect { arms } => {
                for arm in arms {
                    annotate_body(&mut arm.body, proven, next);
                }
            }
            Statement::Loop { body, .. } => annotate_body(body, proven, next),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomo_ir::{Parameter, ValueType};

    fn array_type(element: ValueType) -> ValueType {
        ValueType::Array(Box::new(element))
    }

    fn fresh_array(name: &str, element: ValueType) -> Statement {
        Statement::Let {
            name: name.to_string(),
            value_type: array_type(element.clone()),
            initializer: ValueExpr::ArrayLiteral {
                elements: vec![ValueExpr::IntLiteral(1), ValueExpr::IntLiteral(2)],
                element_type: element,
            },
        }
    }

    fn alias_array(name: &str, source: &str, element: ValueType) -> Statement {
        Statement::Let {
            name: name.to_string(),
            value_type: array_type(element),
            initializer: ValueExpr::Variable(source.to_string()),
        }
    }

    fn store(name: &str, element: ValueType) -> Statement {
        Statement::ArrayIndexAssign {
            root: name.to_string(),
            indices: vec![ValueExpr::IntLiteral(0)],
            array_types: vec![element],
            value: ValueExpr::IntLiteral(7),
            mutation_mode: ArrayMutationMode::CheckedCow,
        }
    }

    fn program(params: Vec<Parameter>, body: Vec<Statement>, is_suspend: bool) -> Program {
        Program {
            package: "test".to_string(),
            imports: Vec::new(),
            extern_functions: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            consts: Vec::new(),
            functions: vec![Function {
                package: "test".to_string(),
                name: "kernel".to_string(),
                is_suspend,
                params,
                return_type: ValueType::Void,
                body,
            }],
        }
    }

    fn mutation_mode(statement: &Statement) -> ArrayMutationMode {
        match statement {
            Statement::ArrayIndexAssign { mutation_mode, .. } => *mutation_mode,
            _ => panic!("expected indexed array store"),
        }
    }

    #[test]
    fn fresh_pod_array_store_is_proven_unique_without_moving_the_detach() {
        let mut program = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                store("values", ValueType::I32),
            ],
            false,
        );
        let report = optimize_release_program(&mut program);

        assert_eq!(report.checked_unique_stores, 1);
        assert_eq!(
            mutation_mode(&program.functions[0].body[1]),
            ArrayMutationMode::CheckedUnique
        );
        let cfg = report.functions[0].cfg.as_ref().unwrap();
        let store_block = cfg
            .blocks
            .iter()
            .find(|block| {
                block
                    .operations
                    .iter()
                    .any(|operation| matches!(operation, Operation::ArrayStore { .. }))
            })
            .unwrap();
        assert!(store_block.operations.windows(3).any(|operations| matches!(
            operations,
            [
                Operation::CheckedArrayBounds { .. },
                Operation::CowDetach { elided: true, .. },
                Operation::ArrayStore { .. }
            ]
        )));
        assert!(
            cfg.panic_edges
                .iter()
                .any(|edge| edge.from == store_block.id && edge.kind == PanicKind::Bounds)
        );
    }

    #[test]
    fn unknown_region_keeps_first_detach_and_proves_only_the_following_store() {
        let mut program = program(
            vec![Parameter {
                name: "values".to_string(),
                mutable: true,
                value_type: array_type(ValueType::I32),
            }],
            vec![
                store("values", ValueType::I32),
                store("values", ValueType::I32),
            ],
            false,
        );
        let report = optimize_release_program(&mut program);

        assert_eq!(report.checked_unique_stores, 1);
        assert_eq!(
            mutation_mode(&program.functions[0].body[0]),
            ArrayMutationMode::CheckedCow
        );
        assert_eq!(
            mutation_mode(&program.functions[0].body[1]),
            ArrayMutationMode::CheckedUnique
        );
    }

    #[test]
    fn method_set_receives_the_same_checked_unique_proof() {
        let mut program = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                Statement::Assign {
                    name: "values".to_string(),
                    value: ValueExpr::ArraySet {
                        array: "values".to_string(),
                        index: Box::new(ValueExpr::IntLiteral(0)),
                        value: Box::new(ValueExpr::IntLiteral(7)),
                        element_type: ValueType::I32,
                        mutation_mode: ArrayMutationMode::CheckedCow,
                    },
                },
            ],
            false,
        );
        let report = optimize_release_program(&mut program);

        assert_eq!(report.checked_unique_stores, 1);
        assert!(matches!(
            &program.functions[0].body[1],
            Statement::Assign {
                value: ValueExpr::ArraySet {
                    mutation_mode: ArrayMutationMode::CheckedUnique,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn unique_state_survives_a_loop_only_when_all_predecessors_are_unique() {
        let loop_store = store("values", ValueType::I32);
        let mut program = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                Statement::Loop {
                    kind: LoopKind::While(ValueExpr::BoolLiteral(true)),
                    body: vec![loop_store],
                },
            ],
            false,
        );
        let report = optimize_release_program(&mut program);

        assert_eq!(report.checked_unique_stores, 1);
        let Statement::Loop { body, .. } = &program.functions[0].body[1] else {
            panic!("expected loop");
        };
        assert_eq!(mutation_mode(&body[0]), ArrayMutationMode::CheckedUnique);
    }

    #[test]
    fn alias_on_one_branch_forces_unknown_at_the_join() {
        let mut program = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                Statement::If {
                    condition: ValueExpr::BoolLiteral(true),
                    body: vec![alias_array("snapshot", "values", ValueType::I32)],
                    else_body: Vec::new(),
                },
                store("values", ValueType::I32),
            ],
            false,
        );
        let report = optimize_release_program(&mut program);

        assert_eq!(report.checked_unique_stores, 0);
        assert_eq!(
            mutation_mode(&program.functions[0].body[2]),
            ArrayMutationMode::CheckedCow
        );
    }

    #[test]
    fn resize_call_and_reassignment_are_region_barriers() {
        let resize = Statement::Assign {
            name: "values".to_string(),
            value: ValueExpr::ArrayPush {
                array: "values".to_string(),
                value: Box::new(ValueExpr::IntLiteral(3)),
                element_type: ValueType::I32,
            },
        };
        let call = Statement::Expr(ValueExpr::Call {
            name: "publish".to_string(),
            args: Vec::new(),
        });
        let reassign = Statement::Assign {
            name: "values".to_string(),
            value: ValueExpr::Call {
                name: "replace".to_string(),
                args: Vec::new(),
            },
        };
        for barrier in [resize, call, reassign] {
            let mut program = program(
                Vec::new(),
                vec![
                    fresh_array("values", ValueType::I32),
                    barrier,
                    store("values", ValueType::I32),
                ],
                false,
            );
            let report = optimize_release_program(&mut program);
            assert_eq!(report.checked_unique_stores, 0);
            assert_eq!(
                mutation_mode(&program.functions[0].body[2]),
                ArrayMutationMode::CheckedCow
            );
        }
    }

    #[test]
    fn nested_managed_defer_and_suspend_inputs_use_the_safe_fallback() {
        let cases = [
            (
                fresh_array("values", array_type(ValueType::I32)),
                store("values", array_type(ValueType::I32)),
                false,
                None,
            ),
            (
                fresh_array("values", ValueType::String),
                store("values", ValueType::String),
                false,
                None,
            ),
            (
                fresh_array(
                    "values",
                    ValueType::Struct("Record".to_string(), Vec::new()),
                ),
                store(
                    "values",
                    ValueType::Struct("Record".to_string(), Vec::new()),
                ),
                false,
                None,
            ),
            (
                fresh_array("values", ValueType::I32),
                store("values", ValueType::I32),
                false,
                Some(Statement::Defer {
                    call: DeferredCall::Expr(ValueExpr::VoidLiteral),
                }),
            ),
            (
                fresh_array("values", ValueType::I32),
                store("values", ValueType::I32),
                true,
                None,
            ),
        ];

        for (fresh, checked_store, is_suspend, defer) in cases {
            let mut body = vec![fresh];
            if let Some(defer) = defer {
                body.push(defer);
            }
            body.push(checked_store);
            let store_index = body.len() - 1;
            let mut program = program(Vec::new(), body, is_suspend);
            let report = optimize_release_program(&mut program);
            assert_eq!(report.checked_unique_stores, 0);
            assert_eq!(
                mutation_mode(&program.functions[0].body[store_index]),
                ArrayMutationMode::CheckedCow
            );
        }
    }

    #[test]
    fn aggregation_mut_borrow_and_iteration_are_proof_barriers() {
        let aggregation = Statement::Expr(ValueExpr::StructLiteral {
            type_name: "Holder".to_string(),
            struct_args: Vec::new(),
            fields: vec![(
                "values".to_string(),
                ValueExpr::Variable("values".to_string()),
            )],
        });
        let mutable_borrow = Statement::Expr(ValueExpr::MutBorrow(vec!["values".to_string()]));
        let iteration = Statement::Loop {
            kind: LoopKind::Iterate {
                binding: "value".to_string(),
                element_type: ValueType::I32,
                iterable: ValueExpr::Variable("values".to_string()),
            },
            body: Vec::new(),
        };

        for barrier in [aggregation, mutable_borrow, iteration] {
            let mut program = program(
                Vec::new(),
                vec![
                    fresh_array("values", ValueType::I32),
                    barrier,
                    store("values", ValueType::I32),
                ],
                false,
            );
            let report = optimize_release_program(&mut program);
            assert_eq!(report.checked_unique_stores, 0);
            assert_eq!(
                mutation_mode(&program.functions[0].body[2]),
                ArrayMutationMode::CheckedCow
            );
        }
    }

    #[test]
    fn unreachable_early_return_is_not_proven_and_loop_exit_preserves_unique() {
        let mut unreachable = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                Statement::Return(None),
                store("values", ValueType::I32),
            ],
            false,
        );
        let report = optimize_release_program(&mut unreachable);
        assert_eq!(report.checked_unique_stores, 0);
        assert_eq!(
            mutation_mode(&unreachable.functions[0].body[2]),
            ArrayMutationMode::CheckedCow
        );

        let mut loop_exit = program(
            Vec::new(),
            vec![
                fresh_array("values", ValueType::I32),
                Statement::Loop {
                    kind: LoopKind::While(ValueExpr::BoolLiteral(true)),
                    body: vec![Statement::Break],
                },
                store("values", ValueType::I32),
            ],
            false,
        );
        let report = optimize_release_program(&mut loop_exit);
        assert_eq!(report.checked_unique_stores, 1);
        assert_eq!(
            mutation_mode(&loop_exit.functions[0].body[2]),
            ArrayMutationMode::CheckedUnique
        );
    }
}
