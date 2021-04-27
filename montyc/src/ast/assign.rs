use std::rc::Rc;

use crate::prelude::*;

use super::{atom::Atom, expr::Expr, AstObject, ObjectIter, Spanned};

#[derive(Debug, PartialEq, Clone)]
pub struct Assign {
    pub name: Spanned<Atom>,
    pub value: Spanned<Expr>,
    pub kind: Option<Spanned<Atom>>,
}

impl Parseable for Assign {
    const PARSER: ParserT<Self> = crate::parser::comb::assignment_unspanned;
}

impl AstObject for Assign {
    fn unspanned(&self) -> Rc<dyn AstObject> {
        Rc::new(self.clone())
    }

    fn span(&self) -> Option<logos::Span> {
        None
    }

    fn walk<'a>(&'a self) -> Option<ObjectIter> {
        let mut it = vec![
            // Rc::new(self.name.clone()) as Rc<dyn AstObject>,
            Rc::new(self.value.clone()) as Rc<dyn AstObject>,
        ];

        if let Some(kind) = self.kind.clone() {
            it.push(Rc::new(kind) as Rc<dyn AstObject>);
        }

        Some(Box::new(it.into_iter()))
    }
}

impl TypedObject for Assign {
    fn infer_type<'a>(&self, _ctx: &LocalContext<'a>) -> crate::Result<LocalTypeId> {
        Ok(TypeMap::NEVER) // assignments do not have types, their values do however.
    }

    fn typecheck<'a>(&self, ctx: &LocalContext<'a>) -> crate::Result<()> {
        let expected = match self.kind.as_ref() {
            Some(at) => Some(at.infer_type(ctx)?),
            None => None,
        };

        let actual = {
            let mut ctx = ctx.clone();
            ctx.this = Some(Rc::new(self.value.clone()) as Rc<dyn AstObject>);
            self.value.infer_type(&ctx)?
        };

        if let ScopeRoot::Func(func) = ctx.scope.root() {
            if let Atom::Name(name) = &self.name.inner {
                if let Some((ty, span)) = func.vars.get(name).map(|kv| kv.value().clone()) {
                    if ty != actual {
                        ctx.exit_with_error(MontyError::IncompatibleReassignment {
                            name: name.clone(),
                            first_assigned: span.clone(),
                            incorrectly_reassigned: ctx.this.clone().unwrap().span().unwrap(),
                            expected: ty,
                            actual,
                        })
                    }
                } else {
                    func.vars.insert(
                        name.clone(),
                        (actual, ctx.this.clone().unwrap().span().unwrap()),
                    );
                }
            }
        } else {
            log::warn!("typecheck:assign unbound assignment in non-function scope {:?}", self);
            // unreachable!("{:?}", ctx.scope.root());
        }

        if let Some(expected) = expected {
            if expected != actual {
                ctx.exit_with_error(MontyError::IncompatibleTypes {
                    left_span: self.name.span.clone(),
                    left: expected,
                    right_span: self.value.span.clone(),
                    right: actual,
                });
            }
        }

        Ok(())
    }
}

impl LookupTarget for Assign {
    fn is_named(&self, target: crate::parser::SpanEntry) -> bool {
        self.name.is_named(target)
    }

    fn name(&self) -> crate::parser::SpanEntry {
        self.name.name()
    }
}
