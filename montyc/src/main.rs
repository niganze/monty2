use cranelift_module::Linkage;
use montyc::{context::GlobalContext, prelude::*, CompilerOptions};

use structopt::StructOpt;

fn main() {
    env_logger::init();

    let opts = CompilerOptions::from_args();
    let file = opts.input.clone();

    let isa = opts.codegen_settings();

    let opts = opts.verify();

    let mut global_context = GlobalContext::initialize(&opts);

    global_context.load_module(file.clone(), move |ctx, mref| {
        let mctx = ctx.modules.get(&mref).unwrap();

        ctx.database.insert_module(mctx);

        for (obj, lctx) in ctx.walk(mref.clone()) {
            obj.typecheck(&lctx).unwrap_or_compiler_error(&lctx);
        }
    });

    let mut cctx = montyc::codegen::context::CodegenBackend::new(&global_context, isa);

    cctx.declare_functions(global_context.functions.borrow().iter().enumerate().map(
        |(idx, func)| {
            (
                idx,
                func.as_ref(),
                func.scope.module_ref(),
                Linkage::Export,
                cranelift_codegen::isa::CallConv::SystemV,
            )
        },
    ));

    cctx.finish(file.file_stem().unwrap());
}
