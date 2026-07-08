use super::*;

pub(super) fn validate_imports(
    path: &Path,
    imports: &[String],
    external_import_roots: &[String],
    local_import_root: Option<&str>,
) -> Result<(), Diagnostic> {
    for import in imports {
        let is_local_import = local_import_root
            .is_some_and(|root| import.split('.').next().is_some_and(|item| item == root));
        if !is_local_import && !is_supported_import(import, external_import_roots) {
            return Err(Diagnostic::new(
                "E0301",
                format!("unsupported import `{import}` in v0.1"),
                path,
                1,
                1,
                import.len().max(1),
                import,
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_standard_type_imports(
    path: &Path,
    imports: &[String],
    ast: &SourceFile,
) -> Result<(), Diagnostic> {
    for item in &ast.structs {
        for field in &item.fields {
            validate_type_ref_imports(path, imports, &field.type_ref, &synthetic_span())?;
        }
    }
    for item in &ast.enums {
        for variant in &item.variants {
            if let Some(type_ref) = &variant.payload {
                validate_type_ref_imports(path, imports, type_ref, &synthetic_span())?;
            }
        }
    }
    for item in &ast.consts {
        validate_type_ref_imports(path, imports, &item.type_ref, &item.span)?;
        validate_expr_type_imports(path, imports, &item.value, &item.span)?;
    }
    for function in ast_functions(ast) {
        for param in &function.params {
            validate_type_ref_imports(path, imports, &param.type_ref, &function.span)?;
        }
        validate_type_ref_imports(path, imports, &function.return_type, &function.span)?;
        for stmt in &function.body {
            validate_stmt_type_imports(path, imports, stmt)?;
        }
    }
    Ok(())
}

pub(super) fn validate_stmt_type_imports(
    path: &Path,
    imports: &[String],
    stmt: &Stmt,
) -> Result<(), Diagnostic> {
    match stmt {
        Stmt::Let {
            type_annotation,
            value,
            span,
            ..
        } => {
            if let Some(type_ref) = type_annotation {
                validate_type_ref_imports(path, imports, type_ref, span)?;
            }
            validate_expr_type_imports(path, imports, value, span)
        }
        Stmt::LetElse {
            value,
            else_body,
            span,
            ..
        } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for stmt in else_body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            Ok(())
        }
        Stmt::IfLet {
            value,
            body,
            else_body,
            span,
            ..
        } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for stmt in body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            if let Some(else_body) = else_body {
                for stmt in else_body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
            }
            Ok(())
        }
        Stmt::Assign { value, span, .. }
        | Stmt::Return {
            value: Some(value),
            span,
        }
        | Stmt::Expr { expr: value, span } => {
            validate_expr_type_imports(path, imports, value, span)
        }
        Stmt::Postfix { .. }
        | Stmt::Return { value: None, .. }
        | Stmt::Break { .. }
        | Stmt::Continue { .. } => Ok(()),
        Stmt::Match { value, arms, span } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for arm in arms {
                for stmt in &arm.body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
            }
            Ok(())
        }
        Stmt::For { variant, span } => match variant {
            ForVariant::Infinite { body } => {
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
            ForVariant::While { condition, body } => {
                validate_expr_type_imports(path, imports, condition, span)?;
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
            ForVariant::Iterate { iterable, body, .. } => {
                validate_expr_type_imports(path, imports, iterable, span)?;
                for stmt in body {
                    validate_stmt_type_imports(path, imports, stmt)?;
                }
                Ok(())
            }
        },
        Stmt::Defer { stmt, .. } => validate_stmt_type_imports(path, imports, stmt),
        Stmt::Unsafe { body, .. } => {
            for stmt in body {
                validate_stmt_type_imports(path, imports, stmt)?;
            }
            Ok(())
        }
    }
}

pub(super) fn validate_expr_type_imports(
    path: &Path,
    imports: &[String],
    expr: &AstExpr,
    span: &Span,
) -> Result<(), Diagnostic> {
    match expr {
        AstExpr::Call {
            type_args, args, ..
        } => {
            for type_ref in type_args {
                validate_type_ref_imports(path, imports, type_ref, span)?;
            }
            for arg in args {
                validate_expr_type_imports(path, imports, arg, span)?;
            }
            Ok(())
        }
        AstExpr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                validate_expr_type_imports(path, imports, value, span)?;
            }
            Ok(())
        }
        AstExpr::Match { value, arms } => {
            validate_expr_type_imports(path, imports, value, span)?;
            for arm in arms {
                validate_expr_type_imports(path, imports, &arm.value, span)?;
            }
            Ok(())
        }
        AstExpr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_expr_type_imports(path, imports, condition, span)?;
            validate_expr_type_imports(path, imports, then_branch, span)?;
            validate_expr_type_imports(path, imports, else_branch, span)
        }
        AstExpr::Panic { message }
        | AstExpr::Question { expr: message }
        | AstExpr::Unary { expr: message, .. } => {
            validate_expr_type_imports(path, imports, message, span)
        }
        AstExpr::Cast { expr, target } => {
            validate_expr_type_imports(path, imports, expr, span)?;
            validate_type_ref_imports(path, imports, target, span)
        }
        AstExpr::Binary { left, right, .. } => {
            validate_expr_type_imports(path, imports, left, span)?;
            validate_expr_type_imports(path, imports, right, span)
        }
        AstExpr::MutArg { .. }
        | AstExpr::Name(_)
        | AstExpr::String(_)
        | AstExpr::Int(_)
        | AstExpr::Float(_)
        | AstExpr::Char(_)
        | AstExpr::Bool(_)
        | AstExpr::Void => Ok(()),
    }
}

pub(super) fn validate_type_ref_imports(
    path: &Path,
    imports: &[String],
    type_ref: &crate::ast::TypeRef,
    span: &Span,
) -> Result<(), Diagnostic> {
    if type_ref.path == ["Array"]
        && !imports
            .iter()
            .any(|item| item == "std.array" || item == "std.array.Array")
    {
        return Err(Diagnostic::new(
            "E0301",
            "`Array` requires `import std.array.Array`",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    for arg in &type_ref.args {
        validate_type_ref_imports(path, imports, arg, span)?;
    }
    Ok(())
}

pub(super) fn is_supported_import(import: &str, external_import_roots: &[String]) -> bool {
    matches!(
        import,
        "std.io"
            | "std.io.print"
            | "std.io.println"
            | "std.io.read_line"
            | "std.io.eprint"
            | "std.io.eprintln"
            | "std.fs"
            | "std.fs.FsError"
            | "std.fs.File"
            | "std.fs.FileMetadata"
            | "std.fs.read_to_string"
            | "std.fs.write_string"
            | "std.fs.read_bytes"
            | "std.fs.write_bytes"
            | "std.fs.exists"
            | "std.fs.metadata"
            | "std.fs.create_dir"
            | "std.fs.remove_dir"
            | "std.fs.read_dir"
            | "std.fs.open"
            | "std.net"
            | "std.net.NetError"
            | "std.net.TcpListener"
            | "std.net.TcpStream"
            | "std.net.UdpDatagram"
            | "std.net.UdpSocket"
            | "std.net.connect"
            | "std.net.listen"
            | "std.net.udp_bind"
            | "std.http"
            | "std.http.HttpExchange"
            | "std.http.HttpError"
            | "std.http.HttpResponse"
            | "std.http.HttpServer"
            | "std.http.accept"
            | "std.http.close_exchange"
            | "std.http.close_server"
            | "std.http.get"
            | "std.http.listen"
            | "std.http.post"
            | "std.http.respond_string"
            | "std.env"
            | "std.env.args"
            | "std.env.cwd"
            | "std.env.get"
            | "std.env.home_dir"
            | "std.env.set"
            | "std.env.temp_dir"
            | "std.result"
            | "std.result.Result"
            | "std.result.is_ok"
            | "std.result.is_err"
            | "std.result.unwrap_or"
            | "std.result.map"
            | "std.result.map_err"
            | "std.result.and_then"
            | "std.option"
            | "std.option.Option"
            | "std.option.is_some"
            | "std.option.is_none"
            | "std.option.unwrap_or"
            | "std.option.map"
            | "std.option.and_then"
            | "std.array"
            | "std.array.Array"
            | "std.array.new"
            | "std.array.len"
            | "std.array.push"
            | "std.array.get"
            | "std.array.set"
            | "std.array.pop"
            | "std.array.insert"
            | "std.array.remove"
            | "std.array.clear"
            | "std.array.iter"
            | "std.string"
            | "std.string.len"
            | "std.string.concat"
            | "std.string.is_empty"
            | "std.string.contains"
            | "std.string.starts_with"
            | "std.string.ends_with"
            | "std.string.split"
            | "std.string.trim"
            | "std.string.to_lower"
            | "std.string.to_upper"
            | "std.char"
            | "std.char.is_digit"
            | "std.char.is_alpha"
            | "std.char.is_whitespace"
            | "std.char.to_string"
            | "std.debug"
            | "std.debug.print"
            | "std.debug.println"
            | "std.debug.panic"
            | "std.debug.backtrace"
            | "std.log"
            | "std.log.debug"
            | "std.log.info"
            | "std.log.warn"
            | "std.log.error"
            | "std.log.enabled"
            | "std.hash"
            | "std.hash.HashState"
            | "std.hash.bytes"
            | "std.hash.new"
            | "std.hash.string"
            | "std.hash.write_bytes"
            | "std.hash.write_string"
            | "std.hash.finish"
            | "std.crypto"
            | "std.crypto.sha256"
            | "std.crypto.sha512"
            | "std.crypto.random_bytes"
            | "std.json"
            | "std.json.JsonValue"
            | "std.json.JsonError"
            | "std.json.parse"
            | "std.json.stringify"
            | "std.regex"
            | "std.regex.Regex"
            | "std.regex.RegexError"
            | "std.regex.compile"
            | "std.regex.is_match"
            | "std.regex.captures"
            | "std.collections"
            | "std.collections.StringMap"
            | "std.collections.StringSet"
            | "std.collections.map_new"
            | "std.collections.map_len"
            | "std.collections.map_get"
            | "std.collections.map_contains"
            | "std.collections.map_set"
            | "std.collections.map_remove"
            | "std.collections.set_new"
            | "std.collections.set_len"
            | "std.collections.set_contains"
            | "std.collections.set_insert"
            | "std.collections.set_remove"
            | "std.os"
            | "std.os.platform"
            | "std.os.arch"
            | "std.os.path_separator"
            | "std.os.line_ending"
            | "std.time"
            | "std.time.Duration"
            | "std.time.duration_millis"
            | "std.time.duration_seconds"
            | "std.time.duration_as_millis"
            | "std.time.format_duration"
            | "std.time.sleep"
            | "std.time.now_millis"
            | "std.time.monotonic_millis"
            | "std.time.sleep_millis"
            | "std.testing"
            | "std.testing.assert"
            | "std.testing.assert_equal"
            | "std.testing.assert_error"
            | "std.process"
            | "std.process.ProcessError"
            | "std.process.ProcessOutput"
            | "std.process.exit"
            | "std.process.spawn"
            | "std.process.status"
            | "std.process.exec"
            | "std.process.output"
            | "std.num"
            | "std.num.NumError"
            | "std.num.parse_i64"
            | "std.num.parse_u64"
            | "std.num.parse_f64"
            | "std.num.checked_add"
            | "std.num.checked_sub"
            | "std.num.checked_mul"
            | "std.num.wrapping_add"
            | "std.num.wrapping_sub"
            | "std.num.wrapping_mul"
            | "std.path"
            | "std.path.join"
            | "std.path.basename"
            | "std.path.dirname"
            | "std.path.extension"
            | "std.path.normalize"
            | "std.path.is_absolute"
            | "std.math"
            | "std.math.abs"
            | "std.math.min"
            | "std.math.max"
            | "std.math.floor"
            | "std.math.ceil"
            | "std.math.round"
            | "std.math.sqrt"
            | "std.math.pow"
            | "std.math.sin"
            | "std.math.cos"
    ) || is_supported_external_import(import, external_import_roots)
}

pub(super) fn is_supported_external_import(import: &str, external_import_roots: &[String]) -> bool {
    let Some((root, _rest)) = import.split_once('.') else {
        return false;
    };
    root != "std" && external_import_roots.iter().any(|alias| alias == root)
}

pub(super) fn unsupported_type_diagnostic(
    path: &Path,
    span: &Span,
    type_ref: &crate::ast::TypeRef,
    message: impl Into<String>,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
) -> Diagnostic {
    if type_ref.path == ["int"] {
        return Diagnostic::new(
            "E0403",
            "`int` is not a v0.1 builtin type; use `i64` or an explicit-width integer type (`i32`, `u32`, `u64`)",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        );
    }

    if let Some(import) = missing_standard_type_import(type_ref, struct_names, enum_names) {
        let type_name = type_ref.path.first().expect("type ref must have a root");
        return Diagnostic::new(
            "E0301",
            format!("`{type_name}` requires `import {import}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        );
    }
    Diagnostic::new(
        "E0403",
        message,
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}

pub(super) fn unsupported_type_diagnostic_from_maps(
    path: &Path,
    span: &Span,
    type_ref: &crate::ast::TypeRef,
    message: impl Into<String>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Diagnostic {
    let struct_names = structs
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    unsupported_type_diagnostic(path, span, type_ref, message, &struct_names, &enum_names)
}

pub(super) fn missing_standard_type_import(
    type_ref: &crate::ast::TypeRef,
    struct_names: &[(String, usize)],
    enum_names: &[(String, usize)],
) -> Option<&'static str> {
    let root = type_ref.path.first()?;
    if struct_names.iter().any(|(name, _)| name == root)
        || enum_names.iter().any(|(name, _)| name == root)
    {
        return None;
    }
    match root.as_str() {
        "Result" => Some("std.result"),
        "Option" => Some("std.option"),
        "Array" => Some("std.array"),
        "FsError" | "File" | "FileMetadata" => Some("std.fs"),
        "IoError" => Some("std.io"),
        "NumError" => Some("std.num"),
        "HashState" => Some("std.hash"),
        "JsonValue" | "JsonError" => Some("std.json"),
        "Regex" | "RegexError" => Some("std.regex"),
        "StringMap" | "StringSet" => Some("std.collections"),
        "Duration" => Some("std.time"),
        _ => None,
    }
}

pub(super) fn validate_type_namespace(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let struct_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    for enum_type in enums {
        if struct_names.contains(enum_type.name.as_str()) {
            return Err(Diagnostic::new(
                "E0312",
                format!("type `{}` is already defined", enum_type.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_no_recursive_value_types(
    path: &Path,
    structs: &[StructType],
    enums: &[EnumType],
) -> Result<(), Diagnostic> {
    let mut graph = HashMap::<String, Vec<String>>::new();
    let nominal_names = structs
        .iter()
        .map(|item| item.name.as_str())
        .chain(enums.iter().map(|item| item.name.as_str()))
        .collect::<HashSet<_>>();

    for struct_type in structs {
        let mut deps = Vec::new();
        for field in &struct_type.fields {
            collect_value_type_dependencies(&field.value_type, &nominal_names, &mut deps);
        }
        graph.insert(struct_type.name.clone(), deps);
    }
    for enum_type in enums {
        let mut deps = Vec::new();
        for variant in &enum_type.variants {
            if let Some(payload) = &variant.payload {
                collect_value_type_dependencies(payload, &nominal_names, &mut deps);
            }
        }
        graph.insert(enum_type.name.clone(), deps);
    }

    for name in graph.keys() {
        let mut visiting = Vec::new();
        let mut visited = HashSet::new();
        if type_dependency_reaches(name, name, &graph, &mut visiting, &mut visited) {
            return Err(Diagnostic::new(
                "E0410",
                format!("type `{name}` is recursively embedded by value"),
                path,
                1,
                1,
                1,
                "",
            ));
        }
    }
    Ok(())
}

pub(super) fn collect_value_type_dependencies(
    value_type: &ValueType,
    nominal_names: &HashSet<&str>,
    out: &mut Vec<String>,
) {
    match value_type {
        ValueType::Struct(name, args) | ValueType::Enum(name, args) => {
            if nominal_names.contains(name.as_str()) {
                out.push(name.clone());
            }
            for arg in args {
                collect_value_type_dependencies(arg, nominal_names, out);
            }
        }
        ValueType::Array(_) => {}
        ValueType::String
        | ValueType::Int
        | ValueType::I32
        | ValueType::U32
        | ValueType::U64
        | ValueType::Float
        | ValueType::Char
        | ValueType::Bool
        | ValueType::TypeParam(_)
        | ValueType::Void
        | ValueType::Never => {}
    }
}

pub(super) fn type_dependency_reaches(
    start: &str,
    current: &str,
    graph: &HashMap<String, Vec<String>>,
    visiting: &mut Vec<String>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(current.to_string()) {
        return false;
    }
    visiting.push(current.to_string());
    for dep in graph.get(current).into_iter().flatten() {
        if dep == start {
            return true;
        }
        if !visiting.iter().any(|item| item == dep)
            && type_dependency_reaches(start, dep, graph, visiting, visited)
        {
            return true;
        }
    }
    visiting.pop();
    false
}

pub(super) fn validate_standard_type_conflicts(
    path: &Path,
    needs: StandardTypeNeeds,
    structs: &[AstStructDef],
    enums: &[AstEnumDef],
) -> Result<(), Diagnostic> {
    if needs.io {
        reject_user_std_struct(path, structs, "IoError")?;
    }
    if needs.fs {
        reject_user_std_struct(path, structs, "FsError")?;
        reject_user_std_struct(path, structs, "File")?;
    }
    if needs.net {
        reject_user_std_struct(path, structs, "NetError")?;
        reject_user_std_struct(path, structs, "TcpListener")?;
        reject_user_std_struct(path, structs, "TcpStream")?;
        reject_user_std_struct(path, structs, "UdpDatagram")?;
        reject_user_std_struct(path, structs, "UdpSocket")?;
    }
    if needs.http {
        reject_user_std_struct(path, structs, "HttpExchange")?;
        reject_user_std_struct(path, structs, "HttpError")?;
        reject_user_std_struct(path, structs, "HttpResponse")?;
        reject_user_std_struct(path, structs, "HttpServer")?;
    }
    if needs.num {
        reject_user_std_struct(path, structs, "NumError")?;
    }
    if needs.process {
        reject_user_std_struct(path, structs, "ProcessError")?;
        reject_user_std_struct(path, structs, "ProcessOutput")?;
    }
    if needs.hash {
        reject_user_std_struct(path, structs, "HashState")?;
    }
    if needs.io || needs.fs || needs.net || needs.http || needs.num || needs.process || needs.result
    {
        reject_user_std_enum(path, enums, "Result")?;
    }
    if needs.env || needs.num || needs.option || needs.array {
        reject_user_std_enum(path, enums, "Option")?;
    }
    Ok(())
}

pub(super) fn reject_user_std_struct(
    path: &Path,
    structs: &[AstStructDef],
    name: &str,
) -> Result<(), Diagnostic> {
    if structs.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}

pub(super) fn reject_user_std_enum(
    path: &Path,
    enums: &[AstEnumDef],
    name: &str,
) -> Result<(), Diagnostic> {
    if enums.iter().any(|item| item.name == name) {
        return Err(Diagnostic::new(
            "E0312",
            format!("type `{name}` conflicts with a required standard library type"),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}
