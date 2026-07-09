use super::*;

pub(super) fn emit_string_char_expr(out: &mut String, expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::StringLiteral(value) => {
            out.push_str("nomo_string_literal(\"");
            out.push_str(&escape_c_string(value));
            out.push_str("\")");
        }
        ValueExpr::CharLiteral(value) => out.push_str(&(*value as u32).to_string()),
        ValueExpr::StringCompare { left, op, right } => {
            out.push('(');
            if matches!(op, BinaryOp::NotEqual) {
                out.push('!');
            }
            out.push_str("nomo_string_equal(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push_str("))");
        }
        ValueExpr::StringLen { value } => {
            out.push_str("((uint64_t)strlen(");
            emit_string_data_expr(out, value);
            out.push_str("))");
        }
        ValueExpr::StringConcat { left, right } => {
            out.push_str("nomo_string_concat(");
            emit_expr(out, left);
            out.push_str(", ");
            emit_expr(out, right);
            out.push(')');
        }
        ValueExpr::StringIsEmpty { value } => {
            out.push_str("nomo_string_is_empty(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringContains { value, needle } => {
            out.push_str("nomo_string_contains(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, needle);
            out.push(')');
        }
        ValueExpr::StringStartsWith { value, prefix } => {
            out.push_str("nomo_string_starts_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, prefix);
            out.push(')');
        }
        ValueExpr::StringEndsWith { value, suffix } => {
            out.push_str("nomo_string_ends_with(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, suffix);
            out.push(')');
        }
        ValueExpr::StringSplit { value, separator } => {
            out.push_str("nomo_string_split(");
            emit_expr(out, value);
            out.push_str(", ");
            emit_expr(out, separator);
            out.push(')');
        }
        ValueExpr::StringTrim { value } => {
            out.push_str("nomo_string_trim(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToLower { value } => {
            out.push_str("nomo_string_to_lower(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::StringToUpper { value } => {
            out.push_str("nomo_string_to_upper(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsDigit { value } => {
            out.push_str("nomo_char_is_digit(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsAlpha { value } => {
            out.push_str("nomo_char_is_alpha(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharIsWhitespace { value } => {
            out.push_str("nomo_char_is_whitespace(");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CharToString { value } => {
            out.push_str("nomo_char_to_string(");
            emit_expr(out, value);
            out.push(')');
        }
        _ => return false,
    }
    true
}
