use std::cmp;
use std::ops::{Add, Deref, Sub};

/// A small, `Copy`, value representing a position in a `CodeMap`'s file.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Debug, Default)]
#[repr(transparent)]
pub struct Pos(pub u32);

impl Add<u64> for Pos {
    type Output = Pos;
    fn add(self, other: u64) -> Pos {
        Pos(self.0 + other as u32)
    }
}

impl Sub<Pos> for Pos {
    type Output = u64;
    fn sub(self, other: Pos) -> u64 {
        (self.0 - other.0) as u64
    }
}

/// A range of text within a CodeMap.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct Span {
    /// The position in the codemap representing the first byte of the span.
    pub(crate) low: Pos,

    /// The position after the last byte of the span.
    pub(crate) high: Pos,
}

// compatibility with other libraries that expect `Span`s to be constructed from a `Range`
impl From<std::ops::Range<Pos>> for Span {
    fn from(r: std::ops::Range<Pos>) -> Self {
        Self { low: r.start, high: r.end }
    }
}

impl From<Span> for std::ops::Range<usize> {
    fn from(s: Span) -> Self {
        s.low.0 as usize..s.high.0 as usize
    }
}

impl Span {
    /// Makes a span from offsets relative to the start of this span.
    ///
    /// # Panics
    ///   * If `end < begin`
    ///   * If `end` is beyond the length of the span
    pub const fn subspan(&self, begin: u64, end: u64) -> Span {
        assert!(end >= begin);
        assert!(self.low.0 + end as u32 <= self.high.0);
        Span {
            low: Pos(self.low.0 + begin as u32),
            high: Pos(self.low.0 + end as u32),
        }
    }

    /// Checks if a span is contained within this span.
    pub const fn contains(&self, other: Span) -> bool {
        self.low.0 <= other.low.0 && self.high.0 >= other.high.0
    }

    /// The position in the codemap representing the first byte of the span.
    pub const fn low(&self) -> Pos {
        self.low
    }

    /// The position after the last byte of the span.
    pub const fn high(&self) -> Pos {
        self.high
    }

    /// The length in bytes of the text of the span
    pub const fn len(&self) -> u64 {
        (self.high.0 - self.low.0) as u64
    }

    /// Checks whether the span is empty
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Create a span that encloses both `self` and `other`.
    pub fn merge(&self, other: Span) -> Span {
        Span {
            low: cmp::min(self.low, other.low),
            high: cmp::max(self.high, other.high),
        }
    }
}

/// Associate a Span with a value of arbitrary type (e.g. an AST node).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Copy)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    /// Maps a `Spanned<T>` to `Spanned<U>` by applying the function to the node,
    /// leaving the span untouched.
    pub fn map_node<U, F: FnOnce(T) -> U>(self, op: F) -> Spanned<U> {
        Spanned {
            node: op(self.node),
            span: self.span,
        }
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.node
    }
}
