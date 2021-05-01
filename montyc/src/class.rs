use std::{collections::HashMap, num::NonZeroUsize};

use crate::{ast::class::ClassDef, context::LocalContext, scope::LocalScope, typing::{FunctionType, LocalTypeId, TypeDescriptor, TypedObject}};

#[derive(Debug)]
pub struct Class {
    pub scope: LocalScope<ClassDef>,
    pub kind: LocalTypeId,
    pub properties: HashMap<NonZeroUsize, LocalTypeId>,
}

impl Class {
    pub fn try_unify_method(&self, ctx: &LocalContext, template: &FunctionType) -> Option<LocalTypeId> {
        for (name, kind) in self.properties.iter() {
            if *name == template.name.unwrap() {
                if ctx.global_context.type_map.unify_func(kind.clone(), &template) {
                    let ret = match ctx
                        .global_context
                        .type_map
                        .get(kind.clone())
                        .map(|i| i.value().clone())
                    {
                        Some(TypeDescriptor::Function(f)) => f.ret,
                        _ => todo!(),
                    };

                    return Some(ret);
                }
            }
        }

        None
    }
}

impl TypedObject for Class {
    fn infer_type<'a>(&self, _ctx: &crate::context::LocalContext<'a>) -> crate::Result<LocalTypeId> {
        todo!()
    }

    fn typecheck<'a>(&self, _ctx: &LocalContext<'a>) -> crate::Result<()> {
        todo!()
    }
}
