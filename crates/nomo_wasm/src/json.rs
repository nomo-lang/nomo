const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: u32 = 128;
const MAX_VALUES: u64 = 262_144;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonError {
    pub code: &'static str,
    pub message: &'static str,
    pub offset: usize,
}

impl JsonError {
    fn new(code: &'static str, offset: usize) -> Self {
        let message = match code {
            "limit" => "json limit exceeded",
            "unsupported_string" => "json string is not representable",
            "invalid_number" => "invalid json number",
            _ => "invalid json syntax",
        };
        Self {
            code,
            message,
            offset,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonKind {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Object,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonNode {
    pub raw: String,
    pub value: JsonNodeValue,
    pub max_depth: u32,
    pub values: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum JsonNodeValue {
    Null,
    Boolean(bool),
    Number(String),
    String(String),
    Array(Vec<JsonNode>),
    Object(Vec<(String, JsonNode)>),
}

impl JsonNode {
    pub fn kind(&self) -> JsonKind {
        match self.value {
            JsonNodeValue::Null => JsonKind::Null,
            JsonNodeValue::Boolean(_) => JsonKind::Boolean,
            JsonNodeValue::Number(_) => JsonKind::Number,
            JsonNodeValue::String(_) => JsonKind::String,
            JsonNodeValue::Array(_) => JsonKind::Array,
            JsonNodeValue::Object(_) => JsonKind::Object,
        }
    }
}

pub(crate) fn parse(input: &str) -> Result<JsonNode, JsonError> {
    if input.len() > MAX_BYTES {
        return Err(JsonError::new("limit", MAX_BYTES));
    }
    let mut parser = Parser {
        input,
        index: 0,
        values: 0,
    };
    let mut node = parser.value(0)?;
    parser.whitespace();
    if parser.index != input.len() {
        return Err(JsonError::new("syntax", parser.index));
    }
    node.raw = input.to_string();
    Ok(node)
}

pub(crate) fn from_number_text(input: &str) -> Result<String, JsonError> {
    if input.len() > MAX_BYTES {
        return Err(JsonError::new("limit", 0));
    }
    let mut parser = Parser {
        input,
        index: 0,
        values: 0,
    };
    if input.is_empty() || parser.number().is_err() || parser.index != input.len() {
        return Err(JsonError::new("invalid_number", parser.index));
    }
    Ok(input.to_string())
}

pub(crate) fn from_string(input: &str) -> Result<String, JsonError> {
    if let Some(offset) = input.as_bytes().iter().position(|byte| *byte == 0) {
        return Err(JsonError::new("unsupported_string", offset));
    }
    let mut out = String::with_capacity(input.len().saturating_add(2));
    out.push('"');
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1f}' => {
                use std::fmt::Write;
                write!(&mut out, "\\u{:04x}", ch as u32).expect("writing to String cannot fail");
            }
            ch => out.push(ch),
        }
        if out.len() > MAX_BYTES {
            return Err(JsonError::new("limit", 0));
        }
    }
    out.push('"');
    if out.len() > MAX_BYTES {
        return Err(JsonError::new("limit", 0));
    }
    Ok(out)
}

pub(crate) fn from_array(values: &[String]) -> Result<String, JsonError> {
    let mut total_values = 1_u64;
    let mut max_child_depth = 0_u32;
    let mut size = 2_usize;
    let mut trimmed = Vec::with_capacity(values.len());
    for (index, raw) in values.iter().enumerate() {
        let node = parse(raw)?;
        total_values = total_values
            .checked_add(node.values)
            .filter(|count| *count <= MAX_VALUES)
            .ok_or_else(|| JsonError::new("limit", 0))?;
        max_child_depth = max_child_depth.max(node.max_depth);
        let raw = raw.trim_matches(is_json_whitespace);
        size = size
            .checked_add(raw.len())
            .and_then(|size| size.checked_add(usize::from(index > 0)))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or_else(|| JsonError::new("limit", 0))?;
        trimmed.push(raw);
    }
    if max_child_depth + 1 > MAX_DEPTH {
        return Err(JsonError::new("limit", 0));
    }
    let mut out = String::with_capacity(size);
    out.push('[');
    for (index, raw) in trimmed.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(raw);
    }
    out.push(']');
    Ok(out)
}

pub(crate) fn from_object(values: &[(String, String)]) -> Result<String, JsonError> {
    let mut total_values = 1_u64;
    let mut max_child_depth = 0_u32;
    let mut size = 2_usize;
    let mut encoded = Vec::with_capacity(values.len());
    for (index, (key, raw)) in values.iter().enumerate() {
        let key = from_string(key)?;
        let node = parse(raw)?;
        total_values = total_values
            .checked_add(node.values)
            .filter(|count| *count <= MAX_VALUES)
            .ok_or_else(|| JsonError::new("limit", 0))?;
        max_child_depth = max_child_depth.max(node.max_depth);
        let raw = raw.trim_matches(is_json_whitespace);
        size = size
            .checked_add(key.len())
            .and_then(|size| size.checked_add(1))
            .and_then(|size| size.checked_add(raw.len()))
            .and_then(|size| size.checked_add(usize::from(index > 0)))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or_else(|| JsonError::new("limit", 0))?;
        encoded.push((key, raw));
    }
    if max_child_depth + 1 > MAX_DEPTH {
        return Err(JsonError::new("limit", 0));
    }
    let mut out = String::with_capacity(size);
    out.push('{');
    for (index, (key, raw)) in encoded.into_iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&key);
        out.push(':');
        out.push_str(raw);
    }
    out.push('}');
    Ok(out)
}

fn is_json_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\r' | '\t')
}

struct Parser<'a> {
    input: &'a str,
    index: usize,
    values: u64,
}

impl Parser<'_> {
    fn value(&mut self, parent_depth: u32) -> Result<JsonNode, JsonError> {
        self.whitespace();
        let start = self.index;
        if self.values >= MAX_VALUES {
            return Err(JsonError::new("limit", self.index));
        }
        self.values += 1;
        let before_values = self.values;
        let (value, max_depth) = match self.byte() {
            Some(b'n') => {
                self.literal("null")?;
                (JsonNodeValue::Null, 0)
            }
            Some(b't') => {
                self.literal("true")?;
                (JsonNodeValue::Boolean(true), 0)
            }
            Some(b'f') => {
                self.literal("false")?;
                (JsonNodeValue::Boolean(false), 0)
            }
            Some(b'"') => (JsonNodeValue::String(self.string()?), 0),
            Some(b'-' | b'0'..=b'9') => {
                let number_start = self.index;
                self.number()?;
                (
                    JsonNodeValue::Number(self.input[number_start..self.index].to_string()),
                    0,
                )
            }
            Some(b'[') => {
                let depth = parent_depth + 1;
                if depth > MAX_DEPTH {
                    return Err(JsonError::new("limit", self.index));
                }
                let (items, nested_depth) = self.array(depth)?;
                (JsonNodeValue::Array(items), depth.max(nested_depth))
            }
            Some(b'{') => {
                let depth = parent_depth + 1;
                if depth > MAX_DEPTH {
                    return Err(JsonError::new("limit", self.index));
                }
                let (members, nested_depth) = self.object(depth)?;
                (JsonNodeValue::Object(members), depth.max(nested_depth))
            }
            Some(_) => return Err(JsonError::new("syntax", self.index)),
            None => return Err(JsonError::new("syntax", self.input.len())),
        };
        let end = self.index;
        Ok(JsonNode {
            raw: self.input[start..end].to_string(),
            value,
            max_depth,
            values: self.values - before_values + 1,
        })
    }

    fn array(&mut self, depth: u32) -> Result<(Vec<JsonNode>, u32), JsonError> {
        self.index += 1;
        self.whitespace();
        let mut values = Vec::new();
        let mut max_depth = depth;
        if self.consume(b']') {
            return Ok((values, max_depth));
        }
        loop {
            let value = self.value(depth)?;
            max_depth = max_depth.max(value.max_depth);
            values.push(value);
            self.whitespace();
            if self.consume(b']') {
                return Ok((values, max_depth));
            }
            self.expect(b',')?;
            self.whitespace();
        }
    }

    fn object(&mut self, depth: u32) -> Result<(Vec<(String, JsonNode)>, u32), JsonError> {
        self.index += 1;
        self.whitespace();
        let mut values = Vec::new();
        let mut max_depth = depth;
        if self.consume(b'}') {
            return Ok((values, max_depth));
        }
        loop {
            if self.byte() != Some(b'"') {
                return Err(JsonError::new("syntax", self.index));
            }
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            let value = self.value(depth)?;
            max_depth = max_depth.max(value.max_depth);
            values.push((key, value));
            self.whitespace();
            if self.consume(b'}') {
                return Ok((values, max_depth));
            }
            self.expect(b',')?;
            self.whitespace();
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(byte) = self.byte() else {
                return Err(JsonError::new("syntax", self.input.len()));
            };
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(out);
                }
                0x00..=0x1f => return Err(JsonError::new("syntax", self.index)),
                b'\\' => {
                    let escape_offset = self.index;
                    self.index += 1;
                    let Some(escaped) = self.byte() else {
                        return Err(JsonError::new("syntax", self.input.len()));
                    };
                    self.index += 1;
                    match escaped {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{08}'),
                        b'f' => out.push('\u{0c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let first = self.hex4()?;
                            if first == 0 {
                                return Err(JsonError::new("unsupported_string", escape_offset));
                            }
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if !self.consume(b'\\') || !self.consume(b'u') {
                                    return Err(JsonError::new(
                                        "unsupported_string",
                                        escape_offset,
                                    ));
                                }
                                let second = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(JsonError::new(
                                        "unsupported_string",
                                        escape_offset,
                                    ));
                                }
                                0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err(JsonError::new("unsupported_string", escape_offset));
                            } else {
                                first
                            };
                            out.push(char::from_u32(scalar).expect("validated Unicode scalar"));
                        }
                        _ => return Err(JsonError::new("syntax", self.index - 1)),
                    }
                }
                byte if byte < 0x80 => {
                    out.push(byte as char);
                    self.index += 1;
                }
                _ => {
                    let ch = self.input[self.index..]
                        .chars()
                        .next()
                        .expect("index points at UTF-8 text");
                    out.push(ch);
                    self.index += ch.len_utf8();
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        if self.input.len().saturating_sub(self.index) < 4 {
            return Err(JsonError::new("syntax", self.input.len()));
        }
        let mut value = 0_u32;
        for _ in 0..4 {
            let byte = self.input.as_bytes()[self.index];
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a') + 10,
                b'A'..=b'F' => u32::from(byte - b'A') + 10,
                _ => return Err(JsonError::new("syntax", self.index)),
            };
            value = (value << 4) | digit;
            self.index += 1;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<(), JsonError> {
        self.consume(b'-');
        match self.byte() {
            Some(b'0') => self.index += 1,
            Some(b'1'..=b'9') => {
                self.index += 1;
                while matches!(self.byte(), Some(b'0'..=b'9')) {
                    self.index += 1;
                }
            }
            Some(_) => return Err(JsonError::new("syntax", self.index)),
            None => return Err(JsonError::new("syntax", self.input.len())),
        }
        if self.consume(b'.') {
            if !matches!(self.byte(), Some(b'0'..=b'9')) {
                return Err(JsonError::new("syntax", self.index));
            }
            while matches!(self.byte(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        if matches!(self.byte(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.byte(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            if !matches!(self.byte(), Some(b'0'..=b'9')) {
                return Err(JsonError::new("syntax", self.index));
            }
            while matches!(self.byte(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        Ok(())
    }

    fn literal(&mut self, expected: &str) -> Result<(), JsonError> {
        if self.input[self.index..].starts_with(expected) {
            self.index += expected.len();
            Ok(())
        } else {
            Err(JsonError::new("syntax", self.index))
        }
    }

    fn whitespace(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.index += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), JsonError> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(JsonError::new("syntax", self.index.min(self.input.len())))
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.byte() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_nested_raw_values_and_duplicate_members() {
        let input = " {\"x\": 1e+2,\"x\":\"\\u006eomo\",\"a\":[null]} ";
        let value = parse(input).unwrap();
        assert_eq!(value.raw, input);
        let JsonNodeValue::Object(members) = value.value else {
            panic!("expected object");
        };
        assert_eq!(members[0].1.raw, "1e+2");
        assert_eq!(members[1].0, "x");
        assert_eq!(members[1].1.raw, "\"\\u006eomo\"");
        assert_eq!(members[2].1.max_depth, 2);
    }

    #[test]
    fn rejects_unrepresentable_json_strings() {
        assert_eq!(parse("\"\\u0000\"").unwrap_err().code, "unsupported_string");
        assert_eq!(parse("\"\\ud800\"").unwrap_err().code, "unsupported_string");
        assert_eq!(
            parse("\"\\ud83d\\ude00\"").unwrap().value,
            JsonNodeValue::String("😀".into())
        );
    }

    #[test]
    fn constructs_compact_bounded_values() {
        assert_eq!(from_string("a\n\"b").unwrap(), "\"a\\n\\\"b\"");
        assert_eq!(
            from_array(&[" 1 ".into(), "{\"x\":true}".into()]).unwrap(),
            "[1,{\"x\":true}]"
        );
        assert_eq!(
            from_object(&[("x".into(), " 1 ".into()), ("x".into(), "2".into())]).unwrap(),
            "{\"x\":1,\"x\":2}"
        );
    }

    #[test]
    fn enforces_exact_text_and_depth_boundaries() {
        let exact_text = format!("\"{}\"", "a".repeat(MAX_BYTES - 2));
        assert_eq!(exact_text.len(), MAX_BYTES);
        assert!(parse(&exact_text).is_ok());

        let oversized_text = format!("\"{}\"", "a".repeat(MAX_BYTES - 1));
        let error = parse(&oversized_text).unwrap_err();
        assert_eq!(error.code, "limit");
        assert_eq!(error.offset, MAX_BYTES);

        let exact_depth = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH as usize),
            "]".repeat(MAX_DEPTH as usize)
        );
        let value = parse(&exact_depth).unwrap();
        assert_eq!(value.max_depth, MAX_DEPTH);

        let oversized_depth = format!(
            "{}null{}",
            "[".repeat(MAX_DEPTH as usize + 1),
            "]".repeat(MAX_DEPTH as usize + 1)
        );
        let error = parse(&oversized_depth).unwrap_err();
        assert_eq!(error.code, "limit");
        assert_eq!(error.offset, MAX_DEPTH as usize);
    }

    #[test]
    fn enforces_exact_value_count_boundary() {
        let exact = format!("[{}]", vec!["null"; MAX_VALUES as usize - 1].join(","));
        let value = parse(&exact).unwrap();
        assert_eq!(value.values, MAX_VALUES);

        let oversized = format!("[{}]", vec!["null"; MAX_VALUES as usize].join(","));
        let error = parse(&oversized).unwrap_err();
        assert_eq!(error.code, "limit");
        assert_eq!(error.offset, 1 + (MAX_VALUES as usize - 1) * 5);
    }

    #[test]
    fn constructors_enforce_limits_and_validate_exact_numbers() {
        assert_eq!(
            from_string(&"a".repeat(MAX_BYTES - 2)).unwrap().len(),
            MAX_BYTES
        );
        assert_eq!(
            from_string(&"a".repeat(MAX_BYTES - 1)).unwrap_err().code,
            "limit"
        );

        let mut exact_depth = "null".to_string();
        for _ in 0..MAX_DEPTH {
            exact_depth = from_array(&[exact_depth]).unwrap();
        }
        assert_eq!(parse(&exact_depth).unwrap().max_depth, MAX_DEPTH);
        assert_eq!(from_array(&[exact_depth]).unwrap_err().code, "limit");

        let mut exact_values = vec!["null".to_string(); MAX_VALUES as usize - 1];
        assert_eq!(
            from_array(&exact_values).unwrap().len(),
            1 + (MAX_VALUES as usize - 1) * 5
        );
        exact_values.push("null".to_string());
        assert_eq!(from_array(&exact_values).unwrap_err().code, "limit");

        for valid in ["0", "-0", "1E+2", "-1.25e-3"] {
            assert_eq!(from_number_text(valid).unwrap(), valid);
        }
        for invalid in ["", "01", "1.", ".1", "1e", "NaN", "Infinity", " 1"] {
            assert_eq!(
                from_number_text(invalid).unwrap_err().code,
                "invalid_number",
                "{invalid}"
            );
        }
    }

    #[test]
    fn errors_never_echo_secret_inputs() {
        let secret = "NOMO_JSON_SECRET_SENTINEL";
        for error in [
            parse(&format!("{{\"{secret}\":")).unwrap_err(),
            parse(&format!("\"{secret}\\ud800\"")).unwrap_err(),
            from_number_text(&format!("1{secret}")).unwrap_err(),
        ] {
            assert!(!error.message.contains(secret));
            assert!(!error.code.contains(secret));
        }
    }
}
