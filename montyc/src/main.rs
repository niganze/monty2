use std::rc::Rc;

use cranelift_module::Linkage;
use montyc::{
    ast::stmt::Statement, context::GlobalContext, func::Function, prelude::*, CompilerOptions,
};

use structopt::StructOpt;

fn main() {
    env_logger::init();

    let opts = CompilerOptions::from_args();
    let file = opts.input.clone();

    let mut global_context = GlobalContext::from(opts);

    global_context.load_module(file.unwrap(), |ctx, mref| {
        for (obj, lctx) in ctx.walk(mref.clone()) {

            if let Some(Statement::FnDef(_)) = obj.as_ref().downcast_ref() {
                let func = Function::new(obj.clone(), &lctx).unwrap_or_compiler_error(&lctx);

                let lctx = LocalContext {
                    global_context: ctx,
                    module_ref: mref.clone(),
                    scope: Rc::new(func.scope.clone()) as Rc<_>,
                    this: lctx.this.clone(),
                };

                func.typecheck(&lctx).unwrap_or_compiler_error(&lctx);

                ctx.functions.borrow_mut().push((func, mref.clone()));
            } else {
                obj.typecheck(&lctx).unwrap_or_compiler_error(&lctx);
            }
        }

        {
            let funcs = ctx.functions.borrow();
            let mut ctx = montyc::context::codegen::CodegenBackend::new(ctx, None);

            ctx.declare_functions(funcs.iter().map(|(f, mref)| {
                (
                    f.as_ref(),
                    mref,
                    Linkage::Export,
                    cranelift_codegen::isa::CallConv::SystemV,
                )
            }));

            for (func, mref) in funcs.iter() {
                ctx.add_function_to_module(
                    func,
                    mref,
                    cranelift_module::Linkage::Export,
                    cranelift_codegen::isa::CallConv::SystemV,
                );
            }

            ctx.finish(None::<&str>);
        }
    });
}
