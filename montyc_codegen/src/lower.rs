use std::convert::TryInto;

use ahash::AHashMap;
use cranelift_codegen::{
    ir::{
        self, AbiParam, FuncRef, Function, InstBuilder, MemFlags, Signature, StackSlotData,
        StackSlotKind,
    },
    isa::CallConv,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{FuncId, Module};
use cranelift_object::ObjectModule;
use montyc_core::patma;
use montyc_hlir::{
    glue::HostGlue,
    typing::{BuiltinType, PythonType, TypingContext},
    value_store::ValueGraphIx,
    Const, PrivInst, RawInst, Value,
};

use crate::{module::Func, pointer::Pointer, prelude::CodegenModule, structbuf::StructBuf};

pub type CxArg<'a> = (&'a mut BuilderContext<'a>, &'a mut FunctionBuilder<'a>);

pub struct BuilderContext<'a> {
    pub(crate) host: &'a mut dyn HostGlue,
    pub(crate) cg_module: &'a mut CodegenModule,
    pub(crate) object_module: &'a mut ObjectModule,
    pub(crate) fid: FuncId,
    pub(crate) func_ix: ValueGraphIx,
    pub(crate) fids: &'a AHashMap<ValueGraphIx, FuncId>,
}

impl BuilderContext<'_> {
    pub fn build(self, func: &mut Function) {
        let mut builder_cx = FunctionBuilderContext::new();

        let Func { hlir, .. } = &*self.cg_module.functions[&self.func_ix];
        let code = &hlir.code;

        let mut builder = FunctionBuilder::new(func, &mut builder_cx);

        let libc_malloc = {
            let name = ir::ExternalName::User {
                namespace: 0,
                index: 0,
            };

            let signature = {
                let mut s = Signature::new(CallConv::SystemV);
                s.params.push(AbiParam::new(ir::types::I64));
                s.returns.push(AbiParam::new(ir::types::I64));
                builder.import_signature(s)
            };

            let data = ir::ExtFuncData {
                name,
                signature,
                colocated: false,
            };

            builder.import_function(data)
        };

        let mut f_refs: AHashMap<ValueGraphIx, FuncRef> = AHashMap::with_capacity(hlir.refs.len());
        let mut values: AHashMap<usize, ir::Value> = AHashMap::with_capacity(code.inst().len());

        let store = self.host.value_store();
        let mut store = store.borrow_mut();

        let func_rib = store.metadata(self.func_ix).rib.clone().unwrap_or_default();

        let (func_recv, func_args) =
            patma!(args_t, Value::Function  {args_t, .. } in store.get(self.func_ix).unwrap())
                .unwrap()
                .clone()
                .unwrap_or_default();

        let func_args = {
            let mut args = AHashMap::with_capacity(func_args.len());

            args.extend(
                func_args
                    .into_iter()
                    .cloned()
                    .enumerate()
                    .map(|(ix, (k, _))| (k.group(), ix + (func_recv.is_some() as usize))),
            );

            args
        };

        let mut locals = AHashMap::with_capacity(func_rib.len());

        let start = builder.create_block();
        let blocks = code
            .inst()
            .iter()
            .filter(|inst| {
                matches!(
                    inst.op,
                    RawInst::JumpTarget | RawInst::PhiRecv | RawInst::If { .. }
                )
            })
            .map(|inst| (inst.value, builder.create_block()))
            .collect::<AHashMap<_, _>>();

        builder.append_block_params_for_function_params(start);
        builder.switch_to_block(start);

        let start_params = builder.block_params(start).to_owned().into_boxed_slice();

        debug_assert_eq!(
            start_params.len(),
            func_args.len() + (func_recv.is_some() as usize)
        );

        if let Some(recv) = func_recv {
            let tid = TypingContext::TSelf;

            let scalar_ty = self.cg_module.scalar_type_of(&tid);
            let size = self.host.tcx().borrow().size_of(tid.clone());

            // FIXME: Don't force the size to a multiple of 16 bytes once
            //        Cranelift gets a way to specify stack slot alignment.
            let slot_size = (size + 15) / 16 * 16;

            let stack_slot = builder
                .create_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, slot_size));

            if func_args.contains_key(&recv.group()) {
                builder.ins().stack_store(start_params[0], stack_slot, 0);
            }

            locals.insert(recv.group(), (stack_slot, tid, scalar_ty));
        }

        for (name, tid) in func_rib.iter() {
            let scalar_ty = self.cg_module.scalar_type_of(tid);
            let size = self.host.tcx().borrow().size_of(tid.clone());

            // FIXME: Don't force the size to a multiple of 16 bytes once
            //        Cranelift gets a way to specify stack slot alignment.
            let slot_size = (size + 15) / 16 * 16;

            let stack_slot = builder
                .create_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, slot_size));

            if let Some(ix) = func_args.get(name) {
                builder.ins().stack_store(start_params[*ix], stack_slot, 0);
            }

            locals.insert(*name, (stack_slot, *tid, scalar_ty));
        }

        let mut block = start;

        for (ix, inst) in code.inst().iter().enumerate() {
            debug_assert_eq!(inst.value, ix);

            log::trace!("[BuilderContext::build] building block for inst {:?}", inst);

            if builder.current_block() != Some(block) {
                assert!(builder.is_filled(), "{:?}", builder.func);
                builder.switch_to_block(block);
            }

            match inst.op.clone() {
                RawInst::Nop
                | RawInst::Import { .. }
                | RawInst::Class { .. }
                | RawInst::Defn { .. } => continue,

                RawInst::Privileged(p) => {
                    let val = match p {
                        PrivInst::UseLocal { var } => {
                            let (ss, _, ty) = locals.get(&var.group()).unwrap();

                            builder.ins().stack_load(*ty, *ss, 0)
                        }

                        PrivInst::RefVal { val } => {
                            let value = store.get(val).unwrap().clone();

                            match value {
                                Value::Object { .. } => todo!(),
                                Value::Module { .. } => todo!(),
                                Value::String(_) => todo!(),
                                Value::Integer(_) => todo!(),
                                Value::Dict { .. } => todo!(),

                                Value::Function { name, class, .. } => {
                                    if !self.fids.contains_key(&val) {
                                        unimplemented!("{:?}", val);
                                    } else if !f_refs.contains_key(&val) {
                                        let fid = self.fids[&val];
                                        let fref = self
                                            .object_module
                                            .declare_func_in_func(fid, builder.func);

                                        f_refs.insert(val, fref);
                                    }
                                }

                                Value::Class { name, properties } => todo!(),
                            };

                            continue;
                        }

                        PrivInst::CallVal { val } => todo!(),

                        PrivInst::IntoMemberPointer { value } => {
                            continue;
                        }

                        PrivInst::AccessMemberPointer {
                            value,
                            offset: logical_offset,
                        } => {
                            let base_ptr = values[&value];

                            let tcx = self.host.tcx();
                            let tcx = tcx.borrow();

                            let ptr = if let RawInst::Const(Const::Int(n)) =
                                code.inst()[logical_offset].op
                            {
                                let (layout, offsets) = if let PythonType::Tuple { members } = tcx
                                    .get(code.inst()[value].attrs.type_id.unwrap())
                                    .unwrap()
                                    .as_python_type()
                                {
                                    let members = members.clone().unwrap_or_default();
                                    montyc_hlir::typing::calculate_layout(
                                        members.iter().map(|tid| tcx.layout_of(*tid)),
                                    )
                                } else {
                                    todo!();
                                };

                                let n = if n < 0 { (-n) as usize } else { n as usize };

                                let byte_offset = offsets[n];

                                builder.ins().load(
                                    ir::types::I64,
                                    MemFlags::new(),
                                    base_ptr,
                                    byte_offset,
                                )
                            } else {
                                todo!()
                            };

                            values.insert(inst.value, ptr);

                            continue;
                        }
                    };

                    values.insert(inst.value, val);
                }

                RawInst::Call {
                    callable,
                    arguments,
                } => {
                    let f_val = match code.inst()[callable].op {
                        RawInst::Privileged(PrivInst::RefVal { val }) => val,
                        _ => unimplemented!(),
                    };

                    let mut args = arguments.iter().map(|arg| values[arg]).collect::<Vec<_>>();
                    let f_ref = *f_refs.get(&f_val).unwrap();

                    {
                        let dfg = &builder.func.dfg;
                        let ex_func = &dfg.ext_funcs[f_ref];
                        let ex_func_sig = &dfg.signatures[ex_func.signature];

                        if let Some(param) = ex_func_sig.params.get(0) {
                            if param.value_type == ir::types::R64 {
                                args[0] = builder.ins().raw_bitcast(ir::types::R64, args[0]);
                            }
                        }
                    }

                    let f_inst = builder.ins().call(f_ref, &args);

                    match builder.inst_results(f_inst) {
                        [res] => {
                            values.insert(inst.value, *res);
                        }
                        _ => (),
                    };
                }

                RawInst::SetVar { variable, value } => {
                    let (ss, _, _) = locals.get(&variable.group()).unwrap();

                    let ptr = Pointer::stack_slot(*ss);
                    let val = values[&value];

                    ptr.store(val, 0, &mut builder);
                }

                RawInst::UseVar { .. } => unreachable!(),
                RawInst::GetDunder { .. } => unreachable!(),

                RawInst::GetAttribute { object, name } => todo!(),

                RawInst::SetAttribute {
                    object,
                    name,
                    value,
                } => todo!(),

                RawInst::SetDunder {
                    object,
                    dunder,
                    value,
                } => todo!(),

                RawInst::Const(cst) => {
                    let val = match cst {
                        montyc_hlir::Const::Int(i) => builder.ins().iconst(ir::types::I64, i),
                        montyc_hlir::Const::Float(f) => builder.ins().f64const(f),
                        montyc_hlir::Const::Bool(i) => builder.ins().bconst(ir::types::B64, i),
                        montyc_hlir::Const::String(_) => todo!(),
                        montyc_hlir::Const::None => todo!(),
                        montyc_hlir::Const::Ellipsis => todo!(),
                    };

                    values.insert(inst.value, val);
                }

                RawInst::Tuple(elements) => {
                    let tcx = self.host.tcx();
                    let tcx = tcx.borrow();

                    let (layout, offsets) = if let PythonType::Tuple { members } = tcx
                        .get(code.inst()[inst.value].attrs.type_id.unwrap())
                        .unwrap()
                        .as_python_type()
                    {
                        let members = members.clone().unwrap_or_default();
                        montyc_hlir::typing::calculate_layout(
                            members.iter().map(|tid| tcx.layout_of(*tid)),
                        )
                    } else {
                        todo!();
                    };

                    assert_ne!(
                        0,
                        layout.size(),
                        "{:?}",
                        tcx.get(inst.attrs.type_id.unwrap())
                            .unwrap()
                            .as_python_type()
                    );

                    let size = builder.ins().iconst(
                        ir::types::I64,
                        <_ as TryInto<i64>>::try_into(layout.size()).unwrap(),
                    );

                    let malloc_inst = builder.ins().call(libc_malloc, &[size]);

                    let addr = patma!(*addr, [addr] in builder.inst_results(malloc_inst)).unwrap();

                    for (ix, elem) in elements.iter().enumerate() {
                        let offset = offsets[ix];

                        builder.ins().store(MemFlags::new(), values[elem], addr, offset);
                    }

                    values.insert(inst.value, addr);
                }

                RawInst::PhiRecv => {
                    block = blocks[&inst.value];

                    if !builder.is_filled() {
                        builder.ins().jump(block, &[]);
                    }

                    let arg = match builder.block_params(blocks[&inst.value]) {
                        [arg] => *arg,
                        _ => unreachable!(),
                    };

                    values.insert(inst.value, arg);
                }

                RawInst::JumpTarget => {
                    block = blocks[&inst.value];

                    if !builder.is_filled() {
                        builder.ins().jump(block, &[]);
                    }
                }

                RawInst::Undefined => {
                    todo!();
                }

                RawInst::If {
                    test,
                    truthy,
                    falsey,
                } => {
                    assert_eq!(code.inst()[test].attrs.type_id, Some(TypingContext::Bool));

                    let test = values[&test];

                    let fx = match (truthy, falsey) {
                        (None, None) => continue,
                        (None, Some(ix)) => {
                            builder.ins().brz(test, blocks[&ix], &[]);
                            inst.value + 1
                        }

                        (Some(ix), None) => {
                            builder.ins().brnz(test, blocks[&ix], &[]);
                            inst.value + 1
                        }

                        (Some(tx), Some(fx)) => {
                            builder.ins().brnz(test, blocks[&tx], &[]);
                            fx
                        }
                    };

                    builder.ins().jump(blocks[&fx], &[]);
                }

                RawInst::Br { to } => {
                    builder.ins().jump(blocks[&to], &[]);
                }

                RawInst::PhiJump { recv, value } => {
                    let ty = code.inst()[value].attrs.type_id.unwrap();
                    let ty = self.cg_module.scalar_type_of(&ty);

                    if builder.block_params(blocks[&recv]).is_empty() {
                        builder.append_block_param(blocks[&recv], ty);
                    }

                    let input = values[&value];

                    builder.ins().jump(blocks[&recv], &[input]);
                }

                RawInst::Return { value } => {
                    let rval = *values.get(&value).unwrap();

                    builder.ins().return_(&[rval]);
                }
            };
        }
    }
}
