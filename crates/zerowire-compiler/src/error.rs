//! Compiler error and diagnostic representations.

use oxc_span::Span;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    #[error("TypeScript syntax error at span {span:?}: {message}")]
    SyntaxError { message: String, span: Span },

    #[error("Type resolution error: {message}")]
    ResolutionError { message: String, span: Option<Span> },

    #[error("Unsupported TypeScript type construct at span {span:?}: {message}")]
    UnsupportedType { message: String, span: Span },

    #[error("Layout calculation failed: {message}")]
    LayoutError { message: String },

    #[error("Perfect hash synthesis failed: {message}")]
    MphfError { message: String },
}
