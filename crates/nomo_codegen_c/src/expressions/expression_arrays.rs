use super::*;

pub(super) fn emit_array_expr(out: &mut String, expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::ArrayNew { element_type } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_new()");
        }
        ValueExpr::ArrayLen { array } => {
            out.push_str("((uint64_t)");
            emit_expr(out, array);
            out.push_str(".len)");
        }
        ValueExpr::ArrayIter {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            if expr_may_share_array_storage(array) {
                out.push_str(&c_array_ident(element_type));
                out.push_str("_retain(");
                emit_expr(out, array);
                out.push(')');
            } else {
                emit_expr(out, array);
            }
        }
        ValueExpr::ArrayGet {
            array,
            index,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_get(");
            emit_expr(out, array);
            out.push_str(", ");
            emit_expr(out, index);
            out.push(')');
        }
        ValueExpr::ArrayPop {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_pop(&");
            out.push_str(&c_var_ident(array));
            out.push(')');
        }
        ValueExpr::ArrayRemove {
            array,
            index,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_remove(&");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push(')');
        }
        ValueExpr::ArrayPush {
            array,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_push(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArraySet {
            array,
            index,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_set(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArrayInsert {
            array,
            index,
            value,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_insert(");
            out.push_str(&c_var_ident(array));
            out.push_str(", ");
            emit_expr(out, index);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::ArrayClear {
            array,
            element_type,
        } if is_supported_array_element(element_type) => {
            out.push_str(&c_array_ident(element_type));
            out.push_str("_clear(");
            out.push_str(&c_var_ident(array));
            out.push(')');
        }
        ValueExpr::ArrayNew { element_type }
        | ValueExpr::ArrayIter { element_type, .. }
        | ValueExpr::ArrayGet { element_type, .. }
        | ValueExpr::ArrayPop { element_type, .. }
        | ValueExpr::ArrayRemove { element_type, .. }
        | ValueExpr::ArrayPush { element_type, .. }
        | ValueExpr::ArraySet { element_type, .. }
        | ValueExpr::ArrayInsert { element_type, .. }
        | ValueExpr::ArrayClear { element_type, .. } => {
            panic!(
                "unsupported Array element type reached C codegen: {}",
                element_type.name()
            );
        }
        _ => return false,
    }
    true
}
