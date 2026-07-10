use nomo_spans::SourceFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

pub fn identifier_at_position(source: &str, position: TextPosition) -> Option<String> {
    let line = source.lines().nth(position.line as usize)?;
    let byte_index = utf16_character_to_byte_index(line, position.character);
    let bytes = line.as_bytes();
    if byte_index > bytes.len() {
        return None;
    }

    let mut start = byte_index;
    if start == bytes.len() && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) && start > 0 {
        start -= 1;
    }
    if !is_ident_byte(bytes.get(start).copied()?) {
        return None;
    }

    let mut end = start;
    while start > 0 && is_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    while end + 1 < bytes.len() && is_ident_byte(bytes[end + 1]) {
        end += 1;
    }
    Some(line[start..=end].to_string())
}

pub(super) fn range_contains(range: TextRange, position: TextPosition) -> bool {
    if position.line < range.start.line || position.line > range.end.line {
        return false;
    }
    if position.line == range.start.line && position.character < range.start.character {
        return false;
    }
    if position.line == range.end.line && position.character > range.end.character {
        return false;
    }
    true
}

pub(super) fn source_line_range(line: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: text.chars().map(|ch| ch.len_utf16() as u32).sum(),
        },
    }
}

pub fn token_range(line: usize, column: usize, text: &str) -> TextRange {
    let line = line.saturating_sub(1) as u32;
    let start = column.saturating_sub(1) as u32;
    let end = start + text.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}

pub fn token_range_in_file(file: &SourceFile, line: usize, column: usize, text: &str) -> TextRange {
    let line_index = line.saturating_sub(1) as u32;
    let start = file
        .utf16_column(line, column)
        .unwrap_or_else(|| column.saturating_sub(1) as u32);
    let end = start + text.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line: line_index,
            character: start,
        },
        end: TextPosition {
            line: line_index,
            character: end,
        },
    }
}

fn utf16_character_to_byte_index(line: &str, character: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_index, ch) in line.char_indices() {
        if utf16_count >= character {
            return byte_index;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    line.len()
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomo_spans::SourceMap;

    #[test]
    fn token_range_in_file_converts_utf8_byte_columns_to_utf16() {
        let source = "io.println(\"你\") value\n";
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file("src/main.nomo", source);
        let file = source_map.file(file_id).unwrap();
        let byte_column = source.find("value").unwrap() + 1;

        assert_eq!(
            token_range_in_file(file, 1, byte_column, "value"),
            TextRange {
                start: TextPosition {
                    line: 0,
                    character: 16,
                },
                end: TextPosition {
                    line: 0,
                    character: 21,
                },
            }
        );
    }
}
