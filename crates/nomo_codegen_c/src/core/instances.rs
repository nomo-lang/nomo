use super::*;

pub(super) fn collect_result_map_err_instances(program: &Program) -> Vec<ResultMapErrInstance> {
    let mut out = Vec::new();
    for function in &program.functions {
        for statement in &function.body {
            collect_stmt_result_map_err(statement, &mut out);
        }
    }
    out
}

pub(super) fn collect_result_unwrap_or_instances(program: &Program) -> Vec<ResultUnwrapOrInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultUnwrapOr {
            ok_type, err_type, ..
        } = expr
        {
            let instance = ResultUnwrapOrInstance {
                ok_type: ok_type.clone(),
                err_type: err_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_result_map_instances(program: &Program) -> Vec<ResultMapInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultMap {
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
            ..
        } = expr
        {
            let instance = ResultMapInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_result_and_then_instances(program: &Program) -> Vec<ResultAndThenInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::ResultAndThen {
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
            ..
        } = expr
        {
            let instance = ResultAndThenInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_option_unwrap_or_instances(program: &Program) -> Vec<OptionUnwrapOrInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionUnwrapOr { payload_type, .. } = expr {
            let instance = OptionUnwrapOrInstance {
                payload_type: payload_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_option_map_instances(program: &Program) -> Vec<OptionMapInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionMap {
            source_type,
            target_type,
            converter,
            ..
        } = expr
        {
            let instance = OptionMapInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_option_and_then_instances(program: &Program) -> Vec<OptionAndThenInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::OptionAndThen {
            source_type,
            target_type,
            converter,
            ..
        } = expr
        {
            let instance = OptionAndThenInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

pub(super) fn collect_num_checked_binary_instances(
    program: &Program,
) -> Vec<NumCheckedBinaryInstance> {
    let mut out = Vec::new();
    walk_program_exprs(program, &mut |expr| {
        if let ValueExpr::NumBinary {
            function: NumBinaryFunction::Checked,
            op,
            value_type,
            ..
        } = expr
        {
            let instance = NumCheckedBinaryInstance {
                op: *op,
                value_type: value_type.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
    });
    out
}

fn collect_stmt_result_map_err(statement: &Statement, out: &mut Vec<ResultMapErrInstance>) {
    match statement {
        Statement::Let { initializer, .. }
        | Statement::QuestionLet {
            result_expr: initializer,
            ..
        }
        | Statement::QuestionReturn {
            result_expr: initializer,
            ..
        }
        | Statement::Assign {
            value: initializer, ..
        }
        | Statement::AssignField {
            value: initializer, ..
        }
        | Statement::Println(initializer)
        | Statement::Print(initializer)
        | Statement::Eprintln(initializer)
        | Statement::Eprint(initializer)
        | Statement::Panic(initializer)
        | Statement::Return(Some(initializer))
        | Statement::Expr(initializer) => collect_expr_result_map_err(initializer, out),
        Statement::LetElse {
            value, else_body, ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(value, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            if let Some(else_body) = else_body {
                for statement in else_body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            collect_expr_result_map_err(condition, out);
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
            for statement in else_body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::LetMatch { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => collect_expr_result_map_err(condition, out),
                LoopKind::CStyle {
                    initializer,
                    condition,
                    update,
                    ..
                } => {
                    collect_expr_result_map_err(initializer, out);
                    collect_expr_result_map_err(condition, out);
                    collect_expr_result_map_err(update, out);
                }
                LoopKind::Iterate { iterable, .. } => collect_expr_result_map_err(iterable, out),
            }
            for statement in body {
                collect_stmt_result_map_err(statement, out);
            }
        }
        Statement::Match { value, arms, .. } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                for statement in &arm.body {
                    collect_stmt_result_map_err(statement, out);
                }
            }
        }
        Statement::Defer { call } => collect_deferred_result_map_err(call, out),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn collect_deferred_result_map_err(call: &DeferredCall, out: &mut Vec<ResultMapErrInstance>) {
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            collect_expr_result_map_err(expr, out);
        }
    }
}

fn collect_expr_result_map_err(expr: &ValueExpr, out: &mut Vec<ResultMapErrInstance>) {
    match expr {
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            collect_expr_result_map_err(result, out);
            let instance = ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            };
            if !out.contains(&instance) {
                out.push(instance);
            }
        }
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
        } => {
            collect_expr_result_map_err(left, out);
            collect_expr_result_map_err(right, out);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            collect_expr_result_map_err(socket, out);
            collect_expr_result_map_err(content, out);
            collect_expr_result_map_err(host, out);
            collect_expr_result_map_err(port, out);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
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
        | ValueExpr::ArrayLen { array: path } => collect_expr_result_map_err(path, out),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => {
            collect_expr_result_map_err(result, out);
            collect_expr_result_map_err(default, out);
        }
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => {
            collect_expr_result_map_err(option, out);
            collect_expr_result_map_err(default, out);
        }
        ValueExpr::FsWriteString { path, content } => {
            collect_expr_result_map_err(path, out);
            collect_expr_result_map_err(content, out);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            collect_expr_result_map_err(path, out);
            collect_expr_result_map_err(bytes, out);
        }
        ValueExpr::FileWriteString { file, content } => {
            collect_expr_result_map_err(file, out);
            collect_expr_result_map_err(content, out);
        }
        ValueExpr::EnvSet { name, value } => {
            collect_expr_result_map_err(name, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::HashWriteString { state, value } => {
            collect_expr_result_map_err(state, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::HashWriteBytes { state, value } => {
            collect_expr_result_map_err(state, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            collect_expr_result_map_err(map, out);
            collect_expr_result_map_err(key, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                collect_expr_result_map_err(arg, out);
            }
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            collect_expr_result_map_err(array, out);
            collect_expr_result_map_err(index, out);
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayRemove { index, .. } => {
            collect_expr_result_map_err(index, out);
        }
        ValueExpr::ArrayPush { value, .. } => collect_expr_result_map_err(value, out),
        ValueExpr::ArraySet { index, value, .. } => {
            collect_expr_result_map_err(index, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            collect_expr_result_map_err(index, out);
            collect_expr_result_map_err(value, out);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_result_map_err(value, out);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                collect_expr_result_map_err(payload, out);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_result_map_err(condition, out);
            collect_expr_result_map_err(then_branch, out);
            collect_expr_result_map_err(else_branch, out);
        }
        ValueExpr::Panic { message, .. } => collect_expr_result_map_err(message, out),
        ValueExpr::Match { value, arms } => {
            collect_expr_result_map_err(value, out);
            for arm in arms {
                collect_expr_result_map_err(&arm.value, out);
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
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}

fn walk_program_exprs<F>(program: &Program, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    for function in &program.functions {
        for statement in &function.body {
            walk_stmt_exprs(statement, visit);
        }
    }
}

fn walk_stmt_exprs<F>(statement: &Statement, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    match statement {
        Statement::Let { initializer, .. }
        | Statement::QuestionLet {
            result_expr: initializer,
            ..
        }
        | Statement::QuestionReturn {
            result_expr: initializer,
            ..
        }
        | Statement::Assign {
            value: initializer, ..
        }
        | Statement::AssignField {
            value: initializer, ..
        }
        | Statement::Println(initializer)
        | Statement::Print(initializer)
        | Statement::Eprintln(initializer)
        | Statement::Eprint(initializer)
        | Statement::Panic(initializer)
        | Statement::Return(Some(initializer))
        | Statement::Expr(initializer) => walk_expr(initializer, visit),
        Statement::LetElse {
            value, else_body, ..
        } => {
            walk_expr(value, visit);
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::IfLet {
            value,
            body,
            else_body,
            ..
        } => {
            walk_expr(value, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            if let Some(else_body) = else_body {
                for statement in else_body {
                    walk_stmt_exprs(statement, visit);
                }
            }
        }
        Statement::If {
            condition,
            body,
            else_body,
        } => {
            walk_expr(condition, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::LetIf {
            condition,
            body,
            else_body,
            ..
        } => {
            walk_expr(condition, visit);
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
            for statement in else_body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::LetMatch { value, arms, .. } | Statement::Match { value, arms, .. } => {
            walk_expr(value, visit);
            for arm in arms {
                for statement in &arm.body {
                    walk_stmt_exprs(statement, visit);
                }
            }
        }
        Statement::Loop { kind, body } => {
            match kind {
                LoopKind::Infinite => {}
                LoopKind::While(condition) => walk_expr(condition, visit),
                LoopKind::CStyle {
                    initializer,
                    condition,
                    update,
                    ..
                } => {
                    walk_expr(initializer, visit);
                    walk_expr(condition, visit);
                    walk_expr(update, visit);
                }
                LoopKind::Iterate { iterable, .. } => walk_expr(iterable, visit),
            }
            for statement in body {
                walk_stmt_exprs(statement, visit);
            }
        }
        Statement::Defer { call } => walk_deferred_exprs(call, visit),
        Statement::Return(None) | Statement::Break | Statement::Continue => {}
    }
}

fn walk_deferred_exprs<F>(call: &DeferredCall, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    match call {
        DeferredCall::Expr(expr)
        | DeferredCall::Println(expr)
        | DeferredCall::Print(expr)
        | DeferredCall::Eprintln(expr)
        | DeferredCall::Eprint(expr) => {
            walk_expr(expr, visit);
        }
    }
}

fn walk_expr<F>(expr: &ValueExpr, visit: &mut F)
where
    F: FnMut(&ValueExpr),
{
    visit(expr);
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
        } => {
            walk_expr(left, visit);
            walk_expr(right, visit);
        }
        ValueExpr::UdpSocketSendToString {
            socket,
            content,
            host,
            port,
        } => {
            walk_expr(socket, visit);
            walk_expr(content, visit);
            walk_expr(host, visit);
            walk_expr(port, visit);
        }
        ValueExpr::FsReadToString { path }
        | ValueExpr::FsReadBytes { path }
        | ValueExpr::FsExists { path }
        | ValueExpr::FsMetadata { path }
        | ValueExpr::FsCreateDir { path }
        | ValueExpr::FsRemoveDir { path }
        | ValueExpr::FsReadDir { path }
        | ValueExpr::FsOpen { path }
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
        | ValueExpr::ArrayLen { array: path } => walk_expr(path, visit),
        ValueExpr::ResultMapErr { result, .. } => walk_expr(result, visit),
        ValueExpr::ResultUnwrapOr {
            result, default, ..
        } => {
            walk_expr(result, visit);
            walk_expr(default, visit);
        }
        ValueExpr::OptionUnwrapOr {
            option, default, ..
        } => {
            walk_expr(option, visit);
            walk_expr(default, visit);
        }
        ValueExpr::FsWriteString { path, content } => {
            walk_expr(path, visit);
            walk_expr(content, visit);
        }
        ValueExpr::FsWriteBytes { path, bytes } => {
            walk_expr(path, visit);
            walk_expr(bytes, visit);
        }
        ValueExpr::FileWriteString { file, content } => {
            walk_expr(file, visit);
            walk_expr(content, visit);
        }
        ValueExpr::EnvSet { name, value } => {
            walk_expr(name, visit);
            walk_expr(value, visit);
        }
        ValueExpr::HashWriteString { state, value } => {
            walk_expr(state, visit);
            walk_expr(value, visit);
        }
        ValueExpr::HashWriteBytes { state, value } => {
            walk_expr(state, visit);
            walk_expr(value, visit);
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            walk_expr(map, visit);
            walk_expr(key, visit);
            walk_expr(value, visit);
        }
        ValueExpr::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        ValueExpr::ArrayGet { array, index, .. } => {
            walk_expr(array, visit);
            walk_expr(index, visit);
        }
        ValueExpr::ArrayPop { .. } | ValueExpr::ArrayClear { .. } => {}
        ValueExpr::ArrayRemove { index, .. } => walk_expr(index, visit),
        ValueExpr::ArrayPush { value, .. } => walk_expr(value, visit),
        ValueExpr::ArraySet { index, value, .. } => {
            walk_expr(index, visit);
            walk_expr(value, visit);
        }
        ValueExpr::ArrayInsert { index, value, .. } => {
            walk_expr(index, visit);
            walk_expr(value, visit);
        }
        ValueExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                walk_expr(value, visit);
            }
        }
        ValueExpr::EnumVariant { payload, .. } => {
            if let Some(payload) = payload {
                walk_expr(payload, visit);
            }
        }
        ValueExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk_expr(condition, visit);
            walk_expr(then_branch, visit);
            walk_expr(else_branch, visit);
        }
        ValueExpr::Panic { message, .. } => walk_expr(message, visit),
        ValueExpr::Match { value, arms } => {
            walk_expr(value, visit);
            for arm in arms {
                walk_expr(&arm.value, visit);
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
        | ValueExpr::EnvCwd
        | ValueExpr::EnvHomeDir
        | ValueExpr::EnvTempDir
        | ValueExpr::OsPlatform
        | ValueExpr::OsArch
        | ValueExpr::OsPathSeparator
        | ValueExpr::OsLineEnding
        | ValueExpr::TimeNowMillis
        | ValueExpr::TimeMonotonicMillis
        | ValueExpr::EnvArgs
        | ValueExpr::IoReadLine
        | ValueExpr::ArrayNew { .. }
        | ValueExpr::FieldAccess { .. } => {}
    }
}
