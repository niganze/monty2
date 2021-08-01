//! An AST-based interpreter runtime for const evaluation.
#![allow(warnings)]

mod exception;
pub(crate) mod object;
pub mod runtime;

pub(self) type HashKeyT = u64;

/// The interpreters Result type.
pub type PyResult<T> = Result<T, exception::PyException>;

use std::fmt::Debug;

use montyc_core::{ModuleRef, SpanRef};
use montyc_parser::ast::ImportDecl;

pub use {object::PyDictRaw, runtime::Runtime};

pub use object::alloc::ObjAllocId;

use crate::ModuleObject;

/// A trait to be implemented by the owner of a runtime.
pub trait HostGlue {
    /// convert a string to a span ref.
    fn str_to_spanref(&self, name: &str) -> SpanRef;

    /// convert a span ref to a string.
    fn spanref_to_str(&self, sref: SpanRef) -> &str;

    /// trigger the importing mechansim to import the given module.
    fn import_module(&self, decl: ImportDecl) -> Vec<(ModuleRef, SpanRef)>;

    /// run the given function with the given module object.
    fn with_module(
        &self,
        mref: ModuleRef,
        f: &mut dyn FnMut(&ModuleObject) -> PyResult<()>,
    ) -> PyResult<()>;

    /// run the given function with the given module object mut ref.
    fn with_module_mut(
        &self,
        mref: ModuleRef,
        f: &mut dyn FnMut(&mut ModuleObject) -> PyResult<()>,
    ) -> PyResult<()>;
}

impl<'a> Debug for &'a dyn HostGlue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostGlue").finish()
    }
}
