use super::*;

pub(super) fn require_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    module_import: &str,
    symbol: &str,
) -> Result<(), Diagnostic> {
    let imported = if symbol == "Array.new" {
        imports.iter().any(|item| {
            matches!(
                item.as_str(),
                "std.array" | "std.array.Array" | "std.array.new"
            )
        })
    } else {
        imports.iter().any(|item| item == module_import)
    };
    if imported {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`{symbol}` requires `import {module_import}`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn require_string_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.string" || item == &format!("std.string.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`string.{method}` requires `import std.string`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn require_array_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports.iter().any(|item| {
        item == "std.array" || item == "std.array.Array" || item == &format!("std.array.{method}")
    }) {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Array.{method}` requires `import std.array`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn require_result_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.result" || item == &format!("std.result.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Result.{method}` requires `import std.result`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn require_option_method_import(
    path: &Path,
    imports: &[String],
    span: &Span,
    method: &str,
) -> Result<(), Diagnostic> {
    if imports
        .iter()
        .any(|item| item == "std.option" || item == &format!("std.option.{method}"))
    {
        return Ok(());
    }
    Err(Diagnostic::new(
        "E0301",
        format!("`Option.{method}` requires `import std.option`"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    ))
}

pub(super) fn resolve_io_print_function<'a>(
    callee: &'a [String],
    imports: &[String],
) -> Option<&'a str> {
    match callee {
        [module, name]
            if module == "io"
                && matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
                && imports.iter().any(|item| item == "std.io") =>
        {
            Some(name.as_str())
        }
        [name]
            if matches!(name.as_str(), "print" | "println" | "eprint" | "eprintln")
                && imports.iter().any(|item| item == &format!("std.io.{name}")) =>
        {
            Some(name.as_str())
        }
        _ => None,
    }
}

pub(super) fn io_print_import_error(callee: &[String]) -> String {
    match callee {
        [module, name] if module == "io" => {
            format!("v0.1 current implementation requires `import std.io` for `io.{name}`")
        }
        [name] => {
            format!("v0.1 current implementation requires `import std.io.{name}` for `{name}`")
        }
        _ => "v0.1 current implementation requires an io import".to_string(),
    }
}

pub(super) fn missing_io_import_diagnostic(
    path: &Path,
    span: &Span,
    callee: &[String],
) -> Diagnostic {
    let import = match callee {
        [module, _] if module == "io" => "import std.io\n".to_string(),
        [name] => format!("import std.io.{name}\n"),
        _ => "import std.io\n".to_string(),
    };
    let description = match callee {
        [module, name] if module == "io" => {
            format!("add `import std.io` to use `io.{name}`")
        }
        [name] => format!("add `import std.io.{name}` to use `{name}`"),
        _ => "add `import std.io` to use io functions".to_string(),
    };
    let mut diagnostic = Diagnostic::new(
        "E0301",
        io_print_import_error(callee),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    );
    diagnostic.suggestions.push(Suggestion {
        line: 2,
        column: 1,
        length: 0,
        text: import,
        description,
    });
    diagnostic
}

pub(super) fn println_type_error(path: &Path, span: &Span, function_name: &str) -> Diagnostic {
    Diagnostic::new(
        "E0402",
        format!("`io.{function_name}` accepts string, numeric, character, or boolean arguments"),
        path,
        span.line,
        span.column,
        span.length,
        &span.text,
    )
}
