use super::*;

pub(super) fn emit_collections_expr(out: &mut String, expr: &ValueExpr) -> bool {
    match expr {
        ValueExpr::CollectionsStringMapNew => {
            out.push_str("nomo_collections_map_new()");
        }
        ValueExpr::CollectionsStringMapLen { map } => {
            out.push_str("nomo_collections_map_len(");
            emit_expr(out, map);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapGet { map, key } => {
            out.push_str("nomo_collections_map_get(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapContains { map, key } => {
            out.push_str("nomo_collections_map_contains(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapSet { map, key, value } => {
            out.push_str("nomo_collections_map_set(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringMapRemove { map, key } => {
            out.push_str("nomo_collections_map_remove(");
            emit_expr(out, map);
            out.push_str(", ");
            emit_expr(out, key);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetNew => {
            out.push_str("nomo_collections_set_new()");
        }
        ValueExpr::CollectionsStringSetLen { set } => {
            out.push_str("nomo_collections_set_len(");
            emit_expr(out, set);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetContains { set, value } => {
            out.push_str("nomo_collections_set_contains(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetInsert { set, value } => {
            out.push_str("nomo_collections_set_insert(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        ValueExpr::CollectionsStringSetRemove { set, value } => {
            out.push_str("nomo_collections_set_remove(");
            emit_expr(out, set);
            out.push_str(", ");
            emit_expr(out, value);
            out.push(')');
        }
        _ => return false,
    }
    true
}
