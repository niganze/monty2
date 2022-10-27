use super::statement::Statement;
use super::{AstNode, AstObject};

use crate::spanned::Spanned;

#[derive(Debug, Clone, Default)]
pub struct Module {
    pub body: Vec<Spanned<Statement>>,
}

impl AstObject for Module {
    fn into_ast_node(&self) -> AstNode {
        todo!()
    }

    fn unspanned<'a>(&'a self) -> &'a dyn AstObject {
        todo!()
    }

    // fn visit_with<U>(&self, visitor: &mut dyn AstVisitor<U>, span: Option<Span>) -> U
    // where
    //     Self: Sized,
    // {
    //     visitor.visit_module(self, span)
    // }
}
