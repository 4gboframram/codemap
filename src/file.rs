pub use super::*;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/// A trait that represents file data
pub trait FileData {
    type Source: ?Sized + AsRef<str>;
    type Name: ?Sized + std::fmt::Display + std::fmt::Debug;

    /// The full source text
    fn source(&self) -> &Self::Source;

    /// The human-readable identifier of the data (in most cases, the name)
    fn name(&self) -> &Self::Name;
}

/// A `CodeMap`'s record of a source file.
pub struct File<T: FileData> {
    /// The span representing the entire file.
    pub span: Span,

    /// The data associated with a file
    pub(crate) source: T,

    /// Byte positions of line beginnings.
    pub(crate) lines: Vec<Pos>,
}

impl<T: FileData> Deref for File<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.source
    }
}

impl<T: FileData> File<T> {
    /// Gets the line number of a Pos.
    ///
    /// The lines are 0-indexed (first line is numbered 0)
    ///
    /// # Panics
    ///
    ///  * If `pos` is not within this file's span
    pub fn find_line(&self, pos: Pos) -> usize {
        assert!(pos >= self.span.low);
        assert!(pos <= self.span.high);
        match self.lines.binary_search(&pos) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// Gets the line and column of a Pos.
    ///
    /// # Panics
    ///
    /// * If `pos` is not with this file's span
    /// * If `pos` points to a byte in the middle of a UTF-8 character
    pub fn find_line_col(&self, pos: Pos) -> LineCol {
        let line = self.find_line(pos);
        let line_span = self.line_span(line);
        let byte_col = pos - line_span.low;
        let column = self.source_slice(line_span)[..byte_col as usize]
            .chars()
            .count();

        LineCol { line, column }
    }

    /// Gets the source text of a Span.
    ///
    /// # Panics
    ///
    ///   * If `span` is not entirely within this file.
    pub fn source_slice(&self, span: Span) -> &str {
        assert!(self.span.contains(span));
        &self.source().as_ref()
            [((span.low - self.span.low) as usize)..((span.high - self.span.low) as usize)]
    }

    /// Gets the span representing a line by line number.
    ///
    /// The line number is 0-indexed (first line is numbered 0). The returned span includes the
    /// line terminator.
    ///
    /// # Panics
    ///
    ///  * If the line number is out of range
    pub fn line_span(&self, line: usize) -> Span {
        assert!(line < self.lines.len());
        Span {
            low: self.lines[line],
            high: *self.lines.get(line + 1).unwrap_or(&self.span.high),
        }
    }

    /// Gets the source text of a line.
    ///
    /// The string returned does not include the terminating \r or \n characters.
    ///
    /// # Panics
    ///
    ///  * If the line number is out of range
    pub fn source_line(&self, line: usize) -> &str {
        self.source_slice(self.line_span(line))
            .trim_end_matches(&['\n', '\r'][..])
    }

    /// Gets the number of lines in the file
    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }
}

impl<T: FileData> fmt::Debug for File<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "File({:?})", self.name())
    }
}

impl<T: FileData> PartialEq for File<T> {
    /// Compares by identity
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const _, other as *const _)
    }
}

impl<T: FileData> Eq for File<T> {}

impl<T: FileData> Hash for File<T> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.span.hash(hasher);
    }
}

/// A line and column.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct LineCol {
    /// The line number within the file (0-indexed).
    pub line: usize,

    /// The column within the line (0-indexed).
    pub column: usize,
}

/// A file, and a line and column within it.
#[derive(Eq, Debug)]
pub struct Loc<T: FileData> {
    pub file: Arc<File<T>>,
    pub position: LineCol,
}

impl<T: FileData> fmt::Display for Loc<T> {
    /// Formats the location as `filename:line:column`, with a 1-indexed
    /// line and column.
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}:{}:{}",
            self.file.name(),
            self.position.line + 1,
            self.position.column + 1
        )
    }
}

impl<T: FileData> Clone for Loc<T> {
    fn clone(&self) -> Self {
        Self {
            file: Arc::clone(&self.file),
            position: self.position,
        }
    }
}

impl<T: FileData> std::cmp::PartialEq for Loc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position && self.file == other.file
    }
}

/// A file, and a line and column range within it.
#[derive(Debug, Eq)]
pub struct SpanLoc<T: FileData> {
    pub file: Arc<File<T>>,
    pub begin: LineCol,
    pub end: LineCol,
}

impl<T: FileData> Clone for SpanLoc<T> {
    fn clone(&self) -> Self {
        Self {
            file: Arc::clone(&self.file),
            begin: self.begin,
            end: self.end,
        }
    }
}

impl<T: FileData> std::cmp::PartialEq for SpanLoc<T> {
    fn eq(&self, other: &Self) -> bool {
        self.begin == other.begin && self.end == other.end && self.file == other.file
    }
}
impl<T: FileData> fmt::Display for SpanLoc<T> {
    /// Formats the span as `filename:start_line:start_column: end_line:end_column`,
    /// or if the span is zero-length, `filename:line:column`, with a 1-indexed line and column.
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.begin == self.end {
            write!(
                f,
                "{}:{}:{}",
                self.file.name(),
                self.begin.line + 1,
                self.begin.column + 1
            )
        } else {
            write!(
                f,
                "{}:{}:{}: {}:{}",
                self.file.name(),
                self.begin.line + 1,
                self.begin.column + 1,
                self.end.line + 1,
                self.end.column + 1
            )
        }
    }
}

//
#[derive(Debug)]
struct BoxStr(Box<str>);
impl Deref for BoxStr {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for BoxStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl AsRef<str> for BoxStr {
    fn as_ref(&self) -> &str {
        self
    }
}

/// A default implementation of `FileData` that contains
#[derive(Debug)]
pub struct DefaultFileData {
    name: BoxStr,
    contents: BoxStr,
}

impl DefaultFileData {
    pub fn new(name: String, contents: String) -> Self {
        Self {
            name: BoxStr(name.into_boxed_str()),
            contents: BoxStr(contents.into_boxed_str()),
        }
    }
}

impl FileData for DefaultFileData {
    type Source = str;
    type Name = str;

    fn source(&self) -> &Self::Source {
        &self.contents
    }

    fn name(&self) -> &Self::Name {
        &self.name
    }
}
