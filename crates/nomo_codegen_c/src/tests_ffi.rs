use super::*;

#[test]
fn emits_cstring_and_opaque_extern_abi() {
    let program = Program {
        package: "app.main".to_string(),
        imports: vec!["std.ffi".to_string()],
        extern_functions: vec![
            ExternFunction {
                symbol: "puts".to_string(),
                params: vec![ValueType::CString],
                return_type: ValueType::I32,
            },
            ExternFunction {
                symbol: "nomo_example_allocate".to_string(),
                params: Vec::new(),
                return_type: ValueType::Opaque,
            },
            ExternFunction {
                symbol: "nomo_example_release".to_string(),
                params: vec![ValueType::Opaque],
                return_type: ValueType::Void,
            },
        ],
        structs: Vec::new(),
        enums: Vec::new(),
        consts: Vec::new(),
        functions: vec![Function {
            package: "app.main".to_string(),
            name: "main".to_string(),
            params: Vec::new(),
            return_type: ValueType::Void,
            body: vec![
                Statement::Let {
                    name: "message".to_string(),
                    value_type: ValueType::CString,
                    initializer: ValueExpr::Call {
                        name: BUILTIN_CSTRING_FROM_STRING_EXPR.to_string(),
                        args: vec![ValueExpr::StringLiteral("ffi values ok".to_string())],
                    },
                },
                Statement::Expr(ValueExpr::Call {
                    name: format!("{EXTERN_CALL_PREFIX}puts"),
                    args: vec![ValueExpr::Call {
                        name: BUILTIN_CSTRING_DATA_EXPR.to_string(),
                        args: vec![ValueExpr::Variable("message".to_string())],
                    }],
                }),
                Statement::Let {
                    name: "handle".to_string(),
                    value_type: ValueType::Opaque,
                    initializer: ValueExpr::Call {
                        name: format!("{EXTERN_CALL_PREFIX}nomo_example_allocate"),
                        args: Vec::new(),
                    },
                },
                Statement::Expr(ValueExpr::Call {
                    name: format!("{EXTERN_CALL_PREFIX}nomo_example_release"),
                    args: vec![ValueExpr::Variable("handle".to_string())],
                }),
            ],
        }],
    };

    let c = emit_c(&program);

    assert!(c.contains("extern int32_t puts(const char *);"));
    assert!(c.contains("extern void * nomo_example_allocate(void);"));
    assert!(c.contains("extern void nomo_example_release(void *);"));
    assert!(c.contains(
        "nomo_string nomo_message = nomo_cstring_from_string(nomo_string_literal(\"ffi values ok\"));"
    ));
    assert!(c.contains("puts((nomo_message).data);"));
    assert!(c.contains("void * nomo_handle = nomo_example_allocate();"));
    assert!(c.contains("nomo_example_release(nomo_handle);"));
    assert!(c.contains("nomo_string_release(nomo_message);"));
}
