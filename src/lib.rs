//! A data structure for tracking source positions in language implementations, inspired by the
//! [CodeMap type in rustc's libsyntax](https://github.com/rust-lang/rust/blob/master/src/libsyntax/codemap.rs).
//!
//! The `CodeMap` tracks all source files and maps positions within them to linear indexes as if all
//! source files were concatenated. This allows a source position to be represented by a small
//! 32-bit `Pos` indexing into the `CodeMap`, under the assumption that the total amount of parsed
//! source code will not exceed 4GiB. The `CodeMap` can look up the source file, line, and column
//! of a `Pos` or `Span`, as well as provide source code snippets for error reporting.
//!
//! # Example
//! ```
//! use codemap::{CodeMap, FileData, DefaultFileData};
//! let mut codemap = CodeMap::new();
//! let file = codemap.add_file(DefaultFileData::new("test.rs".to_string(), "fn test(){\n    println!(\"Hello\");\n}\n".to_string()));
//! let string_literal_span = file.span.subspan(24, 31);
//!
//! let location = codemap.look_up_span(string_literal_span);
//! assert_eq!(location.file.name(), "test.rs");
//! assert_eq!(location.begin.line, 1);
//! assert_eq!(location.begin.column, 13);
//! assert_eq!(location.end.line, 1);
//! assert_eq!(location.end.column, 20);
//! ```
mod pos;
pub use pos::*;
mod file;
pub use file::*;

use std::cmp::Ordering;
use std::fmt;

use std::sync::Arc;

extern crate memchr;
use memchr::memchr_iter;

/// A data structure recording source code files for position lookup.
#[derive(Default, Debug)]
pub struct CodeMap<T: FileData = DefaultFileData> {
    end_pos: Pos,
    files: Vec<Arc<File<T>>>,
}

impl<T: FileData> CodeMap<T> {
    /// Creates an empty `CodeMap`.
    pub fn new() -> Self {
        CodeMap {
            end_pos: Pos(0),
            files: vec![],
        }
    }

    /// Adds a file with the given name and contents.
    ///
    /// Use the returned `File` and its `.span` property to create `Spans`
    /// representing substrings of the file.
    pub fn add_file(&mut self, source: T) -> Arc<File<T>> {
        let low = self.end_pos + 1;
        let src = source.source().as_ref();
        let high = low + src.len() as u64;
        self.end_pos = high;
        let mut lines = vec![low];

        let iter = memchr_iter(b'\n', src.as_bytes()).map(|i| low + (i + 1) as u64);
        lines.extend(iter);

        let file = Arc::new(File {
            span: Span { low, high },
            source,
            lines,
        });

        self.files.push(file.clone());
        file
    }

    /// Looks up the `File` that contains the specified position.
    pub fn find_file(&self, pos: Pos) -> &Arc<File<T>> {
        self.files
            .binary_search_by(|file| {
                if file.span.high < pos {
                    Ordering::Less
                } else if file.span.low > pos {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            })
            .ok()
            .map(|i| &self.files[i])
            .expect("Mapping unknown source location")
    }

    /// Gets the file, line, and column represented by a `Pos`.
    pub fn look_up_pos(&self, pos: Pos) -> Loc<T> {
        let file = self.find_file(pos);
        let position = file.find_line_col(pos);
        Loc {
            file: file.clone(),
            position,
        }
    }

    /// Gets the file and its line and column ranges represented by a `Span`.
    pub fn look_up_span(&self, span: Span) -> SpanLoc<T> {
        let file = self.find_file(span.low);
        let begin = file.find_line_col(span.low);
        let end = file.find_line_col(span.high);
        SpanLoc {
            file: file.clone(),
            begin,
            end,
        }
    }
}

#[test]
fn test_codemap() {
    let mut codemap = CodeMap::new();
    let f1 = codemap.add_file(DefaultFileData::new(
        "test1.rs".to_string(),
        "abcd\nefghij\nqwerty".to_string(),
    ));
    let f2 = codemap.add_file(DefaultFileData::new(
        "test2.rs".to_string(),
        "foo\nbar".to_string(),
    ));

    assert_eq!(codemap.find_file(f1.span.low()).name(), "test1.rs");
    assert_eq!(codemap.find_file(f1.span.high()).name(), "test1.rs");
    assert_eq!(codemap.find_file(f2.span.low()).name(), "test2.rs");
    assert_eq!(codemap.find_file(f2.span.high()).name(), "test2.rs");

    let x = f1.span.subspan(5, 10);
    let f = codemap.find_file(x.low);
    assert_eq!(f.name(), "test1.rs");
    assert_eq!(
        f.find_line_col(f.span.low()),
        LineCol { line: 0, column: 0 }
    );
    assert_eq!(
        f.find_line_col(f.span.low() + 4),
        LineCol { line: 0, column: 4 }
    );
    assert_eq!(
        f.find_line_col(f.span.low() + 5),
        LineCol { line: 1, column: 0 }
    );
    assert_eq!(
        f.find_line_col(f.span.low() + 16),
        LineCol { line: 2, column: 4 }
    );

    let x = f2.span.subspan(4, 7);
    assert_eq!(codemap.find_file(x.low()).name(), "test2.rs");
    assert_eq!(codemap.find_file(x.high()).name(), "test2.rs");
}

#[test]
fn test_issue2() {
    let mut codemap = CodeMap::new();
    let content = "a \nxyz\r\n";
    let file = codemap.add_file(DefaultFileData::new(
        "<test>".to_owned(),
        content.to_owned(),
    ));

    let span = file.span.subspan(2, 3);
    assert_eq!(
        codemap.look_up_span(span),
        SpanLoc {
            file: file.clone(),
            begin: LineCol { line: 0, column: 2 },
            end: LineCol { line: 1, column: 0 }
        }
    );

    assert_eq!(file.source_line(0), "a ");
    assert_eq!(file.source_line(1), "xyz");
    assert_eq!(file.source_line(2), "");
}

#[test]
fn test_multibyte() {
    let mut codemap = CodeMap::new();
    let content = "65°00′N 18°00′W 汉语\n🔬";
    let file = codemap.add_file(DefaultFileData::new(
        "<test>".to_owned(),
        content.to_owned(),
    ));

    assert_eq!(
        codemap.look_up_pos(file.span.low() + 21),
        Loc {
            file: file.clone(),
            position: LineCol {
                line: 0,
                column: 15
            }
        }
    );
    assert_eq!(
        codemap.look_up_pos(file.span.low() + 28),
        Loc {
            file: file.clone(),
            position: LineCol {
                line: 0,
                column: 18
            }
        }
    );
    assert_eq!(
        codemap.look_up_pos(file.span.low() + 33),
        Loc {
            file: file.clone(),
            position: LineCol { line: 1, column: 1 }
        }
    );
}
