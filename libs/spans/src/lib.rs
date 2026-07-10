//! Shared source-file identities, spans, and source position mapping.

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

impl FileId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> u32 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
}

impl Span {
    pub fn new(line: usize, column: usize, length: usize, text: impl Into<String>) -> Self {
        Self {
            line,
            column,
            length,
            text: text.into(),
        }
    }

    pub fn end_column(&self) -> usize {
        self.column.saturating_add(self.length)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(id: FileId, path: PathBuf, source: String) -> Self {
        let line_starts = collect_line_starts(&source);
        Self {
            id,
            path,
            source,
            line_starts,
        }
    }

    fn replace_source(&mut self, source: String) {
        self.line_starts = collect_line_starts(&source);
        self.source = source;
    }

    pub const fn id(&self) -> FileId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line(&self, line: usize) -> Option<&str> {
        let range = self.line_byte_range(line)?;
        self.source.get(range)
    }

    pub fn byte_offset(&self, line: usize, column: usize) -> Option<usize> {
        if column == 0 {
            return None;
        }
        let range = self.line_byte_range(line)?;
        let offset = range.start.checked_add(column - 1)?;
        if offset > range.end || !self.source.is_char_boundary(offset) {
            return None;
        }
        Some(offset)
    }

    pub fn byte_range(&self, span: &Span) -> Option<Range<usize>> {
        let start = self.byte_offset(span.line, span.column)?;
        let end = start.checked_add(span.length)?;
        let line_range = self.line_byte_range(span.line)?;
        if end > line_range.end || !self.source.is_char_boundary(end) {
            return None;
        }
        Some(start..end)
    }

    pub fn span_text(&self, span: &Span) -> Option<&str> {
        self.source.get(self.byte_range(span)?)
    }

    pub fn utf16_column(&self, line: usize, column: usize) -> Option<u32> {
        let line_range = self.line_byte_range(line)?;
        let offset = self.byte_offset(line, column)?;
        u32::try_from(self.source[line_range.start..offset].encode_utf16().count()).ok()
    }

    fn line_byte_range(&self, line: usize) -> Option<Range<usize>> {
        let index = line.checked_sub(1)?;
        let start = *self.line_starts.get(index)?;
        let mut end = self
            .line_starts
            .get(index + 1)
            .map(|next| next.saturating_sub(1))
            .unwrap_or(self.source.len());
        if end > start && self.source.as_bytes().get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        Some(start..end)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
    ids_by_path: HashMap<PathBuf, FileId>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: impl Into<PathBuf>, source: impl Into<String>) -> FileId {
        let path = path.into();
        let source = source.into();
        if let Some(id) = self.ids_by_path.get(&path).copied() {
            self.files[id.index()].replace_source(source);
            return id;
        }

        let raw = u32::try_from(self.files.len()).expect("source map exhausted FileId space");
        let id = FileId::from_raw(raw);
        self.files.push(SourceFile::new(id, path.clone(), source));
        self.ids_by_path.insert(path, id);
        id
    }

    pub fn update_file(&mut self, id: FileId, source: impl Into<String>) -> bool {
        let Some(file) = self.files.get_mut(id.index()) else {
            return false;
        };
        file.replace_source(source.into());
        true
    }

    pub fn file_id(&self, path: &Path) -> Option<FileId> {
        self.ids_by_path.get(path).copied()
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.index())
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SourceFile> {
        self.files.iter()
    }
}

fn collect_line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(
        source
            .bytes()
            .enumerate()
            .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
    );
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_map_keeps_stable_ids_for_overlays() {
        let mut map = SourceMap::new();
        let first = map.add_file("src/main.nomo", "package app.main\n");
        let second = map.add_file("src/util.nomo", "package app.util\n");
        let overlaid = map.add_file("src/main.nomo", "package app.updated\n");

        assert_eq!(first, overlaid);
        assert_ne!(first, second);
        assert_eq!(map.len(), 2);
        assert_eq!(map.file(first).unwrap().source(), "package app.updated\n");
        assert_eq!(map.file_id(Path::new("src/util.nomo")), Some(second));
    }

    #[test]
    fn source_file_maps_byte_and_utf16_columns() {
        let mut map = SourceMap::new();
        let id = map.add_file("src/main.nomo", "let value = \"你\"\r\nnext\n");
        let file = map.file(id).unwrap();
        let unicode_column = file.line(1).unwrap().find('你').unwrap() + 1;

        assert_eq!(file.line_count(), 3);
        assert_eq!(file.line(1), Some("let value = \"你\""));
        assert_eq!(file.line(2), Some("next"));
        assert_eq!(file.utf16_column(1, unicode_column), Some(13));
        assert_eq!(file.byte_offset(2, 1), Some(19));
    }

    #[test]
    fn source_file_resolves_span_text() {
        let mut map = SourceMap::new();
        let id = map.add_file("src/main.nomo", "alpha beta\n");
        let file = map.file(id).unwrap();
        let span = Span::new(1, 7, 4, "alpha beta");

        assert_eq!(span.end_column(), 11);
        assert_eq!(file.byte_range(&span), Some(6..10));
        assert_eq!(file.span_text(&span), Some("beta"));
        assert_eq!(file.byte_offset(0, 1), None);
        assert_eq!(file.byte_offset(1, 12), None);
    }

    #[test]
    fn update_file_rebuilds_line_index() {
        let mut map = SourceMap::new();
        let id = map.add_file("src/main.nomo", "one\n");

        assert!(map.update_file(id, "one\ntwo\nthree"));
        assert_eq!(map.file(id).unwrap().line_count(), 3);
        assert_eq!(map.file(id).unwrap().line(3), Some("three"));
        assert!(!map.update_file(FileId::from_raw(99), "missing"));
    }
}
