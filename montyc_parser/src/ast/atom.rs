use logos::Span;
use montyc_core::SpanRef;

use crate::{spanned::Spanned, token::PyToken, AstNode, AstObject, AstVisitor};

use super::Expr;

#[derive(Debug, Clone, derive_more::Unwrap)]
pub enum Atom {
    None,
    Ellipsis,
    Int(i64),
    Str(SpanRef),
    Bool(bool),
    Float(f64),
    Tuple(Vec<Spanned<Expr>>),
    Comment(SpanRef),
    Name(SpanRef),
}

impl AstObject for Atom {
    fn into_ast_node(&self) -> AstNode {
        match self {
            Atom::None => AstNode::None(self.clone()),
            Atom::Ellipsis => AstNode::Ellipsis(self.clone()),
            Atom::Int(_) => AstNode::Int(self.clone()),
            Atom::Str(_) => AstNode::Str(self.clone()),
            Atom::Bool(_) => AstNode::Bool(self.clone()),
            Atom::Float(_) => AstNode::Float(self.clone()),
            Atom::Tuple(_) => AstNode::Tuple(self.clone()),
            Atom::Comment(_) => AstNode::Comment(self.clone()),
            Atom::Name(_) => AstNode::Name(self.clone()),
        }
    }

    fn span(&self) -> Option<Span> {
        None
    }

    fn unspanned<'a>(&'a self) -> &'a dyn AstObject {
        self
    }

    fn visit_with<U>(&self, visitor: &mut dyn AstVisitor<U>, span: Option<Span>) -> U
    where
        Self: Sized,
    {
        match self {
            Atom::None => visitor.visit_none(self, span.or(self.span())),
            Atom::Ellipsis => visitor.visit_ellipsis(self, span.or(self.span())),
            Atom::Int(_) => visitor.visit_int(self, span.or(self.span())),
            Atom::Str(_) => visitor.visit_str(self, span.or(self.span())),
            Atom::Bool(_) => visitor.visit_bool(self, span.or(self.span())),
            Atom::Float(_) => visitor.visit_float(self, span.or(self.span())),
            Atom::Tuple(_) => visitor.visit_tuple(self, span.or(self.span())),
            Atom::Comment(_) => visitor.visit_any(self),
            Atom::Name(_) => visitor.visit_name(self, span.or(self.span())),
        }
    }
}

impl Atom {
    pub fn as_name(&self) -> Option<SpanRef> {
        match self {
            Self::Name(t) => Some(t.clone()),
            _ => None,
        }
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(a), Self::Bool(k)) => a == k,
            (Self::Int(n), Self::Int(i)) => n == i,
            (Self::Str(s), Self::Str(t)) => s == t,
            (Self::Name(a), Self::Name(b)) => a == b,

            (Self::Ellipsis, Self::Ellipsis) | (Self::None, Self::None) => true,

            _ => unimplemented!(),
        }
    }
}

impl From<PyToken> for Atom {
    fn from(value: PyToken) -> Self {
        match value {
            PyToken::Ellipsis => Self::Ellipsis,
            PyToken::None => Self::None,
            PyToken::True => Self::Bool(true),
            PyToken::False => Self::Bool(false),
            PyToken::Digits(n) => Self::Int(n),
            PyToken::CommentRef(n) => Self::Comment(n),
            PyToken::StringRef(n) => Self::Str(n),
            PyToken::Ident(n) => Self::Name(n),
            _ => unreachable!("{:?}", value),
        }
    }
}
