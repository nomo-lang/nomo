use super::*;
use crate::lexer::lex;

#[test]
fn parses_dot_chain_line_continuation() {
    let source = "package app\n    .main\n\nimport std\n    .io\n\nfn main() -> void {\n    let count: u64 = message\n        .len()\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.package, ["app", "main"]);
    assert_eq!(ast.imports[0], ["std", "io"]);
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            value: Expr::Call { ref callee, .. },
            ..
        } if callee == &vec!["message".to_string(), "len".to_string()]
    ));
}

#[test]
fn parses_keyword_as_dot_path_segment() {
    let source = "package app.main\n\nimport std.debug.panic\n\nfn main() -> void {\n    debug.panic(\"boom\")\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.imports[0], ["std", "debug", "panic"]);
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Expr {
            expr: Expr::Call { ref callee, .. },
            ..
        } if callee == &vec!["debug".to_string(), "panic".to_string()]
    ));
}

#[test]
fn parses_repeated_line_start_dot_continuations_on_named_values() {
    let source = "package app.main\n\nfn make() -> Result<string, string> {\n    let prefix: string = \"newline\"\n    let with_dot: string = prefix\n        .concat(\" dot\")\n    return Result\n        .Ok(with_dot\n            .concat(\" ok\"))\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Let {
            value: Expr::Call { ref callee, .. },
            ..
        } if callee == &vec!["prefix".to_string(), "concat".to_string()]
    ));
    assert!(matches!(
        ast.functions[0].body[2],
        Stmt::Return {
            value: Some(Expr::Call {
                ref callee,
                ref args,
                ..
            }),
            ..
        } if callee == &vec!["Result".to_string(), "Ok".to_string()]
            && matches!(
                args.as_slice(),
                [Expr::Call { callee: arg_callee, .. }]
                    if arg_callee == &vec!["with_dot".to_string(), "concat".to_string()]
            )
    ));
}

#[test]
fn parses_operator_call_and_type_arg_line_continuations() {
    let source = "package app.main\n\nstruct Box<T> {\n    value: T\n}\n\nfn add(left: i32, right: i32) -> i32 {\n    return left +\n        right\n}\n\nfn main() -> void {\n    let total: i32 = add(\n        1,\n        2\n    )\n    let ratio: f64 = total as\n        f64\n    let boxed: Box<i32> = Box.new<\n        i32\n    >(\n        total\n    )\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Binary {
                op: BinaryOp::Add,
                ..
            }),
            ..
        }
    ));
    assert!(matches!(
        ast.functions[1].body[0],
        Stmt::Let {
            value: Expr::Call { ref args, .. },
            ..
        } if args.len() == 2
    ));
    assert!(matches!(
        ast.functions[1].body[1],
        Stmt::Let {
            value: Expr::Cast { ref target, .. },
            ..
        } if target.path == ["f64"]
    ));
    assert!(matches!(
        ast.functions[1].body[2],
        Stmt::Let {
            ref type_annotation,
            value: Expr::Call { ref type_args, .. },
            ..
        } if type_annotation.as_ref().unwrap().args.len() == 1 && type_args.len() == 1
    ));
}

#[test]
fn parses_match_arrow_line_continuation() {
    let source = "package app.main\n\nenum Option<T> {\n    Some(T)\n    None\n}\n\nfn label(value: Option<i32>) -> string {\n    return match value {\n        Option.Some(n) =>\n            \"some\"\n        Option.None =>\n            \"none\"\n    }\n}\n\nfn print(value: Option<i32>) -> void {\n    match value {\n        Option.Some(n) =>\n            {\n                println(\"some\")\n            }\n        Option.None =>\n            {\n                println(\"none\")\n            }\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Match { ref arms, .. }),
            ..
        } if arms.len() == 2
    ));
    assert!(matches!(
        ast.functions[1].body[0],
        Stmt::Match { ref arms, .. } if arms.len() == 2
    ));
}

#[test]
fn rejects_multiple_newline_separated_items_on_one_line() {
    for (source, message) in [
        (
            "package app.main\n\nstruct User {\n    id: string email: string\n}\n\nfn main() -> void {\n}\n",
            "expected newline after struct field",
        ),
        (
            "package app.main\n\nenum Color {\n    Red Blue\n}\n\nfn main() -> void {\n}\n",
            "expected newline after enum variant",
        ),
        (
            "package app.main\n\nfn main() -> void {\n    let left: i32 = 1 let right: i32 = 2\n}\n",
            "expected newline after statement",
        ),
        (
            "package app.main\n\nenum Color {\n    Red\n    Blue\n}\n\nfn label(color: Color) -> string {\n    return match color {\n        Color.Red => \"red\" Color.Blue => \"blue\"\n    }\n}\n\nfn main() -> void {\n}\n",
            "expected newline after match arm",
        ),
    ] {
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let err = parse(Path::new("main.nomo"), &tokens).unwrap_err();

        assert_eq!(err.code, "E0211");
        assert!(err.message.contains(message), "{:?}", err.message);
    }
}
