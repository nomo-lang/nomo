use super::*;

#[test]
fn accepts_string_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<string> = Array.new<string>()
    items.push("first")
    items.push("second")
    items.set(0, "updated")
    let size: u64 = items.len()
    let first: Option<string> = items.get(0)
    let message: string = match first {
        Option.Some(text) => text
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::String,
            },
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush { .. },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArraySet { .. },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet { .. },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn accepts_i32_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.push(2)
    items.set(0, 7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Option.Some(value) => if value == 7 {
            "array ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::I32,
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
}

#[test]
fn accepts_extended_array_methods() {
    let source = r#"package app.main

import std.array

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    items.insert(1, 2)
    let removed: Option<i32> = items.remove(0)
    let popped: Option<i32> = items.pop()
    items.clear()
    let size: u64 = items.len()
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayInsert {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayRemove {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayPop {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayClear {
                element_type: ValueType::I32,
                ..
            },
        } if name == "items"
    ));
    assert!(matches!(
        main.body[6],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
}

#[test]
fn accepts_array_iter_method() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = Array.new<i32>()
    items.push(1)
    let snapshot: Array<i32> = items.iter()
    for item in items.iter() {
        io.println("item")
    }
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayIter {
                element_type: ValueType::I32,
                ..
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
    assert!(matches!(
        main.body[3],
        Statement::Loop {
            kind: LoopKind::Iterate {
                element_type: ValueType::I32,
                iterable: ValueExpr::ArrayIter {
                    element_type: ValueType::I32,
                    ..
                },
                ..
            },
            ..
        }
    ));
}

#[test]
fn rejects_mutating_array_method_on_immutable_variable() {
    let source = r#"package app.main

import std.array

fn main() -> void {
    let items: Array<i32> = Array.new<i32>()
    items.push(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0501");
    assert!(err.message.contains("immutable variable"));
}

#[test]
fn accepts_struct_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io

struct Point {
    x: i32
    y: i32
}

fn main() -> void {
    let mut points: Array<Point> = Array.new<Point>()
    points.push(Point { x: 3, y: 4 })
    let first: Option<Point> = points.get(0)
    let message: string = match first {
        Option.Some(point) => if point.x == 3 {
            "struct array ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let point_type = ValueType::Struct("Point".to_string(), Vec::new());
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::Struct(ref name, ref args),
            },
            ..
        } if element.as_ref() == &point_type && name == "Point" && args.is_empty()
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::Struct(ref struct_name, ref args),
                ..
            },
        } if name == "points" && struct_name == "Point" && args.is_empty()
    ));
    assert!(matches!(
        main.body[2],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::Struct(ref struct_name, ref struct_args),
                ..
            },
            ..
        } if name == "Option"
            && args == &vec![point_type]
            && struct_name == "Point"
            && struct_args.is_empty()
    ));
}

#[test]
fn accepts_enum_array_builtins() {
    let source = r#"package app.main

import std.array
import std.io
import std.option

fn main() -> void {
    let mut values: Array<Option<i32>> = Array.new<Option<i32>>()
    values.push(Option.Some(7))
    values.push(Option.None)
    let first: Option<Option<i32>> = values.get(0)
    let message: string = match first {
        Option.Some(value) => match value {
            Option.Some(number) => if number == 7 {
                "enum array ok"
            } else {
                "wrong"
            }
            Option.None => "inner missing"
        }
        Option.None => "outer missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    let option_i32 = ValueType::Enum("Option".to_string(), vec![ValueType::I32]);
    let option_option_i32 = ValueType::Enum("Option".to_string(), vec![option_i32.clone()]);
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::Enum(ref name, ref args),
            },
            ..
        } if element.as_ref() == &option_i32 && name == "Option" && args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush {
                element_type: ValueType::Enum(ref enum_name, ref enum_args),
                ..
            },
        } if name == "values" && enum_name == "Option" && enum_args == &vec![ValueType::I32]
    ));
    assert!(matches!(
        main.body[3],
        Statement::Let {
            ref value_type,
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::Enum(ref enum_name, ref enum_args),
                ..
            },
            ..
        } if value_type == &option_option_i32
            && enum_name == "Option"
            && enum_args == &vec![ValueType::I32]
    ));
}

#[test]
fn accepts_arrays_for_all_v0_1_primitive_elements() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut strings: Array<string> = Array.new<string>()
    strings.push("nomo")
    let mut ints: Array<i64> = Array.new<i64>()
    ints.push(1)
    let mut i32s: Array<i32> = Array.new<i32>()
    i32s.push(2)
    let mut u32s: Array<u32> = Array.new<u32>()
    u32s.push(3 as u32)
    let mut u64s: Array<u64> = Array.new<u64>()
    u64s.push(4 as u64)
    let mut floats: Array<f64> = Array.new<f64>()
    floats.push(1.5)
    let mut chars: Array<char> = Array.new<char>()
    chars.push('n')
    let mut bools: Array<bool> = Array.new<bool>()
    bools.push(true)
    io.println("arrays ok")
}
"#;

    let program = parse_inline(source).unwrap();
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    let array_elements = main
        .body
        .iter()
        .filter_map(|statement| match statement {
            Statement::Let {
                value_type: ValueType::Array(element),
                initializer: ValueExpr::ArrayNew { element_type },
                ..
            } if element.as_ref() == element_type => Some(element_type.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        array_elements,
        vec![
            ValueType::String,
            ValueType::Int,
            ValueType::I32,
            ValueType::U32,
            ValueType::U64,
            ValueType::Float,
            ValueType::Char,
            ValueType::Bool,
        ]
    );
}

#[test]
fn rejects_array_void_in_type_positions_before_codegen() {
    for source in [
        r#"package app.main

import std.array

fn main() -> void {
    let values: Array<void> = Array.new<void>()
}
"#,
        r#"package app.main

import std.array

fn bad(values: Array<void>) -> void {
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

fn bad() -> Array<void> {
    return Array.new<void>()
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

struct Bad {
    values: Array<void>
}

fn main() -> void {
}
"#,
        r#"package app.main

import std.array

enum Bad {
    Values(Array<void>)
}

fn main() -> void {
}
"#,
    ] {
        let err = parse_inline(source).unwrap_err();
        assert!(err.code == "E0403" || err.code == "E0404");
        assert!(err.message.contains("Array elements"));
    }
}

#[test]
fn accepts_generic_array_type_positions_before_instantiation() {
    let source = r#"package app.main

import std.array

struct Bag<T> {
    values: Array<T>
}

fn id<T>(values: Array<T>) -> Array<T> {
    return values
}

fn main() -> void {
    let values: Array<i32> = Array.new<i32>()
    let copy: Array<i32> = id<i32>(values)
}
"#;

    let program = parse_inline(source).unwrap();
    assert_eq!(program.structs[0].type_params, ["T"]);
    let id = program
        .functions
        .iter()
        .find(|f| f.name == "id_i32")
        .unwrap();
    assert_eq!(id.return_type, ValueType::Array(Box::new(ValueType::I32)));
}

#[test]
fn accepts_specific_array_new_import() {
    let source = r#"package app.main

import std.array.new
import std.array.Array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(7)
    let first: Option<i32> = items.get(0)
    let message: string = match first {
        Option.Some(value) => if value == 7 {
            "array new import ok"
        } else {
            "wrong"
        }
        Option.None => "missing"
    }
    io.println(message)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayNew {
                element_type: ValueType::I32,
            },
            ..
        } if element.as_ref() == &ValueType::I32
    ));
}

#[test]
fn accepts_specific_array_method_imports() {
    let source = r#"package app.main

import std.env.args
import std.array.Array
import std.array.get
import std.array.clear
import std.array.insert
import std.array.iter
import std.array.len
import std.array.pop
import std.array.push
import std.array.remove
import std.array.set

fn main() -> void {
    let mut values = args()
    values.push("extra")
    values.insert(1, "middle")
    values.set(0, "program")
    let removed: Option<string> = values.remove(1)
    let popped: Option<string> = values.pop()
    values.clear()
    let snapshot: Array<string> = values.iter()
    let size: u64 = values.len()
    let first: Option<string> = values.get(0)
}
"#;

    let program = parse_inline(source).unwrap();
    assert!(program.enums.iter().any(|item| item.name == "Option"));
    let main = program.functions.iter().find(|f| f.name == "main").unwrap();
    assert!(matches!(
        main.body[0],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::EnvArgs,
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[1],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayPush { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[2],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayInsert { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[3],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArraySet { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[4],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayRemove {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[5],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayPop {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
    assert!(matches!(
        main.body[6],
        Statement::Assign {
            ref name,
            value: ValueExpr::ArrayClear { .. },
        } if name == "values"
    ));
    assert!(matches!(
        main.body[7],
        Statement::Let {
            value_type: ValueType::Array(ref element),
            initializer: ValueExpr::ArrayIter {
                element_type: ValueType::String,
                ..
            },
            ..
        } if element.as_ref() == &ValueType::String
    ));
    assert!(matches!(
        main.body[8],
        Statement::Let {
            value_type: ValueType::U64,
            initializer: ValueExpr::ArrayLen { .. },
            ..
        }
    ));
    assert!(matches!(
        main.body[9],
        Statement::Let {
            value_type: ValueType::Enum(ref name, ref args),
            initializer: ValueExpr::ArrayGet {
                element_type: ValueType::String,
                ..
            },
            ..
        } if name == "Option" && args == &vec![ValueType::String]
    ));
}

#[test]
fn rejects_unqualified_array_new_without_specific_import() {
    let source = r#"package app.main

import std.array
import std.io

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    io.println("done")
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0305");
    assert!(err.message.contains("new"));
}

#[test]
fn rejects_array_method_without_array_import() {
    let source = r#"package app.main

import std.array.new

fn main() -> void {
    let mut items: Array<i32> = new<i32>()
    items.push(1)
}
"#;

    let err = parse_inline(source).unwrap_err();
    assert_eq!(err.code, "E0301");
    assert!(err.message.contains("std.array"));
}
