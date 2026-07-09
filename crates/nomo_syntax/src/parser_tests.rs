use super::*;
use crate::ast::UnaryOp;
use crate::lexer::lex;

#[test]
fn parses_v0_1_ast() {
    let source =
        "package app.main\n\nimport std.io\n\nfn main() -> void {\n    io.println(\"Hello\")\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.package, vec!["app", "main"]);
    assert_eq!(ast.imports, vec![vec!["std", "io"]]);
    assert!(ast.structs.is_empty());
    assert!(ast.enums.is_empty());
    assert_eq!(ast.functions.len(), 1);
    assert!(ast.functions[0].params.is_empty());
}

#[test]
fn rejects_wildcard_imports_in_v0_1() {
    let source = "package app.main\n\nimport std.io.*\n\nfn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let err = parse(Path::new("main.nomo"), &tokens).unwrap_err();

    assert_eq!(err.code, "E0274");
    assert!(err.message.contains("wildcard imports"));
    assert!(err.message.contains("v0.1"));
}

#[test]
fn parses_let_and_variable_reference() {
    let source = "package app.main\n\nimport std.io\n\nfn main() -> void {\n    let message: string = \"Hello\"\n    io.println(message)\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            ref name,
            ref type_annotation,
            value: Expr::String(_),
            ..
        } if name == "message"
            && type_annotation.as_ref().unwrap().path == ["string"]
    ));
    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Expr {
            expr: Expr::Call { ref args, .. },
            ..
        } if args == &[Expr::Name(vec!["message".to_string()])]
    ));
}

#[test]
fn parses_function_params_return_and_addition() {
    let source = "package app.main\n\nfn add(a: i64, b: i64) -> i64 {\n    return a + b\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.functions[0].params.len(), 2);
    assert_eq!(ast.functions[0].params[0].name, "a");
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
}

#[test]
fn parses_binary_arithmetic_precedence() {
    let source = "package app.main\n\nfn calc(a: i64, b: i64, c: i64, d: i64, e: i64) -> i64 {\n    return a - b * c / d % e\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Binary {
                op: BinaryOp::Subtract,
                ref right,
                ..
            }),
            ..
        } if matches!(right.as_ref(), Expr::Binary {
            op: BinaryOp::Remainder,
            ..
        })
    ));
}

#[test]
fn parses_logical_operator_precedence() {
    let source = "package app.main\n\nfn check(a: bool, b: bool, c: bool) -> bool {\n    return !a && b || c\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Binary {
                op: BinaryOp::LogicalOr,
                ref left,
                ..
            }),
            ..
        } if matches!(left.as_ref(), Expr::Binary {
            op: BinaryOp::LogicalAnd,
            left,
            ..
        } if matches!(left.as_ref(), Expr::Unary { .. }))
    ));
}

#[test]
fn parses_unary_negation_and_parenthesized_expressions() {
    let source = "package app.main\n\nfn calc(a: i64, b: i64, c: i64) -> i64 {\n    return -(a + b) * -c\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Binary {
                op: BinaryOp::Multiply,
                ref left,
                ref right,
            }),
            ..
        } if matches!(left.as_ref(), Expr::Unary {
            op: UnaryOp::Negate,
            expr,
        } if matches!(expr.as_ref(), Expr::Binary { op: BinaryOp::Add, .. }))
            && matches!(right.as_ref(), Expr::Unary {
                op: UnaryOp::Negate,
                expr,
            } if matches!(expr.as_ref(), Expr::Name(name) if name == &vec!["c".to_string()]))
    ));
}

#[test]
fn parses_bitwise_operator_precedence() {
    let source = "package app.main\n\nfn mask(a: i64, b: i64, c: i64, d: i64) -> i64 {\n    return a | b ^ c & d << 1\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Binary {
                op: BinaryOp::BitXor,
                ref left,
                ref right,
                ..
            }),
            ..
        } if matches!(left.as_ref(), Expr::Binary {
            op: BinaryOp::BitOr,
            ..
        }) && matches!(right.as_ref(), Expr::Binary {
            op: BinaryOp::ShiftLeft,
            left,
            ..
        } if matches!(left.as_ref(), Expr::Binary {
            op: BinaryOp::BitAnd,
            ..
        }))
    ));
}

#[test]
fn parses_omitted_function_return_type_as_void() {
    let source = "package app.main\n\nfn main() {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.functions[0].name, "main");
    assert_eq!(ast.functions[0].return_type.path, ["void"]);
    assert!(ast.functions[0].return_type.args.is_empty());
}

#[test]
fn parses_mut_call_argument() {
    let source = "package app.main\n\nfn touch(mut count: i64) -> i64 {\n    return count\n}\n\nfn main() -> void {\n    let mut count: i64 = 1\n    let value: i64 = touch(mut count)\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(ast.functions[0].params[0].mutable);
    assert!(matches!(
        ast.functions[1].body[1],
        Stmt::Let {
            value:
                Expr::Call {
                    ref args,
                    ..
                },
            ..
        } if args == &[Expr::MutArg {
            name: vec!["count".to_string()]
        }]
    ));
}

#[test]
fn parses_if_expression_and_comparison() {
    let source = "package app.main\n\nfn label(score: i64) -> string {\n    return if score >= 60 {\n        \"pass\"\n    } else {\n        \"fail\"\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::If {
                ref condition,
                ref then_branch,
                ref else_branch,
            }),
            ..
        } if matches!(
            condition.as_ref(),
            Expr::Binary {
                op: BinaryOp::GreaterEqual,
                ..
            }
        ) && then_branch.as_ref() == &Expr::String("pass".to_string())
            && else_branch.as_ref() == &Expr::String("fail".to_string())
    ));
}

#[test]
fn parses_panic_expression() {
    let source = "package app.main\n\nfn main() -> void {\n    panic(\"boom\")\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Expr {
            expr: Expr::Panic { .. },
            ..
        }
    ));
}

#[test]
fn parses_void_expression() {
    let source = "package app.main\n\nenum Result<T, E> {\n    Ok(T)\n    Err(E)\n}\n\nfn done() -> Result<void, string> {\n    return Result.Ok(void)\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Call { ref args, .. }),
            ..
        } if args == &[Expr::Void]
    ));
}

#[test]
fn parses_assignment_statement() {
    let source = "package app.main\n\nimport std.io\n\nfn main() -> void {\n    let mut count: i64 = 1\n    count = count + 1\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Assign {
            ref target,
            value: Expr::Binary { .. },
            ..
        } if target == &["count".to_string()]
    ));
}

#[test]
fn parses_compound_assignment_statement() {
    let source =
        "package app.main\n\nfn main() -> void {\n    let mut count: i64 = 1\n    count += 2\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Assign {
            ref target,
            op: AssignOp::Add,
            value: Expr::Int(2),
            ..
        } if target == &["count".to_string()]
    ));
}

#[test]
fn parses_compound_field_assignment_statement() {
    let source = "package app.main\n\nstruct Counter {\n    value: i64\n}\n\nfn main() -> void {\n    let mut counter: Counter = Counter { value: 1 }\n    counter.value &^= 1\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Assign {
            ref target,
            op: AssignOp::BitAndNot,
            value: Expr::Int(1),
            ..
        } if target == &["counter".to_string(), "value".to_string()]
    ));
}

#[test]
fn parses_postfix_update_statement() {
    let source =
        "package app.main\n\nfn main() -> void {\n    let mut count: i64 = 1\n    count++\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Postfix {
            ref target,
            op: PostfixOp::Increment,
            ..
        } if target == &["count".to_string()]
    ));
}

#[test]
fn parses_postfix_field_update_statement() {
    let source = "package app.main\n\nstruct Counter {\n    value: i64\n}\n\nfn main() -> void {\n    let mut counter: Counter = Counter { value: 1 }\n    counter.value--\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Postfix {
            ref target,
            op: PostfixOp::Decrement,
            ..
        } if target == &["counter".to_string(), "value".to_string()]
    ));
}

#[test]
fn rejects_postfix_update_as_expression_value() {
    let source = "package app.main\n\nfn main() -> void {\n    let mut count: i64 = 1\n    let next: i64 = count++\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();

    assert!(parse(Path::new("main.nomo"), &tokens).is_err());
}

#[test]
fn parses_field_assignment_statement() {
    let source = "package app.main\n\nstruct Counter {\n    value: i64\n}\n\nfn main() -> void {\n    let mut counter: Counter = Counter { value: 1 }\n    counter.value = counter.value + 1\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::Assign {
            ref target,
            value: Expr::Binary { .. },
            ..
        } if target == &["counter".to_string(), "value".to_string()]
    ));
}

#[test]
fn parses_struct_definition_and_literal() {
    let source = "package app.main\n\nstruct Point {\n    x: i64\n    y: i64\n}\n\nfn main() -> void {\n    let point: Point = Point { x: 1, y: 2 }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.structs.len(), 1);
    assert_eq!(ast.structs[0].name, "Point");
    assert_eq!(ast.structs[0].fields.len(), 2);
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            value: Expr::StructLiteral { ref type_name, .. },
            ..
        } if type_name == &["Point".to_string()]
    ));
}

#[test]
fn parses_generic_struct_definition() {
    let source = "package app.main\n\nstruct Box<T> {\n    value: T\n}\n\nfn main() -> void {\n    let item: Box<i32> = Box { value: 7 }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.structs[0].name, "Box");
    assert_eq!(ast.structs[0].type_params, ["T"]);
    let type_annotation = match &ast.functions[0].body[0] {
        Stmt::Let {
            type_annotation, ..
        } => type_annotation.as_ref().expect("expected type annotation"),
        other => panic!("unexpected statement: {other:?}"),
    };
    assert_eq!(type_annotation.path, ["Box"]);
    assert_eq!(type_annotation.args[0].path, ["i32"]);
}

#[test]
fn parses_impl_method_with_self_parameter() {
    let source = "package app.main\n\nstruct User {\n    email: string\n}\n\nimpl User {\n    pub fn get_email(self) -> string {\n        return self.email\n    }\n}\n\nfn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.impls.len(), 1);
    assert_eq!(ast.impls[0].type_name.path, ["User"]);
    assert_eq!(ast.impls[0].methods.len(), 1);
    assert!(ast.impls[0].methods[0].public);
    assert_eq!(ast.impls[0].methods[0].params[0].name, "self");
    assert_eq!(ast.impls[0].methods[0].params[0].type_ref.path, ["User"]);
}

#[test]
fn parses_pub_declarations_and_fields() {
    let source = "package app.main\n\npub struct User {\n    pub id: string\n    email: string\n}\n\npub enum Color {\n    Red\n    Blue\n}\n\npub fn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(ast.structs[0].public);
    assert!(ast.structs[0].fields[0].public);
    assert!(!ast.structs[0].fields[1].public);
    assert!(ast.enums[0].public);
    assert!(ast.functions[0].public);
}

#[test]
fn parses_enum_and_match_expression() {
    let source = "package app.main\n\nenum Color {\n    Red\n    Blue\n}\n\nfn label(color: Color) -> string {\n    return match color {\n        Color.Red => \"red\"\n        Color.Blue => \"blue\"\n    }\n}\n\nfn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.enums.len(), 1);
    assert_eq!(
        ast.enums[0]
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Red", "Blue"]
    );
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Match { ref arms, .. }),
            ..
        } if arms.len() == 2
    ));
}

#[test]
fn parses_payload_enum_and_match_binding() {
    let source = "package app.main\n\nenum MaybeInt {\n    Some(i64)\n    None\n}\n\nfn value(input: MaybeInt) -> i64 {\n    return match input {\n        MaybeInt.Some(n) => n\n        MaybeInt.None => 0\n    }\n}\n\nfn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(ast.enums[0].variants[0].payload.is_some());
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Match { ref arms, .. }),
            ..
        } if arms[0].binding.as_deref() == Some("n")
    ));
}

#[test]
fn parses_generic_enum_type_reference() {
    let source = "package app.main\n\nenum Option<T> {\n    Some(T)\n    None\n}\n\nfn main() -> void {\n    let value: Option<i64> = Option.Some(1)\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.enums[0].type_params, vec!["T"]);
    assert_eq!(
        ast.enums[0].variants[0].payload.as_ref().unwrap().path,
        ["T"]
    );
    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            ref type_annotation,
            ..
        } if type_annotation.as_ref().unwrap().args.len() == 1
    ));
}

#[test]
fn parses_question_postfix() {
    let source = "package app.main\n\nenum Result<T, E> {\n    Ok(T)\n    Err(E)\n}\n\nfn parse() -> Result<i64, string> {\n    return Result.Ok(1)\n}\n\nfn main() -> void {\n    let value: i64 = parse()?\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[1].body[0],
        Stmt::Let {
            value: Expr::Question { .. },
            ..
        }
    ));
}

#[test]
fn rejects_try_keyword_style_propagation() {
    let source = "package app.main\n\nfn parse() -> Result<i64, string> {\n    return Ok(1)\n}\n\nfn main() -> Result<i64, string> {\n    return try parse()?\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let err = parse(Path::new("main.nomo"), &tokens).unwrap_err();

    assert_eq!(err.code, "E0211");
    assert!(err.message.contains("use postfix `?` instead"));
}

#[test]
fn parses_float_literal_and_cast_expression() {
    let source = "package app.main\n\nfn ratio(age: i64) -> f64 {\n    return age as f64\n}\n\nfn main() -> void {\n    let pi: f64 = 3.14\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Return {
            value: Some(Expr::Cast { ref target, .. }),
            ..
        } if target.path == ["f64"]
    ));
    assert!(matches!(
        ast.functions[1].body[0],
        Stmt::Let {
            value: Expr::Float(ref value),
            ..
        } if value == "3.14"
    ));
}

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

#[test]
fn parses_match_scrutinee_as_expression() {
    let source = "package app.main\n\nfn print() -> void {\n    match load()? {\n        Some(text) => {\n            println(text)\n        }\n        None => {\n            println(\"none\")\n        }\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Match {
            value: Expr::Question { .. },
            ..
        }
    ));
}

#[test]
fn parses_let_else_statement() {
    let source = "package app.main\n\nfn label(value: Option<string>) -> string {\n    let Some(text) = value else {\n        return \"missing\"\n    }\n    return text\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::LetElse {
            ref pattern,
            ref binding,
            ref else_body,
            ..
        } if pattern == &vec!["Some".to_string()]
            && binding == "text"
            && matches!(else_body.as_slice(), [Stmt::Return { .. }])
    ));
}

#[test]
fn parses_if_let_statement() {
    let source = "package app.main\n\nfn label(value: Option<string>) -> string {\n    if let Some(text) = value {\n        return text\n    } else {\n        return \"missing\"\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::IfLet {
            ref pattern,
            ref binding,
            ref body,
            ref else_body,
            ..
        } if pattern == &vec!["Some".to_string()]
            && binding.as_deref() == Some("text")
            && matches!(body.as_slice(), [Stmt::Return { .. }])
            && matches!(else_body.as_deref(), Some([Stmt::Return { .. }]))
    ));
}

#[test]
fn parses_multiline_struct_literal() {
    let source = "package app.main\n\nstruct Point {\n    x: i32\n    y: i32\n}\n\nfn main() -> void {\n    let point: Point = Point {\n        x: 3,\n        y: 4,\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            value: Expr::StructLiteral { ref fields, .. },
            ..
        } if fields.len() == 2
    ));
}

#[test]
fn rejects_match_wildcards_in_v0_1() {
    for source in [
        "package app.main\n\nenum Option<T> {\n    Some(T)\n    None\n}\n\nfn label(value: Option<i32>) -> string {\n    return match value {\n        _ => \"wild\"\n        Option.None => \"none\"\n    }\n}\n",
        "package app.main\n\nenum Option<T> {\n    Some(T)\n    None\n}\n\nfn label(value: Option<i32>) -> string {\n    return match value {\n        Option.Some(_) => \"some\"\n        Option.None => \"none\"\n    }\n}\n",
    ] {
        let tokens = lex(Path::new("main.nomo"), source).unwrap();
        let err = parse(Path::new("main.nomo"), &tokens).unwrap_err();

        assert_eq!(err.code, "E0238");
        assert!(err.message.contains("not supported in v0.1"));
    }
}

#[test]
fn parses_char_literal() {
    let source = "package app.main\n\nfn main() -> void {\n    let letter: char = 'N'\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            value: Expr::Char('N'),
            ..
        }
    ));
}

#[test]
fn parses_generic_array_new_call() {
    let source = "package app.main\n\nfn main() -> void {\n    let items: Array<string> = Array.new<string>()\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::Let {
            value:
                Expr::Call {
                    ref callee,
                    ref type_args,
                    ref args,
                },
            ..
        } if callee == &["Array".to_string(), "new".to_string()]
            && type_args.len() == 1
            && type_args[0].path == ["string"]
            && args.is_empty()
    ));
}

#[test]
fn parses_generic_function_call() {
    let source = "package app.main\n\nfn identity<T>(value: T) -> T {\n    return value\n}\n\nfn main() -> void {\n    let value: i32 = identity<i32>(7)\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.functions[0].type_params, ["T"]);
    assert!(matches!(
        ast.functions[1].body[0],
        Stmt::Let {
            value:
                Expr::Call {
                    ref callee,
                    ref type_args,
                    ..
                },
            ..
        } if callee == &["identity".to_string()] && type_args[0].path == ["i32"]
    ));
}

#[test]
fn keeps_less_than_as_comparison_after_name() {
    let source = "package app.main\n\nfn main() -> void {\n    let left: i32 = 1\n    let right: i32 = 2\n    let ok: bool = left < right\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[2],
        Stmt::Let {
            value: Expr::Binary {
                op: BinaryOp::Less,
                ..
            },
            ..
        }
    ));
}

#[test]
fn parses_for_loop_three_forms() {
    let source = "package app.main\n\nfn main() -> void {\n    for {}\n    for done {}\n    for x in xs {}\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert!(matches!(
        ast.functions[0].body[0],
        Stmt::For {
            variant: ForVariant::Infinite { .. },
            ..
        }
    ));
    assert!(matches!(
        ast.functions[0].body[1],
        Stmt::For {
            variant: ForVariant::While { .. },
            ..
        }
    ));
    assert!(matches!(
        ast.functions[0].body[2],
        Stmt::For {
            variant: ForVariant::Iterate { ref binding, .. },
            ..
        } if binding == "x"
    ));
}

#[test]
fn parses_break_continue_and_defer() {
    let source = "package app.main\n\nfn main() -> void {\n    for {\n        break\n        continue\n        defer cleanup()\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    let Stmt::For {
        variant: ForVariant::Infinite { body },
        ..
    } = &ast.functions[0].body[0]
    else {
        panic!("expected infinite for loop");
    };
    assert!(matches!(body[0], Stmt::Break { .. }));
    assert!(matches!(body[1], Stmt::Continue { .. }));
    assert!(matches!(body[2], Stmt::Defer { .. }));
}

#[test]
fn parses_top_level_const() {
    let source = "package app.main\n\nconst MAX: i32 = 100\n\nfn main() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.consts.len(), 1);
    assert_eq!(ast.consts[0].name, "MAX");
    assert_eq!(ast.consts[0].type_ref.path, vec!["i32"]);
    assert!(matches!(ast.consts[0].value, Expr::Int(100)));
}

#[test]
fn parses_top_level_script_statements_after_declarations() {
    let source = "package app.main\n\nfn greeting() -> string {\n    return \"hi\"\n}\n\nlet message: string = greeting()\nio.println(message)\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(ast.functions.len(), 1);
    assert_eq!(ast.script_body.len(), 2);
    assert!(matches!(ast.script_body[0], Stmt::Let { .. }));
    assert!(matches!(ast.script_body[1], Stmt::Expr { .. }));
}

#[test]
fn rejects_declarations_after_top_level_script_statements() {
    let source = "package app.main\n\nio.println(\"hi\")\n\nfn helper() -> void {\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let err = parse(Path::new("main.nomo"), &tokens).unwrap_err();

    assert_eq!(err.code, "E0201");
    assert!(err.message.contains("declarations must appear before"));
}

#[test]
fn parser_ast_golden_snapshot() {
    let source = "package app.main\n\nimport std.option.Option\n\nstruct Box<T> {\n    value: T\n}\n\nenum State {\n    Ready\n    Done(i32)\n}\n\nfn label(value: State) -> string {\n    return match value {\n        State.Ready => \"ready\"\n        State.Done(code) => \"done\"\n    }\n}\n";
    let tokens = lex(Path::new("main.nomo"), source).unwrap();
    let ast = parse(Path::new("main.nomo"), &tokens).unwrap();

    assert_eq!(
        format!("{ast:#?}"),
        r#"SourceFile {
    package: [
        "app",
        "main",
    ],
    imports: [
        [
            "std",
            "option",
            "Option",
        ],
    ],
    structs: [
        StructDef {
            public: false,
            package: [
                "app",
                "main",
            ],
            name: "Box",
            type_params: [
                "T",
            ],
            fields: [
                Field {
                    public: false,
                    name: "value",
                    type_ref: TypeRef {
                        path: [
                            "T",
                        ],
                        args: [],
                    },
                    span: Span {
                        line: 6,
                        column: 5,
                        length: 5,
                        text: "    value: T",
                    },
                },
            ],
            span: Span {
                line: 5,
                column: 1,
                length: 1,
                text: "struct Box<T> {",
            },
        },
    ],
    enums: [
        EnumDef {
            public: false,
            package: [
                "app",
                "main",
            ],
            name: "State",
            type_params: [],
            variants: [
                EnumVariant {
                    name: "Ready",
                    payload: None,
                    span: Span {
                        line: 10,
                        column: 5,
                        length: 5,
                        text: "    Ready",
                    },
                },
                EnumVariant {
                    name: "Done",
                    payload: Some(
                        TypeRef {
                            path: [
                                "i32",
                            ],
                            args: [],
                        },
                    ),
                    span: Span {
                        line: 11,
                        column: 5,
                        length: 4,
                        text: "    Done(i32)",
                    },
                },
            ],
            span: Span {
                line: 9,
                column: 1,
                length: 1,
                text: "enum State {",
            },
        },
    ],
    interfaces: [],
    extern_blocks: [],
    impls: [],
    consts: [],
    functions: [
        Function {
            public: false,
            is_test: false,
            package: [
                "app",
                "main",
            ],
            name: "label",
            type_params: [],
            params: [
                Param {
                    name: "value",
                    mutable: false,
                    type_ref: TypeRef {
                        path: [
                            "State",
                        ],
                        args: [],
                    },
                },
            ],
            return_type: TypeRef {
                path: [
                    "string",
                ],
                args: [],
            },
            body: [
                Return {
                    value: Some(
                        Match {
                            value: Name(
                                [
                                    "value",
                                ],
                            ),
                            arms: [
                                MatchArm {
                                    pattern: [
                                        "State",
                                        "Ready",
                                    ],
                                    binding: None,
                                    value: String(
                                        "ready",
                                    ),
                                },
                                MatchArm {
                                    pattern: [
                                        "State",
                                        "Done",
                                    ],
                                    binding: Some(
                                        "code",
                                    ),
                                    value: String(
                                        "done",
                                    ),
                                },
                            ],
                        },
                    ),
                    span: Span {
                        line: 15,
                        column: 5,
                        length: 1,
                        text: "    return match value {",
                    },
                },
            ],
            span: Span {
                line: 14,
                column: 1,
                length: 1,
                text: "fn label(value: State) -> string {",
            },
        },
    ],
    script_body: [],
}"#
    );
}
