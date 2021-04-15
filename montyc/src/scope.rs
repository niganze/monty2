use std::{
    any::{Any, TypeId},
    cell::RefCell,
    fmt,
    marker::PhantomData,
    rc::Rc,
};

use fmt::Debug;

use crate::{ast::{
        assign::Assign, atom::Atom, class::ClassDef, funcdef::FunctionDef, import::Import,
        primary::Primary, stmt::Statement, AstObject,
    }, context::{GlobalContext, LocalContext, ModuleRef}, func::Function, parser::SpanEntry, typing::{LocalTypeId, TypeMap, TypedObject}};

pub trait LookupTarget {
    fn is_named(&self, target: SpanEntry) -> bool;
    fn name(&self) -> SpanEntry;
    fn renamed_properties(&self) -> Option<ModuleRef> { None }
}

#[derive(Debug)]
pub struct Renamed {
    inner: Rc<dyn AstObject>,
    name: SpanEntry,
    mref: ModuleRef,
}

impl TypedObject for Renamed {
    fn infer_type<'a>(&self, ctx: &LocalContext<'a>) -> Option<LocalTypeId> {
        self.inner.infer_type(ctx)
    }

    fn typecheck<'a>(&self, ctx: LocalContext<'a>) {
        self.inner.typecheck(ctx)
    }
}

impl LookupTarget for Renamed {
    fn is_named(&self, target: SpanEntry) -> bool {
        self.name == target
    }

    fn renamed_properties(&self) -> Option<ModuleRef> { Some(self.mref.clone()) }

    fn name(&self) -> SpanEntry {
        self.name.clone()
    }
}

impl AstObject for Renamed {
    fn span(&self) -> Option<logos::Span> {
        self.inner.span()
    }

    fn unspanned(&self) -> Rc<dyn AstObject> {
        self.inner.unspanned()
    }

    fn walk(&self) -> Option<crate::ast::ObjectIter> {
        self.inner.walk()
    }
}

fn collect_subnodes(object: &dyn AstObject) -> Vec<Rc<dyn AstObject>> {
    let stream = match object.walk() {
        Some(it) => it,
        None => return vec![],
    };

    let mut nodes = vec![];

    stream.for_each(|object| {
        nodes.push(object.clone());

        if let Some(Statement::FnDef(_)) = downcast_ref::<Statement>(object.unspanned().as_ref()) {
            return;
        }

        for subnode in collect_subnodes(object.as_ref()) {
            nodes.push(subnode)
        }
    });

    nodes
}

pub fn downcast_ref<T: Any>(o: &dyn AstObject) -> Option<&T> {
    if o.type_id() == TypeId::of::<T>() {
        // SAFETY: This is the exact same logic present in
        //         `std::any::Any::downcast_ref` minus the
        //         'static lifetime bound on the trait.
        //
        //         If this is unsound then that one probably is too.
        unsafe { Some(&*(o as *const _ as *const T)) }
    } else {
        None
    }
}

// -- enum ScopeRoot

#[derive(Debug, Clone)]
pub enum ScopeRoot {
    AstObject(Rc<dyn AstObject>),
    Func(Rc<Function>),
    Class(Rc<crate::class::Class>),
}

// -- trait Scope

pub type ScopeIter<'object, 'iter, 'ctx> =
    Box<dyn Iterator<Item = (Rc<dyn AstObject + 'object>, LocalContext<'ctx>)> + 'iter>;

pub trait Scope: core::fmt::Debug {
    fn iter<'b>(&'b self) -> Box<(dyn Iterator<Item = ScopedObject> + 'b)>;

    fn root(&self) -> ScopeRoot;

    fn module_ref(&self) -> ModuleRef;

    fn walk<'a, 'b>(&'b self, global_context: &'a GlobalContext) -> ScopeIter<'a, 'b, 'b>;

    fn lookup_with(&self, key: &dyn Fn(&dyn AstObject) -> bool) -> Option<Rc<dyn AstObject>>;

    fn lookup_any(
        &self,
        target: SpanEntry,
        global_context: &GlobalContext,
    ) -> Vec<Rc<dyn AstObject>>;
}

// -- OpaqueScope

#[derive(Debug, Clone)]
pub struct OpaqueScope {
    pub root: ScopeRoot,
    pub module_ref: Option<ModuleRef>,
    pub nodes: Vec<Rc<dyn AstObject>>,
}

impl<T> From<LocalScope<T>> for OpaqueScope {
    fn from(scope: LocalScope<T>) -> Self {
        let LocalScope { inner, .. } = scope;

        inner
    }
}

impl From<Rc<dyn AstObject>> for OpaqueScope {
    fn from(root: Rc<dyn AstObject>) -> Self {
        let nodes = collect_subnodes(root.as_ref());

        Self {
            root: ScopeRoot::AstObject(root),
            nodes,
            module_ref: None,
        }
    }
}

impl Scope for OpaqueScope {
    fn iter<'b>(&'b self) -> Box<(dyn Iterator<Item = ScopedObject> + 'b)> {
        let it = self.nodes.iter().map(move |object| ScopedObject {
            scope: Rc::new(self.clone()),
            object: object.clone(),
        });

        Box::new(it)
    }

    fn root<'b>(&'b self) -> ScopeRoot {
        self.root.clone()
    }

    fn lookup_with(&self, key: &dyn Fn(&dyn AstObject) -> bool) -> Option<Rc<dyn AstObject>> {
        self.nodes
            .iter()
            .filter(|node| key(node.as_ref()))
            .next()
            .cloned()
    }

    fn lookup_any(
        &self,
        target: SpanEntry,
        global_context: &GlobalContext,
    ) -> Vec<Rc<dyn AstObject>> {
        log::trace!(
            "lookup: Performing generic lookup on target=({:?} -> {:?})",
            target,
            {
                let mctx = global_context.modules.get(self.module_ref.as_ref().unwrap()).unwrap();
                global_context.span_ref.borrow().resolve_ref(target, mctx.source.as_ref()).unwrap()
            }
        );

        let mut results = vec![];

        for object in self.nodes.iter().map(|o| o.unspanned()) {
            let item = object.as_ref();

            if item.is_named(target) {
                results.push(object.clone());
            }
        }

        {
            log::trace!("lookup: checking builtins for matches");

            let mctx = global_context.modules.get(self.module_ref.as_ref().unwrap()).unwrap();
            let st = global_context.span_ref.borrow().resolve_ref(target, mctx.source.as_ref()).unwrap();

            for (type_id, (object, mref)) in global_context.builtins.iter() {
                let item = object.as_ref();

                let mctx = global_context.modules.get(mref).unwrap();
                let lst = global_context.span_ref.borrow().resolve_ref(object.name(), mctx.source.as_ref()).unwrap();

                if lst == st {
                    log::trace!("lookup: renaming builtin object to={:?} from={:?}", target, object.name());

                    let renamed = Renamed { inner: object.clone(), name: target.clone(), mref: self.module_ref.clone().unwrap() };
                    let renamed = Rc::new(renamed) as Rc<dyn AstObject>;

                    results.push(renamed);
                }
            }
        }

        results
    }

    fn walk<'a, 'b>(&'b self, global_context: &'a GlobalContext) -> ScopeIter<'a, 'b, 'b> {
        let nodes: Vec<_> = self.iter().collect();
        let mut nodes = nodes.into_iter();

        let it = std::iter::from_fn(move || {
            let scoped = nodes.next()?;

            let object = scoped.object.unspanned();
            let ctx = LocalContext {
                global_context,
                module_ref: scoped.scope.module_ref(),
                scope: scoped.scope,
                this: Some(object.clone()),
            };

            Some((object, ctx))
        });

        Box::new(it)
    }

    fn module_ref(&self) -> ModuleRef {
        self.module_ref.clone().unwrap()
    }
}

// -- LocalScope

#[derive(Debug, Clone)]
pub struct LocalScope<T> {
    pub _t: PhantomData<Rc<T>>,
    pub inner: OpaqueScope,
}

impl<T> From<T> for LocalScope<T>
where
    T: AstObject,
{
    fn from(root: T) -> Self {
        let root = Rc::new(root);
        let nodes = collect_subnodes(root.as_ref());

        Self {
            inner: OpaqueScope {
                root: ScopeRoot::AstObject(root as Rc<dyn AstObject>),
                nodes,
                module_ref: None,
            },
            _t: PhantomData,
        }
    }
}

impl<T: Debug> Scope for LocalScope<T> {
    fn iter<'b>(&'b self) -> Box<(dyn Iterator<Item = ScopedObject> + 'b)> {
        self.inner.iter()
    }

    fn root<'b>(&'b self) -> ScopeRoot {
        self.inner.root.clone()
    }

    fn lookup_with(&self, key: &dyn Fn(&dyn AstObject) -> bool) -> Option<Rc<dyn AstObject>> {
        self.inner.lookup_with(key)
    }

    fn walk<'a, 'b>(&'b self, global_context: &'a GlobalContext) -> ScopeIter<'a, 'b, 'b> {
        self.inner.walk(global_context)
    }

    fn lookup_any(
        &self,
        target: SpanEntry,
        global_context: &GlobalContext,
    ) -> Vec<Rc<dyn AstObject>> {
        self.inner.lookup_any(target, global_context)
    }

    fn module_ref(&self) -> ModuleRef {
        self.inner.module_ref.clone().unwrap()
    }
}

// -- ScopeObject

pub struct ScopedObject {
    pub scope: Rc<dyn Scope>,
    pub object: Rc<dyn AstObject>,
}

impl ScopedObject {
    pub fn with_context<F, T>(&self, global_context: &GlobalContext, f: F) -> T
    where
        F: Fn(LocalContext, Rc<dyn AstObject>) -> T,
    {
        let ctx = LocalContext {
            scope: self.scope.clone(),
            this: Some(self.object.clone()),
            global_context,
            module_ref: self.scope.module_ref(),
        };

        f(ctx, self.object.clone())
    }
}

impl fmt::Debug for ScopedObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedObject")
            .field("object", &self.object)
            .field("scope", &self.scope.root())
            .finish()
    }
}