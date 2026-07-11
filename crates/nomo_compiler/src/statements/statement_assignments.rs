use super::*;

pub(super) fn lower_assign_stmt(
    path: &Path,
    target: &[String],
    op: AssignOp,
    value: &AstExpr,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let compound_value = compound_assign_value(target, op, value);
    let value = compound_value.as_ref().unwrap_or(value);
    match target {
        [name] => {
            let Some(binding) = scope.get(name) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{name}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if !binding.mutable {
                let message = format!(
                    "cannot assign to immutable {} `{name}`",
                    binding_source_noun(binding)
                );
                return Err(Diagnostic::new(
                    "E0501",
                    message,
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let expected_type = binding.value_type.clone();
            let (actual_type, value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&expected_type),
                span,
            )?;
            if actual_type != expected_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "cannot assign `{}` to variable `{name}` of type `{}`",
                        actual_type.name(),
                        expected_type.name()
                    ),
                    &expected_type,
                    &actual_type,
                ));
            }
            Ok(Statement::Assign {
                name: name.clone(),
                value,
            })
        }
        [base, field] => {
            let Some(binding) = scope.get(base) else {
                return Err(Diagnostic::new(
                    "E0303",
                    format!("unknown variable `{base}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            if !binding.mutable {
                let message = format!(
                    "cannot assign to field of immutable {} `{base}`",
                    binding_source_noun(binding)
                );
                return Err(Diagnostic::new(
                    "E0501",
                    message,
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            }
            let ValueType::Struct(struct_name, struct_args) = &binding.value_type else {
                return Err(type_mismatch(
                    path,
                    span,
                    format!("`{base}` is not a struct"),
                ));
            };
            let struct_type = structs
                .get(struct_name)
                .expect("struct binding must refer to a known struct");
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|item| item.name == *field)
                .map(|item| {
                    substitute_type_params(&item.value_type, &struct_type.type_params, struct_args)
                })
            else {
                return Err(Diagnostic::new(
                    "E0316",
                    format!("struct `{struct_name}` has no field `{field}`"),
                    path,
                    span.line,
                    span.column,
                    span.length,
                    &span.text,
                ));
            };
            let (actual_type, value) = lower_value_expr_with_expected(
                path,
                value,
                scope,
                imports,
                signatures,
                structs,
                enums,
                Some(&field_type),
                span,
            )?;
            if actual_type != field_type {
                return Err(type_mismatch_expected_found(
                    path,
                    span,
                    format!(
                        "cannot assign `{}` to field `{field}` of type `{}`",
                        actual_type.name(),
                        field_type.name()
                    ),
                    &field_type,
                    &actual_type,
                ));
            }
            Ok(Statement::AssignField {
                base: base.clone(),
                field: field.clone(),
                value_type: field_type,
                value,
            })
        }
        _ => Err(Diagnostic::new(
            "E0217",
            "assignment target must be a variable or field",
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_postfix_stmt(
    path: &Path,
    target: &[String],
    op: PostfixOp,
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    span: &Span,
) -> Result<Statement, Diagnostic> {
    let assign_op = match op {
        PostfixOp::Increment => AssignOp::Add,
        PostfixOp::Decrement => AssignOp::Subtract,
    };
    lower_assign_stmt(
        path,
        target,
        assign_op,
        &AstExpr::Int(1),
        scope,
        imports,
        signatures,
        structs,
        enums,
        span,
    )
}

pub(super) fn compound_assign_value(
    target: &[String],
    op: AssignOp,
    value: &AstExpr,
) -> Option<AstExpr> {
    let op = assign_op_to_binary_op(op)?;
    Some(AstExpr::Binary {
        left: Box::new(AstExpr::Name(target.to_vec())),
        op,
        right: Box::new(value.clone()),
    })
}

pub(super) fn assign_op_to_binary_op(op: AssignOp) -> Option<AstBinaryOp> {
    match op {
        AssignOp::Assign => None,
        AssignOp::Add => Some(AstBinaryOp::Add),
        AssignOp::Subtract => Some(AstBinaryOp::Subtract),
        AssignOp::Multiply => Some(AstBinaryOp::Multiply),
        AssignOp::Divide => Some(AstBinaryOp::Divide),
        AssignOp::Remainder => Some(AstBinaryOp::Remainder),
        AssignOp::ShiftLeft => Some(AstBinaryOp::ShiftLeft),
        AssignOp::ShiftRight => Some(AstBinaryOp::ShiftRight),
        AssignOp::BitAnd => Some(AstBinaryOp::BitAnd),
        AssignOp::BitXor => Some(AstBinaryOp::BitXor),
        AssignOp::BitOr => Some(AstBinaryOp::BitOr),
        AssignOp::BitAndNot => Some(AstBinaryOp::BitAndNot),
    }
}
