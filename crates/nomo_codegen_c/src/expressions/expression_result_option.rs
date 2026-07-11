use super::*;

pub(super) fn emit_result_option_expr(out: &mut String, expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::ResultMapErr {
            result,
            ok_type,
            source_err_type,
            target_err_type,
            converter,
        } => {
            out.push_str(&c_result_map_err_helper_ident(&ResultMapErrInstance {
                ok_type: ok_type.clone(),
                source_err_type: source_err_type.clone(),
                target_err_type: target_err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::ResultIsOk {
            result,
            ok_type,
            err_type,
        } => {
            out.push('(');
            emit_expr(out, result);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Result",
                &[ok_type.clone(), err_type.clone()],
                "Ok",
            ));
            out.push(')');
        }
        ValueExpr::ResultIsErr {
            result,
            ok_type,
            err_type,
        } => {
            out.push('(');
            emit_expr(out, result);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Result",
                &[ok_type.clone(), err_type.clone()],
                "Err",
            ));
            out.push(')');
        }
        ValueExpr::ResultUnwrapOr {
            result,
            default,
            ok_type,
            err_type,
        } => {
            out.push_str(&c_result_unwrap_or_helper_ident(&ResultUnwrapOrInstance {
                ok_type: ok_type.clone(),
                err_type: err_type.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push_str(", ");
            emit_expr(out, default);
            out.push(')');
        }
        ValueExpr::ResultMap {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
        } => {
            out.push_str(&c_result_map_helper_ident(&ResultMapInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::ResultAndThen {
            result,
            source_ok_type,
            target_ok_type,
            err_type,
            converter,
        } => {
            out.push_str(&c_result_and_then_helper_ident(&ResultAndThenInstance {
                source_ok_type: source_ok_type.clone(),
                target_ok_type: target_ok_type.clone(),
                err_type: err_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, result);
            out.push(')');
        }
        ValueExpr::OptionIsSome {
            option,
            payload_type,
        } => {
            out.push('(');
            emit_expr(out, option);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Option",
                &[payload_type.clone()],
                "Some",
            ));
            out.push(')');
        }
        ValueExpr::OptionIsNone {
            option,
            payload_type,
        } => {
            out.push('(');
            emit_expr(out, option);
            out.push_str(".tag == ");
            out.push_str(&c_enum_variant_ident(
                "Option",
                &[payload_type.clone()],
                "None",
            ));
            out.push(')');
        }
        ValueExpr::OptionUnwrapOr {
            option,
            default,
            payload_type,
        } => {
            out.push_str(&c_option_unwrap_or_helper_ident(&OptionUnwrapOrInstance {
                payload_type: payload_type.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push_str(", ");
            emit_expr(out, default);
            out.push(')');
        }
        ValueExpr::OptionMap {
            option,
            source_type,
            target_type,
            converter,
        } => {
            out.push_str(&c_option_map_helper_ident(&OptionMapInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push(')');
        }
        ValueExpr::OptionAndThen {
            option,
            source_type,
            target_type,
            converter,
        } => {
            out.push_str(&c_option_and_then_helper_ident(&OptionAndThenInstance {
                source_type: source_type.clone(),
                target_type: target_type.clone(),
                converter: converter.clone(),
            }));
            out.push('(');
            emit_expr(out, option);
            out.push(')');
        }
        _ => return false,
    }
    true
}
