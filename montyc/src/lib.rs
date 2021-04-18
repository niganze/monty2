#![feature(
    variant_count,
    bool_to_option,
    drain_filter,
    assert_matches,
    type_ascription
)]
#![allow(warnings)]

use std::rc::Rc;

use ast::{
    expr::{Expr, InfixOp},
    funcdef::{self, FunctionDef},
    retrn::Return,
    AstObject, Spanned,
};
use codespan_reporting::diagnostic::{Diagnostic, Label};
use context::LocalContext;
use parser::Span;
use typing::{LocalTypeId, TypedObject};

pub mod ast;
pub mod class;
pub mod context;
pub mod func;
pub mod parser;
pub mod scope;
pub mod typing;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MontyError>;

#[macro_use]
#[macro_export]
macro_rules! isinstance {
    ($e:expr, $t:ident) => {{
        if let Some($crate::ast::Spanned { inner, .. }) =
            $crate::scope::downcast_ref::<$crate::ast::Spanned<$t>>($e)
        {
            Some(inner)
        } else {
            $crate::scope::downcast_ref::<$t>($e)
        }
    }};
}

#[derive(Debug, Error)]
pub enum MontyError {
    #[error("Failed to infer the type of a value (internal error.)")]
    InferenceFailure,

    #[error("Incompatible return type.")]
    BadReturnType {
        expected: LocalTypeId,
        actual: LocalTypeId,
        ret_node: Rc<Spanned<Return>>,
        def_node: Rc<Spanned<FunctionDef>>,
    },

    #[error("Incompatible return type due to implicit return.")]
    MissingReturn {
        expected: LocalTypeId,
        actual: LocalTypeId,
        def_node: Rc<Spanned<FunctionDef>>,
    },

    #[error("Failed to infer the type of a value.")]
    UnknownType {
        node: Rc<dyn AstObject>,
    },

    #[error("Could not find the definition of this variable.")]
    UndefinedVariable {
        node: Rc<dyn AstObject>,
    },

    #[error("Incompatible argument type.")]
    BadArgumentType {
        expected: LocalTypeId,
        actual: LocalTypeId,
        arg_node: Rc<Spanned<Expr>>,
        def_node: Rc<Spanned<FunctionDef>>,
    },

    #[error("foo")]
    BadBinaryOp {
        span: Span,
        left: LocalTypeId,
        right: LocalTypeId,
        op: InfixOp,
    },
}

impl MontyError {
    pub fn into_diagnostic(self, ctx: &LocalContext<'_>) -> codespan_reporting::diagnostic::Diagnostic<()> {
        match self {
            MontyError::InferenceFailure => unreachable!(),

            MontyError::BadReturnType {
                expected,
                actual,
                ret_node,
                def_node
            } => {
                let mut labels = vec![];
                let type_map = ctx.global_context.type_map.borrow();

                let ret_span = ret_node.span().unwrap();

                let primary = match &ret_node.inner.value {
                    Some(value) => Label::primary((), value.span().unwrap_or(ret_span))
                        .with_message("but value of this type is returned instead."),

                    None => Label::primary((), ret_span).with_message("`None` is returned here."),
                };

                labels.push(primary);

                let def = Label::secondary(
                    (),
                    def_node
                        .inner
                        .returns
                        .span()
                        .unwrap_or(def_node.span.clone()),
                )
                .with_message(if def_node.inner.returns.is_some() {
                    format!(
                        "function defined here supposedly returning a value of {}.",
                        type_map.get(expected).unwrap()
                    )
                } else {
                    "function defined here is expected to return `None`".to_string()
                });

                labels.push(def);

                Diagnostic::error()
                    .with_message("incomaptible return type for function.")
                    .with_labels(labels)
                    .with_notes(vec![
                        format!("expected: {}", type_map.get(expected).unwrap()),
                        format!("actual: {}", type_map.get(actual).unwrap()),
                    ])
            }

            MontyError::MissingReturn {
                expected,
                actual,
                def_node,
            } => Diagnostic::error()
                .with_message("missing return for annotated function.")
                .with_labels(vec![
                    Label::primary((), def_node.span().unwrap())
                        .with_message("This function will implicitly return `None`"),
                    Label::secondary((), def_node.inner.returns.span().unwrap())
                        .with_message("But it has been annotated to return this instead."),
                ]),

            MontyError::UnknownType { node, } => Diagnostic::error()
                .with_message("unable to infer type for value.")
                .with_labels(vec![Label::primary((), node.span().unwrap())
                    .with_message("annotate this name with a type")]),

            MontyError::UndefinedVariable { node } => Diagnostic::error()
                .with_message("use of undefined variable.")
                .with_labels(vec![Label::primary((), node.span().unwrap()).with_message(
                    format!(
                        "\"{}\" is not defined anywhere.",
                        ctx.global_context
                            .resolver
                            .sources
                            .borrow()
                            .get(&ctx.module_ref)
                            .unwrap()
                            .get(node.span().unwrap())
                            .unwrap()
                            .to_string()
                    ),
                )]),

            MontyError::BadArgumentType {
                expected,
                actual,
                arg_node,
                def_node,
            } => Diagnostic::error()
                .with_message("incompatible argument type in function call.")
                .with_labels(vec![
                    Label::primary((), arg_node.span.clone()).with_message(format!(
                        "The argument here evaluates to \"{}\"",
                        ctx.global_context.resolver.resolve_type(actual).unwrap()
                    )),
                    Label::secondary((), def_node.inner.name.span.clone()).with_message(format!(
                        "Function defined here expected \"{}\"",
                        ctx.global_context.resolver.resolve_type(expected).unwrap(),
                    )),
                ]),

            MontyError::BadBinaryOp {
                span,
                left,
                right,
                op,
            } => {
                let ty = ctx.global_context.type_map.borrow();

                let left = ty.get(left).unwrap();
                let right = ty.get(right).unwrap();

                Diagnostic::error()
                    .with_message("Unsupported binary expression.")
                    .with_labels(vec![Label::primary((), span.clone()).with_message(
                        format!(
                            "Operator {:?} is not supported for types \"{}\" and \"{}\"",
                            op.sigil(),
                            left,
                            right
                        ),
                    )])
            }
        }
    }
}


use structopt::*;

#[derive(Debug, StructOpt)]
pub struct CompilerOptions {
    #[structopt(short, long, parse(from_os_str), default_value = "libstd/")]
    pub libstd: std::path::PathBuf,

    #[structopt(parse(from_os_str))]
    pub input: Option<std::path::PathBuf>,
}
