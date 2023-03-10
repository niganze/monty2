//! The collection of various AST nodes.

mod atom;
mod expr;
mod funcdef;
mod ifstmt;
mod import;
mod primary;
mod statement;

use logos::Span;

use crate::spanned::Spanned;

/// An explicit enumeration of all AST nodes.
#[derive(Debug, Clone)]
pub enum AstNode {
    Import(models::Import),
    ClassDef(models::ClassDef),
    FuncDef(models::FunctionDef),
    If(ifstmt::IfChain),
    Assign(models::Assign),
    Int(models::Atom),
    Str(models::Atom),
    Comment(models::Atom),
    Bool(models::Atom),
    Float(models::Atom),
    Tuple(models::Atom),
    Name(models::Atom),
    BinOp(models::Expr),
    IfExpr(models::Expr),
    Unary(models::Expr),
    NamedExpr(models::Expr),
    None(models::Atom),
    Ellipsis(models::Atom),
    Subscript(models::Primary),
    Call(models::Primary),
    Attr(models::Primary),
    Ret(models::Return),
    While(models::While),
    Annotation(models::Annotation),
    Pass,
}

impl AstNode {
    pub fn as_inner(&self) -> &dyn AstObject {
        match self {
            AstNode::Import(import) => import,
            AstNode::ClassDef(classdef) => classdef,
            AstNode::FuncDef(fndef) => fndef,
            AstNode::If(ifch) => ifch,
            AstNode::Assign(asn) => asn,
            AstNode::Pass => self,
            _ => todo!(),
        }
    }
}

impl AstObject for AstNode {
    fn into_ast_node(&self) -> AstNode {
        self.clone()
    }

    fn span(&self) -> Option<Span> {
        None
    }

    fn unspanned<'a>(&'a self) -> &'a dyn AstObject {
        self.as_inner()
    }

    fn visit_with<U>(&self, visitor: &mut dyn AstVisitor<U>, span: Option<Span>) -> U
    where
        Self: Sized,
    {
        if let Self::Pass = self {
            visitor.visit_pass()
        } else {
            match self {
                AstNode::Import(import) => import.visit_with(visitor, span),
                AstNode::ClassDef(classdef) => classdef.visit_with(visitor, span),
                AstNode::FuncDef(fndef) => fndef.visit_with(visitor, span),
                AstNode::If(ifch) => ifch.visit_with(visitor, span),
                AstNode::Assign(asn) => asn.visit_with(visitor, span),
                AstNode::Ret(ret) => ret.visit_with(visitor, span),
                AstNode::Str(st) => st.visit_with(visitor, span),
                _ => todo!("{:?}", self),
            }
        }
    }
}

impl From<AstNode> for Box<dyn AstObject> {
    fn from(node: AstNode) -> Self {
        match node {
            AstNode::Import(import) => Box::new(import),
            AstNode::ClassDef(classdef) => Box::new(classdef),
            AstNode::FuncDef(funcdef) => Box::new(funcdef),
            AstNode::If(ifstmt) => Box::new(ifstmt),
            AstNode::Pass => Box::new(Statement::Pass),
            AstNode::Assign(asn) => Box::new(asn),
            _ => todo!(),
        }
    }
}

/// An opaque representation of an AST node.
pub trait AstObject {
    /// A copy of `Self` wrapped into an `AstNode` variant.
    fn into_ast_node(&self) -> AstNode;

    /// The span of the AST object, if available.
    fn span(&self) -> Option<Span>;

    /// The inner unspanned AST object.
    fn unspanned<'a>(&'a self) -> &'a dyn AstObject;

    /// Invoke the appropriate visitor method for this current AST node.
    fn visit_with<U>(&self, visitor: &mut dyn AstVisitor<U>, span: Option<Span>) -> U
    where
        Self: Sized;
}

impl<T> AstObject for Spanned<T>
where
    T: AstObject,
{
    fn into_ast_node(&self) -> AstNode {
        self.inner.into_ast_node()
    }

    fn span(&self) -> Option<Span> {
        Some(self.span.clone())
    }

    fn unspanned<'a>(&'a self) -> &'a dyn AstObject {
        &self.inner
    }

    fn visit_with<U>(&self, visitor: &mut dyn AstVisitor<U>, span: Option<Span>) -> U
    where
        Self: Sized,
    {
        self.inner.visit_with(visitor, span.or(self.span()))
    }
}

/// A visitor trait used for walking an AST object.
pub trait AstVisitor<T = ()> {
    fn visit_any(&mut self, _: &dyn AstObject) -> T;

    fn visit_funcdef(&mut self, fndef: &FunctionDef, _span: Option<Span>) -> T {
        self.visit_any(fndef)
    }

    fn visit_expr(&mut self, expr: &Expr, _span: Option<Span>) -> T {
        self.visit_any(expr)
    }

    fn visit_int(&mut self, int: &Atom, _span: Option<Span>) -> T {
        self.visit_any(int)
    }

    fn visit_float(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_str(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_none(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_name(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_tuple(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_ellipsis(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_bool(&mut self, node: &Atom, _span: Option<Span>) -> T {
        self.visit_any(node)
    }

    fn visit_import(&mut self, import: &Import, _span: Option<Span>) -> T {
        self.visit_any(import)
    }

    fn visit_classdef(&mut self, classdef: &ClassDef, _span: Option<Span>) -> T {
        self.visit_any(classdef)
    }

    fn visit_ifstmt(&mut self, ifch: &IfChain, _span: Option<Span>) -> T {
        self.visit_any(ifch)
    }

    fn visit_pass(&mut self) -> T {
        self.visit_any(&Statement::Pass)
    }

    fn visit_assign(&mut self, asn: &Assign, _span: Option<Span>) -> T {
        self.visit_any(asn)
    }

    fn visit_return(&mut self, ret: &Return, _span: Option<Span>) -> T {
        self.visit_any(ret)
    }

    fn visit_while(&mut self, while_: &While, _span: Option<Span>) -> T {
        self.visit_any(while_)
    }

    fn visit_binop(&mut self, expr: &Expr, _span: Option<Span>) -> T {
        self.visit_any(expr)
    }

    fn visit_unary(&mut self, unary: &Expr, _span: Option<Span>) -> T {
        self.visit_any(unary)
    }

    fn visit_ternary(&mut self, ternary: &Expr, _span: Option<Span>) -> T {
        self.visit_any(ternary)
    }

    fn visit_named_expr(&mut self, expr: &Expr, _span: Option<Span>) -> T {
        self.visit_any(expr)
    }

    fn visit_call(&mut self, call: &Primary, _span: Option<Span>) -> T {
        self.visit_any(call)
    }

    fn visit_subscript(&mut self, call: &Primary, _span: Option<Span>) -> T {
        self.visit_any(call)
    }

    fn visit_attr(&mut self, attr: &Primary, _span: Option<Span>) -> T {
        self.visit_any(attr)
    }

    fn visit_module(&mut self, module: &Module, _span: Option<Span>) -> T {
        self.visit_any(module)
    }

    fn visit_annotation(&mut self, ann: &Annotation, _span: Option<Span>) -> T {
        self.visit_any(ann)
    }
}

pub use models::*;

pub mod models;
