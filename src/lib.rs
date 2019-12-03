//! This crate defines a
//! [Wadler-style](http://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf)
//! pretty-printing API.
//!
//! Start with with the static functions of [Doc](enum.Doc.html).
//!
//! ## Quick start
//!
//! Let's pretty-print simple sexps!  We want to pretty print sexps like
//!
//! ```lisp
//! (1 2 3)
//! ```
//! or, if the line would be too long, like
//!
//! ```lisp
//! ((1)
//!  (2 3)
//!  (4 5 6))
//! ```
//!
//! A _simple symbolic expression_ consists of a numeric _atom_ or a nested ordered _list_ of
//! symbolic expression children.
//!
//! ```rust
//! # use pretty::*;
//! enum SExp {
//!     Atom(u32),
//!     List(Vec<SExp>),
//! }
//! use SExp::*;
//! # fn main() { }
//! ```
//!
//! We define a simple conversion to a [Doc](enum.Doc.html).  Atoms are rendered as strings; lists
//! are recursively rendered, with spaces between children where appropriate.  Children are
//! [nested]() and [grouped](), allowing them to be laid out in a single line as appropriate.
//!
//! ```rust
//! # use pretty::*;
//! # enum SExp {
//! #     Atom(u32),
//! #     List(Vec<SExp>),
//! # }
//! # use SExp::*;
//! impl SExp {
//!     /// Return a pretty printed format of self.
//!     pub fn to_doc(&self) -> RcDoc<()> {
//!         match *self {
//!             Atom(ref x) => RcDoc::as_string(x),
//!             List(ref xs) =>
//!                 RcDoc::text("(")
//!                     .append(RcDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), Doc::line()).nest(1).group())
//!                     .append(RcDoc::text(")"))
//!         }
//!     }
//! }
//! # fn main() { }
//! ```
//!
//! Next, we convert the [Doc](enum.Doc.html) to a plain old string.
//!
//! ```rust
//! # use pretty::*;
//! # enum SExp {
//! #     Atom(u32),
//! #     List(Vec<SExp>),
//! # }
//! # use SExp::*;
//! # impl SExp {
//! #     /// Return a pretty printed format of self.
//! #     pub fn to_doc(&self) -> BoxDoc<()> {
//! #         match *self {
//! #             Atom(ref x) => BoxDoc::as_string(x),
//! #             List(ref xs) =>
//! #                 BoxDoc::text("(")
//! #                     .append(BoxDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), Doc::line()).nest(1).group())
//! #                     .append(BoxDoc::text(")"))
//! #         }
//! #     }
//! # }
//! impl SExp {
//!     pub fn to_pretty(&self, width: usize) -> String {
//!         let mut w = Vec::new();
//!         self.to_doc().render(width, &mut w).unwrap();
//!         String::from_utf8(w).unwrap()
//!     }
//! }
//! # fn main() { }
//! ```
//!
//! And finally we can test that the nesting and grouping behaves as we expected.
//!
//! ```rust
//! # use pretty::*;
//! # enum SExp {
//! #     Atom(u32),
//! #     List(Vec<SExp>),
//! # }
//! # use SExp::*;
//! # impl SExp {
//! #     /// Return a pretty printed format of self.
//! #     pub fn to_doc(&self) -> BoxDoc<()> {
//! #         match *self {
//! #             Atom(ref x) => BoxDoc::as_string(x),
//! #             List(ref xs) =>
//! #                 BoxDoc::text("(")
//! #                     .append(BoxDoc::intersperse(xs.into_iter().map(|x| x.to_doc()), Doc::line()).nest(1).group())
//! #                     .append(BoxDoc::text(")"))
//! #         }
//! #     }
//! # }
//! # impl SExp {
//! #     pub fn to_pretty(&self, width: usize) -> String {
//! #         let mut w = Vec::new();
//! #         self.to_doc().render(width, &mut w).unwrap();
//! #         String::from_utf8(w).unwrap()
//! #     }
//! # }
//! # fn main() {
//! let atom = SExp::Atom(5);
//! assert_eq!("5", atom.to_pretty(10));
//! let list = SExp::List(vec![SExp::Atom(1), SExp::Atom(2), SExp::Atom(3)]);
//! assert_eq!("(1 2 3)", list.to_pretty(10));
//! assert_eq!("\
//! (1
//!  2
//!  3)", list.to_pretty(5));
//! # }
//! ```
//!
//! ## Advanced usage
//!
//! There's a more efficient pattern that uses the [DocAllocator](trait.DocAllocator.html) trait, as
//! implemented by [BoxAllocator](struct.BoxAllocator.html), to allocate
//! [DocBuilder](struct.DocBuilder.html) instances.  See
//! [examples/trees.rs](https://github.com/freebroccolo/pretty.rs/blob/master/examples/trees.rs#L39)
//! for this approach.

#[cfg(feature = "termcolor")]
pub extern crate termcolor;

use std::{borrow::Cow, convert::TryInto, fmt, io, ops::Deref, rc::Rc};
#[cfg(feature = "termcolor")]
use termcolor::{ColorSpec, WriteColor};

mod render;

#[cfg(feature = "termcolor")]
pub use self::render::TermColored;
pub use self::render::{FmtWrite, IoWrite, Render, RenderAnnotated};

/// The concrete document type. This type is not meant to be used directly. Instead use the static
/// functions on `Doc` or the methods on an `DocAllocator`.
///
/// The `T` parameter is used to abstract over pointers to `Doc`. See `RefDoc` and `BoxDoc` for how
/// it is used
#[derive(Clone)]
pub enum Doc<'a, T: DocPtr<'a, A>, A = ()> {
    Nil,
    Append(T, T),
    Group(T),
    FlatAlt(T, T),
    Nest(isize, T),
    Line,
    OwnedText(Box<str>),
    BorrowedText(&'a str),
    Annotated(A, T),
    Union(T, T),
    Column(T::ColumnFn),
    Nesting(T::ColumnFn),
}

impl<'a, T, A> fmt::Debug for Doc<'a, T, A>
where
    T: DocPtr<'a, A> + fmt::Debug,
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Doc::Nil => f.debug_tuple("Nil").finish(),
            Doc::Append(ref ldoc, ref rdoc) => {
                f.debug_tuple("Append").field(ldoc).field(rdoc).finish()
            }
            Doc::FlatAlt(ref x, ref y) => f.debug_tuple("FlatAlt").field(x).field(y).finish(),
            Doc::Group(ref doc) => f.debug_tuple("Group").field(doc).finish(),
            Doc::Nest(off, ref doc) => f.debug_tuple("Nest").field(&off).field(doc).finish(),
            Doc::Line => f.debug_tuple("Line").finish(),
            Doc::OwnedText(ref s) => f.debug_tuple("Text").field(s).finish(),
            Doc::BorrowedText(ref s) => f.debug_tuple("Text").field(s).finish(),
            Doc::Annotated(ref ann, ref doc) => {
                f.debug_tuple("Annotated").field(ann).field(doc).finish()
            }
            Doc::Union(ref l, ref r) => f.debug_tuple("Union").field(l).field(r).finish(),
            Doc::Column(_) => f.debug_tuple("Column(..)").finish(),
            Doc::Nesting(_) => f.debug_tuple("Nesting(..)").finish(),
        }
    }
}

macro_rules! impl_doc {
    ($name: ident, $allocator: ident) => {
        #[derive(Clone)]
        pub struct $name<'a, A = ()>(Box<Doc<'a, $name<'a, A>, A>>);

        impl<'a, A> fmt::Debug for $name<'a, A>
        where
            A: fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl<'a, A> $name<'a, A> {
            pub fn new(doc: Doc<'a, $name<'a, A>, A>) -> $name<'a, A> {
                $name(Box::new(doc))
            }
        }

        impl<'a, A> From<Doc<'a, Self, A>> for $name<'a, A> {
            fn from(doc: Doc<'a, $name<'a, A>, A>) -> $name<'a, A> {
                $name::new(doc)
            }
        }

        impl<'a, A> Deref for $name<'a, A> {
            type Target = Doc<'a, $name<'a, A>, A>;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'a, A> DocAllocator<'a, A> for $allocator
        where
            A: 'a,
        {
            type Doc = $name<'a, A>;

            #[inline]
            fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc {
                $name::new(doc)
            }
            fn alloc_column_fn(
                &'a self,
                f: impl Fn(usize) -> Self::Doc + 'a,
            ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn {
                Rc::new(f)
            }
            fn alloc_width_fn(
                &'a self,
                f: impl Fn(isize) -> Self::Doc + 'a,
            ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn {
                Rc::new(f)
            }
        }

        impl<'a, A> DocPtr<'a, A> for $name<'a, A> {
            type ColumnFn = std::rc::Rc<dyn Fn(usize) -> Self + 'a>;
            type WidthFn = std::rc::Rc<dyn Fn(isize) -> Self + 'a>;
        }

        impl<'a, A> StaticDoc<'a, A> for $name<'a, A> {
            type Allocator = $allocator;
            const ALLOCATOR: &'static Self::Allocator = &$allocator;
        }

        impl_doc_methods!($name ('a, A) where () where ());

        impl<'a, A> $name<'a, A> {
            /// Append the given document after this document.
            #[inline]
            pub fn append<D>(self, that: D) -> Self
            where
                D: Into<BuildDoc<'a, Self, A>>,
            {
                DocBuilder(&$allocator, self.into()).append(that).into_doc()
            }

            /// A single document concatenating all the given documents.
            #[inline]
            pub fn concat<I>(docs: I) -> Self
            where
                I: IntoIterator,
                I::Item: Into<BuildDoc<'a, Self, A>>,
            {
                $allocator.concat(docs).into_doc()
            }

            /// A single document interspersing the given separator `S` between the given documents.  For
            /// example, if the documents are `[A, B, C, ..., Z]`, this yields `[A, S, B, S, C, S, ..., S, Z]`.
            ///
            /// Compare [the `intersperse` method from the `itertools` crate](https://docs.rs/itertools/0.5.9/itertools/trait.Itertools.html#method.intersperse).
            ///
            /// NOTE: The separator type, `S` may need to be cloned. Consider using cheaply cloneable ptr
            /// like `RefDoc` or `RcDoc`
            #[inline]
            pub fn intersperse<I, S>(docs: I, separator: S) -> Self
            where
                I: IntoIterator,
                I::Item: Into<BuildDoc<'a, Self, A>>,
                S: Into<BuildDoc<'a, Self, A>> + Clone,
                A: Clone,
            {
                $allocator.intersperse(docs, separator).into_doc()
            }

            /// Acts as `self` when laid out on multiple lines and acts as `that` when laid out on a single line.
            #[inline]
            pub fn flat_alt<D>(self, doc: D) -> Self
            where
                D: Into<BuildDoc<'a, Self, A>>,
            {
                DocBuilder(&$allocator, self.into())
                    .flat_alt(doc)
                    .into_doc()
            }

            /// Mark this document as a group.
            ///
            /// Groups are layed out on a single line if possible.  Within a group, all basic documents with
            /// several possible layouts are assigned the same layout, that is, they are all layed out
            /// horizontally and combined into a one single line, or they are each layed out on their own
            /// line.
            #[inline]
            pub fn group(self) -> Self {
                DocBuilder(&$allocator, self.into()).group().into_doc()
            }

            /// Increase the indentation level of this document.
            #[inline]
            pub fn nest(self, offset: isize) -> Self {
                DocBuilder(&$allocator, self.into()).nest(offset).into_doc()
            }

            #[inline]
            pub fn annotate(self, ann: A) -> Self {
                DocBuilder(&$allocator, self.into())
                    .annotate(ann)
                    .into_doc()
            }

            #[inline]
            pub fn union<D>(self, other: D) -> Self
            where
                D: Into<BuildDoc<'a, Self, A>>,
            {
                DocBuilder(&$allocator, self.into()).union(other).into_doc()
            }
        }
    };
}

macro_rules! impl_doc_methods {
    ($name: ident ( $($params: tt)* ) where ( $($where_: tt)* ) where ( $($where_2: tt)* )) => {
        impl< $($params)* > $name< $($params)* >
            where $($where_)*
        {
            /// An empty document.
            #[inline]
            pub fn nil() -> Self {
                Doc::Nil.into()
            }

            /// The text `t.to_string()`.
            ///
            /// The given text must not contain line breaks.
            #[inline]
            pub fn as_string<U: ToString>(data: U) -> Self {
                Self::text(data.to_string())
            }

            /// A single hardline.
            #[inline]
            pub fn hardline() -> Self {
                Doc::Line.into()
            }

            /// The given text, which must not contain line breaks.
            #[inline]
            pub fn text<U: Into<Cow<'a, str>>>(data: U) -> Self {
                match data.into() {
                    Cow::Owned(t) => Doc::OwnedText(t.into()).into(),
                    Cow::Borrowed(t) => Doc::BorrowedText(t).into(),
                }
            }

            #[inline]
            pub fn space() -> Self {
                Doc::BorrowedText(" ").into()
            }
        }

        impl< $($params)* > $name< $($params)* >
            where $($where_2)*
        {
            /// A line acts like a `\n` but behaves like `space` if it is grouped on a single line.
            #[inline]
            pub fn line() -> Self {
                Self::hardline().flat_alt(Self::space()).into()
            }

            /// Acts like `line` but behaves like `nil` if grouped on a single line
            #[inline]
            pub fn line_() -> Self {
                Self::hardline().flat_alt(Self::nil()).into()
            }
        }
    };
}

impl_doc!(BoxDoc, BoxAllocator);
impl_doc!(RcDoc, RcAllocator);

impl_doc_methods!(Doc ('a, D, A) where (D: DocPtr<'a, A>) where (D: StaticDoc<'a, A>));
impl_doc_methods!(BuildDoc ('a, D, A) where (D: DocPtr<'a, A>) where (D: StaticDoc<'a, A>));

pub struct BoxAllocator;

pub struct RcAllocator;

impl<'a, T, A> BuildDoc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    fn flat_alt<D>(self, doc: D) -> Self
    where
        D: Into<BuildDoc<'a, T, A>>,
    {
        DocBuilder(&T::ALLOCATOR, self.into()).flat_alt(doc).1
    }
}

impl<'a, T, A> Doc<'a, T, A>
where
    T: StaticDoc<'a, A>,
{
    fn flat_alt<D>(self, doc: D) -> Self
    where
        D: Into<BuildDoc<'a, T, A>>,
    {
        DocBuilder(&T::ALLOCATOR, self.into())
            .flat_alt(doc)
            .into_plain_doc()
    }
}

pub trait StaticDoc<'a, A>: DocPtr<'a, A>
where
    A: 'a,
{
    type Allocator: DocAllocator<'a, A, Doc = Self> + 'static;
    const ALLOCATOR: &'static Self::Allocator;
}

impl<'a, T, A, S> From<S> for Doc<'a, T, A>
where
    T: DocPtr<'a, A>,
    S: Into<Cow<'a, str>>,
{
    fn from(s: S) -> Doc<'a, T, A> {
        Doc::text(s)
    }
}

pub struct Pretty<'a, 'd, T, A>
where
    A: 'a,
    T: DocPtr<'a, A> + 'a,
{
    doc: &'d Doc<'a, T, A>,
    width: usize,
}

impl<'a, T, A> fmt::Display for Pretty<'a, '_, T, A>
where
    T: DocPtr<'a, A>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.doc.render_fmt(self.width, f)
    }
}

impl<'a, T, A> Doc<'a, T, A>
where
    T: DocPtr<'a, A> + 'a,
{
    /// Writes a rendered document to a `std::io::Write` object.
    #[inline]
    pub fn render<W>(&self, width: usize, out: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.render_raw(width, &mut IoWrite::new(out))
    }

    /// Writes a rendered document to a `std::fmt::Write` object.
    #[inline]
    pub fn render_fmt<W>(&self, width: usize, out: &mut W) -> fmt::Result
    where
        W: ?Sized + fmt::Write,
    {
        self.render_raw(width, &mut FmtWrite::new(out))
    }

    /// Writes a rendered document to a `RenderAnnotated<A>` object.
    #[inline]
    pub fn render_raw<W>(&self, width: usize, out: &mut W) -> Result<(), W::Error>
    where
        W: ?Sized + render::RenderAnnotated<A>,
    {
        render::best(self, width, out)
    }

    /// Returns a value which implements `std::fmt::Display`
    ///
    /// ```
    /// use pretty::{Doc, BoxDoc};
    /// let doc = BoxDoc::<()>::group(
    ///     BoxDoc::text("hello").append(Doc::line()).append(Doc::text("world"))
    /// );
    /// assert_eq!(format!("{}", doc.pretty(80)), "hello world");
    /// ```
    #[inline]
    pub fn pretty<'d>(&'d self, width: usize) -> Pretty<'a, 'd, T, A> {
        Pretty { doc: self, width }
    }
}

#[cfg(feature = "termcolor")]
impl<'a, T> Doc<'a, T, ColorSpec>
where
    T: DocPtr<'a, ColorSpec> + 'a,
{
    #[inline]
    pub fn render_colored<W>(&self, width: usize, out: W) -> io::Result<()>
    where
        W: WriteColor,
    {
        render::best(self, width, &mut TermColored::new(out))
    }
}

/// The `DocBuilder` type allows for convenient appending of documents even for arena allocated
/// documents by storing the arena inline.
pub struct DocBuilder<'a, D, A = ()>(pub &'a D, pub BuildDoc<'a, D::Doc, A>)
where
    D: ?Sized + DocAllocator<'a, A>;

impl<'a, A, D> Clone for DocBuilder<'a, D, A>
where
    A: Clone,
    D: DocAllocator<'a, A> + 'a,
    D::Doc: Clone,
{
    fn clone(&self) -> Self {
        DocBuilder(self.0, self.1.clone())
    }
}

impl<'a, D, A> Into<BuildDoc<'a, D::Doc, A>> for DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
{
    fn into(self) -> BuildDoc<'a, D::Doc, A> {
        self.1
    }
}

pub trait DocPtr<'a, A>: Deref<Target = Doc<'a, Self, A>> + Sized
where
    A: 'a,
{
    type ColumnFn: Deref<Target = dyn Fn(usize) -> Self + 'a> + Clone + 'a;
    type WidthFn: Deref<Target = dyn Fn(isize) -> Self + 'a> + Clone + 'a;
}

impl<'a, A> DocPtr<'a, A> for RefDoc<'a, A> {
    type ColumnFn = &'a (dyn Fn(usize) -> Self + 'a);
    type WidthFn = &'a (dyn Fn(isize) -> Self + 'a);
}

/// The `DocAllocator` trait abstracts over a type which can allocate (pointers to) `Doc`.
pub trait DocAllocator<'a, A = ()>
where
    A: 'a,
{
    type Doc: DocPtr<'a, A>;

    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc;

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn;

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn;

    fn alloc_cow(&'a self, doc: BuildDoc<'a, Self::Doc, A>) -> Self::Doc {
        match doc {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => self.alloc(d),
        }
    }

    /// Allocate an empty document.
    #[inline]
    fn nil(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Nil.into())
    }

    /// Allocate a single hardline.
    #[inline]
    fn hardline(&'a self) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Line.into())
    }

    #[inline]
    fn space(&'a self) -> DocBuilder<'a, Self, A> {
        self.text(" ")
    }

    /// A line acts like a `\n` but behaves like `space` if it is grouped on a single line.
    #[inline]
    fn line(&'a self) -> DocBuilder<'a, Self, A> {
        self.hardline().flat_alt(self.space())
    }

    /// Acts like `line` but behaves like `nil` if grouped on a single line
    ///
    /// ```
    /// use pretty::{Doc, RcDoc};
    ///
    /// let doc = RcDoc::<()>::group(
    ///     RcDoc::text("(")
    ///         .append(
    ///             RcDoc::line_()
    ///                 .append(Doc::text("test"))
    ///                 .append(Doc::line())
    ///                 .append(Doc::text("test"))
    ///                 .nest(2),
    ///         )
    ///         .append(Doc::line_())
    ///         .append(Doc::text(")")),
    /// );
    /// assert_eq!(doc.pretty(5).to_string(), "(\n  test\n  test\n)");
    /// assert_eq!(doc.pretty(100).to_string(), "(test test)");
    /// ```
    #[inline]
    fn line_(&'a self) -> DocBuilder<'a, Self, A> {
        self.hardline().flat_alt(self.nil())
    }

    /// A `softline` acts like `space` if the document fits the page, otherwise like `line`
    #[inline]
    fn softline(&'a self) -> DocBuilder<'a, Self, A> {
        self.line().group()
    }

    /// A `softline_` acts like `nil` if the document fits the page, otherwise like `line_`
    #[inline]
    fn softline_(&'a self) -> DocBuilder<'a, Self, A> {
        self.line_().group()
    }

    /// Allocate a document containing the text `t.to_string()`.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    fn as_string<U: ToString>(&'a self, data: U) -> DocBuilder<'a, Self, A> {
        self.text(data.to_string())
    }

    /// Allocate a document containing the given text.
    ///
    /// The given text must not contain line breaks.
    #[inline]
    fn text<U: Into<Cow<'a, str>>>(&'a self, data: U) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::text(data).into())
    }

    /// Allocate a document concatenating the given documents.
    #[inline]
    fn concat<I>(&'a self, docs: I) -> DocBuilder<'a, Self, A>
    where
        I: IntoIterator,
        I::Item: Into<BuildDoc<'a, Self::Doc, A>>,
    {
        docs.into_iter().fold(self.nil(), |a, b| a.append(b))
    }

    /// Allocate a document that intersperses the given separator `S` between the given documents
    /// `[A, B, C, ..., Z]`, yielding `[A, S, B, S, C, S, ..., S, Z]`.
    ///
    /// Compare [the `intersperse` method from the `itertools` crate](https://docs.rs/itertools/0.5.9/itertools/trait.Itertools.html#method.intersperse).
    ///
    /// NOTE: The separator type, `S` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RefDoc` or `RcDoc`
    #[inline]
    fn intersperse<I, S>(&'a self, docs: I, separator: S) -> DocBuilder<'a, Self, A>
    where
        I: IntoIterator,
        I::Item: Into<BuildDoc<'a, Self::Doc, A>>,
        S: Into<BuildDoc<'a, Self::Doc, A>> + Clone,
    {
        let mut result = self.nil();
        let mut iter = docs.into_iter();

        if let Some(first) = iter.next() {
            result = result.append(first);

            for doc in iter {
                result = result.append(separator.clone());
                result = result.append(doc);
            }
        }

        result
    }

    /// Allocate a document that acts differently based on the position and page layout
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("prefix ")
    ///     .append(arena.column(|l| {
    ///         arena.text("| <- column ").append(arena.as_string(l)).into_doc()
    ///     }));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix | <- column 7");
    /// ```
    #[inline]
    fn column(&'a self, f: impl Fn(usize) -> Self::Doc + 'a) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Column(self.alloc_column_fn(f)).into())
    }

    /// Allocate a document that acts differently based on the current nesting level
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("prefix ")
    ///     .append(arena.nesting(|l| {
    ///         arena.text("[Nested: ").append(arena.as_string(l)).append("]").into_doc()
    ///     }).nest(4));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix [Nested: 4]");
    /// ```
    #[inline]
    fn nesting(&'a self, f: impl Fn(usize) -> Self::Doc + 'a) -> DocBuilder<'a, Self, A> {
        DocBuilder(self, Doc::Nesting(self.alloc_column_fn(f)).into())
    }

    /// Reflows `text` inserting `softline` in place of any whitespace
    #[inline]
    fn reflow(&'a self, text: &'a str) -> DocBuilder<'a, Self, A>
    where
        Self: Sized,
        Self::Doc: Clone,
        A: Clone,
    {
        self.intersperse(text.split(char::is_whitespace), self.line().group())
    }
}

/// Either a `Doc` or a pointer to a `Doc` (`D`)
#[derive(Clone, Debug)]
pub enum BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    DocPtr(D),
    Doc(Doc<'a, D, A>),
}

impl<'a, D, A> Deref for BuildDoc<'a, D, A>
where
    D: DocPtr<'a, A>,
{
    type Target = Doc<'a, D, A>;
    fn deref(&self) -> &Self::Target {
        match self {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => d,
        }
    }
}

impl<'a, A> From<RefDoc<'a, A>> for BuildDoc<'a, RefDoc<'a, A>, A> {
    fn from(s: RefDoc<'a, A>) -> Self {
        BuildDoc::DocPtr(s)
    }
}

impl<'a, A> From<BoxDoc<'a, A>> for BuildDoc<'a, BoxDoc<'a, A>, A> {
    fn from(s: BoxDoc<'a, A>) -> Self {
        BuildDoc::DocPtr(s)
    }
}

impl<'a, A> From<RcDoc<'a, A>> for BuildDoc<'a, RcDoc<'a, A>, A> {
    fn from(s: RcDoc<'a, A>) -> Self {
        BuildDoc::DocPtr(s)
    }
}

impl<'a, T, A> From<Doc<'a, T, A>> for BuildDoc<'a, T, A>
where
    T: DocPtr<'a, A>,
{
    fn from(s: Doc<'a, T, A>) -> Self {
        BuildDoc::Doc(s)
    }
}

impl<'a, T, A, S> From<S> for BuildDoc<'a, T, A>
where
    T: DocPtr<'a, A>,
    S: Into<Cow<'a, str>>,
{
    fn from(s: S) -> Self {
        BuildDoc::Doc(Doc::text(s))
    }
}

impl<'a, 's, D, A> DocBuilder<'a, D, A>
where
    D: ?Sized + DocAllocator<'a, A>,
{
    /// Append the given document after this document.
    #[inline]
    pub fn append<E>(self, that: E) -> DocBuilder<'a, D, A>
    where
        E: Into<BuildDoc<'a, D::Doc, A>>,
    {
        let DocBuilder(allocator, this) = self;
        let that = that.into();
        let doc = match (&*this, &*that) {
            (Doc::Nil, _) => that,
            (_, Doc::Nil) => this,
            _ => Doc::Append(allocator.alloc_cow(this), allocator.alloc_cow(that)).into(),
        };
        DocBuilder(allocator, doc)
    }

    /// Acts as `self` when laid out on multiple lines and acts as `that` when laid out on a single line.
    ///
    /// ```
    /// use pretty::{Arena, DocAllocator};
    ///
    /// let arena = Arena::<()>::new();
    /// let body = arena.line().append("x");
    /// let doc = arena.text("let")
    ///     .append(arena.line())
    ///     .append("x")
    ///     .group()
    ///     .append(
    ///         body.clone()
    ///             .flat_alt(
    ///                 arena.line()
    ///                     .append("in")
    ///                     .append(body)
    ///             )
    ///     )
    ///     .group();
    ///
    /// assert_eq!(doc.1.pretty(100).to_string(), "let x in x");
    /// assert_eq!(doc.1.pretty(8).to_string(), "let x\nx");
    /// ```
    #[inline]
    pub fn flat_alt<E>(self, that: E) -> DocBuilder<'a, D, A>
    where
        E: Into<BuildDoc<'a, D::Doc, A>>,
    {
        let DocBuilder(allocator, this) = self;
        let that = that.into();
        DocBuilder(
            allocator,
            Doc::FlatAlt(allocator.alloc_cow(this.into()), allocator.alloc_cow(that)).into(),
        )
    }

    /// Mark this document as a group.
    ///
    /// Groups are layed out on a single line if possible.  Within a group, all basic documents with
    /// several possible layouts are assigned the same layout, that is, they are all layed out
    /// horizontally and combined into a one single line, or they are each layed out on their own
    /// line.
    #[inline]
    pub fn group(self) -> DocBuilder<'a, D, A> {
        let DocBuilder(allocator, this) = self;
        DocBuilder(allocator, Doc::Group(allocator.alloc_cow(this)).into())
    }

    /// Increase the indentation level of this document.
    #[inline]
    pub fn nest(self, offset: isize) -> DocBuilder<'a, D, A> {
        if let Doc::Nil = &*self.1 {
            return self;
        }
        if offset == 0 {
            return self;
        }
        let DocBuilder(allocator, this) = self;
        DocBuilder(
            allocator,
            Doc::Nest(offset, allocator.alloc_cow(this)).into(),
        )
    }

    #[inline]
    pub fn annotate(self, ann: A) -> DocBuilder<'a, D, A> {
        let DocBuilder(allocator, this) = self;
        DocBuilder(
            allocator,
            Doc::Annotated(ann, allocator.alloc_cow(this)).into(),
        )
    }

    #[inline]
    pub fn union<E>(self, other: E) -> DocBuilder<'a, D, A>
    where
        E: Into<BuildDoc<'a, D::Doc, A>>,
    {
        let DocBuilder(allocator, this) = self;
        let other = other.into();
        let doc = Doc::Union(allocator.alloc_cow(this), allocator.alloc_cow(other));
        DocBuilder(allocator, doc.into())
    }

    /// Lays out `self` so with the nesting level set to the current column
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RefDoc` or `RcDoc`
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("lorem").append(arena.text(" "))
    ///     .append(arena.intersperse(["ipsum", "dolor"].iter().cloned(), arena.line_()).align());
    /// assert_eq!(doc.1.pretty(80).to_string(), "lorem ipsum\n      dolor");
    /// ```
    #[inline]
    pub fn align(self) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        let allocator = self.0;
        allocator.column(move |col| {
            let self_ = self.clone();
            allocator
                .nesting(move |nest| self_.clone().nest(col as isize - nest as isize).into_doc())
                .into_doc()
        })
    }

    /// Lays out `self` with a nesting level set to the current level plus `adjust`.
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RefDoc` or `RcDoc`
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("prefix").append(arena.text(" "))
    ///     .append(arena.reflow("Indenting these words with nest").hang(4));
    /// assert_eq!(
    ///     doc.1.pretty(24).to_string(),
    ///     "prefix Indenting these\n           words with\n           nest",
    /// );
    /// ```
    #[inline]
    pub fn hang(self, adjust: isize) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        self.nest(adjust).align()
    }

    /// Indents `self` by `adjust` spaces from the current cursor position
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RefDoc` or `RcDoc`
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("prefix").append(arena.text(" "))
    ///     .append(arena.reflow("The indent function indents these words!").indent(4));
    /// assert_eq!(
    ///     doc.1.pretty(24).to_string(),
    /// "
    /// prefix     The indent
    ///            function
    ///            indents these
    ///            words!".trim_start(),
    /// );
    /// ```
    #[inline]
    pub fn indent(self, adjust: usize) -> DocBuilder<'a, D, A>
    where
        DocBuilder<'a, D, A>: Clone,
    {
        let spaces = {
            use crate::render::SPACES;
            let DocBuilder(allocator, _) = self;
            let mut doc = allocator.nil();
            let mut remaining = adjust;
            while remaining != 0 {
                let i = SPACES.len().min(remaining);
                remaining -= i;
                doc = doc.append(allocator.text(&SPACES[..i]))
            }
            doc
        };
        spaces.append(self).hang(adjust.try_into().unwrap())
    }

    /// Lays out `self` and provides the column width of it available to `f`
    ///
    /// NOTE: The doc pointer type, `D` may need to be cloned. Consider using cheaply cloneable ptr
    /// like `RefDoc` or `RcDoc`
    ///
    /// ```rust
    /// use pretty::DocAllocator;
    ///
    /// let arena = pretty::Arena::<()>::new();
    /// let doc = arena.text("prefix ")
    ///     .append(arena.column(|l| {
    ///         arena.text("| <- column ").append(arena.as_string(l)).into_doc()
    ///     }));
    /// assert_eq!(doc.1.pretty(80).to_string(), "prefix | <- column 7");
    /// ```
    #[inline]
    pub fn width(self, f: impl Fn(isize) -> D::Doc + 'a) -> DocBuilder<'a, D, A>
    where
        BuildDoc<'a, D::Doc, A>: Clone,
    {
        let DocBuilder(allocator, this) = self;
        let f = allocator.alloc_width_fn(f);
        allocator.column(move |start| {
            let f = f.clone();

            DocBuilder(allocator, this.clone())
                .append(allocator.column(move |end| f(end as isize - start as isize)))
                .into_doc()
        })
    }

    /// Puts `self` between `before` and `after`
    #[inline]
    pub fn enclose<E, F>(self, before: E, after: F) -> DocBuilder<'a, D, A>
    where
        E: Into<BuildDoc<'a, D::Doc, A>>,
        F: Into<BuildDoc<'a, D::Doc, A>>,
    {
        let DocBuilder(allocator, _) = self;
        DocBuilder(allocator, before.into())
            .append(self)
            .append(after)
    }

    pub fn single_quotes(self) -> DocBuilder<'a, D, A> {
        self.enclose("'", "'")
    }

    pub fn double_quotes(self) -> DocBuilder<'a, D, A> {
        self.enclose("\"", "\"")
    }
    pub fn parens(self) -> DocBuilder<'a, D, A> {
        self.enclose("(", ")")
    }

    pub fn angles(self) -> DocBuilder<'a, D, A> {
        self.enclose("<", ">")
    }
    pub fn braces(self) -> DocBuilder<'a, D, A> {
        self.enclose("{", "}")
    }

    pub fn brackets(self) -> DocBuilder<'a, D, A> {
        self.enclose("[", "]")
    }

    pub fn into_doc(self) -> D::Doc {
        match self.1 {
            BuildDoc::DocPtr(d) => d,
            BuildDoc::Doc(d) => self.0.alloc(d),
        }
    }

    fn into_plain_doc(self) -> Doc<'a, D::Doc, A> {
        match self.1 {
            BuildDoc::DocPtr(_) => unreachable!(),
            BuildDoc::Doc(d) => d,
        }
    }
}

/// Newtype wrapper for `&Doc`
pub struct RefDoc<'a, A>(&'a Doc<'a, RefDoc<'a, A>, A>);

impl<A> Copy for RefDoc<'_, A> {}
impl<A> Clone for RefDoc<'_, A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, A> fmt::Debug for RefDoc<'a, A>
where
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'a, A> Deref for RefDoc<'a, A> {
    type Target = Doc<'a, RefDoc<'a, A>, A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

trait DropT {}
impl<T> DropT for T {}

/// An arena which can be used to allocate `Doc` values.
pub struct Arena<'a, A = ()> {
    docs: typed_arena::Arena<Doc<'a, RefDoc<'a, A>, A>>,
    column_fns: typed_arena::Arena<Box<dyn DropT>>,
}

impl<A> Default for Arena<'_, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, A> Arena<'a, A> {
    pub fn new() -> Self {
        Arena {
            docs: typed_arena::Arena::new(),
            column_fns: Default::default(),
        }
    }

    fn alloc_any<T>(&'a self, f: T) -> &'a T
    where
        T: 'a,
    {
        let f = Box::new(f);
        let f_ptr = &*f as *const T;
        // Until #[may_dangle] https://github.com/rust-lang/rust/issues/34761 is stabilized (or
        // equivalent) we need to use unsafe to cast away the lifetime of the function as we do not
        // have any other way of asserting that the `typed_arena::Arena` destructor does not touch
        // `'a`
        //
        // Since `'a` is used elsewhere in our `Arena` type we still have all the other lifetime
        // checks in place (the other arena stores no `Drop` value which touches `'a` which lets it
        // compile)
        unsafe {
            self.column_fns
                .alloc(std::mem::transmute::<Box<dyn DropT>, Box<dyn DropT>>(f));
            &*f_ptr
        }
    }
}

impl<'a, D, A> DocAllocator<'a, A> for &'a D
where
    D: ?Sized + DocAllocator<'a, A>,
    A: 'a,
{
    type Doc = D::Doc;

    #[inline]
    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc {
        (**self).alloc(doc)
    }

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn {
        (**self).alloc_column_fn(f)
    }

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn {
        (**self).alloc_width_fn(f)
    }
}

impl<'a, A> DocAllocator<'a, A> for Arena<'a, A> {
    type Doc = RefDoc<'a, A>;

    #[inline]
    fn alloc(&'a self, doc: Doc<'a, Self::Doc, A>) -> Self::Doc {
        RefDoc(match doc {
            // Return 'static references for common variants to avoid some allocations
            Doc::Nil => &Doc::Nil,
            Doc::Line => &Doc::Line,
            // line()
            Doc::FlatAlt(RefDoc(Doc::Line), RefDoc(Doc::BorrowedText(" "))) => {
                &Doc::FlatAlt(RefDoc(&Doc::Line), RefDoc(&Doc::BorrowedText(" ")))
            }
            // line_()
            Doc::FlatAlt(RefDoc(Doc::Line), RefDoc(Doc::Nil)) => {
                &Doc::FlatAlt(RefDoc(&Doc::Line), RefDoc(&Doc::Nil))
            }
            _ => self.docs.alloc(doc),
        })
    }

    fn alloc_column_fn(
        &'a self,
        f: impl Fn(usize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::ColumnFn {
        self.alloc_any(f)
    }

    fn alloc_width_fn(
        &'a self,
        f: impl Fn(isize) -> Self::Doc + 'a,
    ) -> <Self::Doc as DocPtr<'a, A>>::WidthFn {
        self.alloc_any(f)
    }
}

#[cfg(test)]
mod tests {
    use difference;

    use super::*;

    macro_rules! test {
        ($size:expr, $actual:expr, $expected:expr) => {
            let mut s = String::new();
            $actual.render_fmt($size, &mut s).unwrap();
            difference::assert_diff!(&s, $expected, "\n", 0);
        };
        ($actual:expr, $expected:expr) => {
            test!(70, $actual, $expected)
        };
    }

    #[test]
    fn box_doc_inference() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("test")
                .append(BoxDoc::line())
                .append(BoxDoc::text("test")),
        );

        test!(doc, "test test");
    }

    #[test]
    fn newline_in_text() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("test").append(
                BoxDoc::line()
                    .append(BoxDoc::text("\"test\n     test\""))
                    .nest(4),
            ),
        );

        test!(5, doc, "test\n    \"test\n     test\"");
    }

    #[test]
    fn forced_newline() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("test")
                .append(BoxDoc::hardline())
                .append(BoxDoc::text("test")),
        );

        test!(doc, "test\ntest");
    }

    #[test]
    fn space_do_not_reset_pos() {
        let doc: BoxDoc<()> = BoxDoc::group(BoxDoc::text("test").append(BoxDoc::line()))
            .append(BoxDoc::text("test"))
            .append(BoxDoc::group(BoxDoc::line()).append(BoxDoc::text("test")));

        test!(9, doc, "test test\ntest");
    }

    // Tests that the `BoxDoc::hardline()` does not cause the rest of document to think that it fits on
    // a single line but instead breaks on the `BoxDoc::line()` to fit with 6 columns
    #[test]
    fn newline_does_not_cause_next_line_to_be_to_long() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("test").append(RcDoc::hardline()).append(
                RcDoc::text("test")
                    .append(RcDoc::line())
                    .append(RcDoc::text("test")),
            ),
        );

        test!(6, doc, "test\ntest\ntest");
    }

    #[test]
    fn newline_after_group_does_not_affect_it() {
        let arena = Arena::<()>::new();
        let doc = arena.text("x").append(arena.line()).append("y").group();

        test!(100, doc.append(arena.hardline()).1, "x y\n");
    }

    #[test]
    fn block() {
        let doc: RcDoc<()> = RcDoc::group(
            RcDoc::text("{")
                .append(
                    RcDoc::line()
                        .append(RcDoc::text("test"))
                        .append(RcDoc::line())
                        .append(RcDoc::text("test"))
                        .nest(2),
                )
                .append(RcDoc::line())
                .append(RcDoc::text("}")),
        );

        test!(5, doc, "{\n  test\n  test\n}");
    }

    #[test]
    fn line_comment() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("{")
                .append(
                    BoxDoc::line()
                        .append(BoxDoc::text("test"))
                        .append(BoxDoc::line())
                        .append(BoxDoc::text("// a").append(BoxDoc::hardline()))
                        .append(BoxDoc::text("test"))
                        .nest(2),
                )
                .append(BoxDoc::line())
                .append(BoxDoc::text("}")),
        );

        test!(14, doc, "{\n  test\n  // a\n  test\n}");
    }

    #[test]
    fn annotation_no_panic() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("test")
                .annotate(())
                .append(BoxDoc::hardline())
                .annotate(())
                .append(BoxDoc::text("test")),
        );

        test!(doc, "test\ntest");
    }

    #[test]
    fn union() {
        let arg: BoxDoc<()> = BoxDoc::text("(");
        let tuple = |line: BoxDoc<'static, ()>| {
            line.append(BoxDoc::text("x").append(",").group())
                .append(BoxDoc::line())
                .append(BoxDoc::text("1234567890").append(",").group())
                .nest(2)
                .append(BoxDoc::line_())
                .append(")")
        };

        let from = BoxDoc::text("let")
            .append(BoxDoc::line())
            .append(BoxDoc::text("x"))
            .append(BoxDoc::line())
            .append(BoxDoc::text("="))
            .group();

        let single = from
            .clone()
            .append(BoxDoc::line())
            .append(arg.clone())
            .group()
            .append(tuple(BoxDoc::line_()))
            .group();

        let hang = from
            .clone()
            .append(BoxDoc::line())
            .append(arg.clone())
            .group()
            .append(tuple(BoxDoc::hardline()))
            .group();

        let break_all = from
            .append(BoxDoc::line())
            .append(arg.clone())
            .append(tuple(BoxDoc::line()))
            .group()
            .nest(2);

        let doc = BoxDoc::group(single.union(hang.union(break_all)));

        test!(doc, "let x = (x, 1234567890,)");
        test!(8, doc, "let x =\n  (\n    x,\n    1234567890,\n  )");
        test!(14, doc, "let x = (\n  x,\n  1234567890,\n)");
    }

    #[test]
    fn usize_max_value() {
        let doc: BoxDoc<()> = BoxDoc::group(
            BoxDoc::text("test")
                .append(BoxDoc::line())
                .append(BoxDoc::text("test")),
        );

        test!(usize::max_value(), doc, "test test");
    }
}
