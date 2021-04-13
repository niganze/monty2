use std::{
    cell::RefCell,
    collections::HashMap,
    fs::DirEntry,
    hash::Hash,
    path::{Path, PathBuf},
    rc::Rc,
};

use log::*;

use crate::{
    ast::{
        atom::Atom,
        class,
        import::{Import, ImportDecl},
        module::Module,
        primary::Primary,
        stmt::Statement,
        AstObject, Spanned,
    },
    func::Function,
    parser::{Parseable, SpanRef},
    scope::{downcast_ref, LocalScope, OpaqueScope, Scope, ScopedObject},
    typing::{LocalTypeId, TypeMap},
    CompilerOptions,
};

fn shorten(path: &Path) -> String {
    let mut c = path
        .components()
        .rev()
        .take(2)
        .map(|c| format!("{}", c.as_os_str().to_string_lossy()))
        .collect::<Vec<_>>();

    c.reverse();
    c.join("/")
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, derive_more::From)]
pub struct ModuleRef(PathBuf);

/// Used to track global compilation state per-compilation.
#[derive(Debug)]
pub struct GlobalContext {
    pub modules: HashMap<ModuleRef, ModuleContext>,
    pub functions: Vec<Function>,
    pub span_ref: Rc<RefCell<SpanRef>>,
    pub type_map: RefCell<TypeMap>,
    pub builtins: HashMap<LocalTypeId, Rc<dyn AstObject>>,
    pub libstd: PathBuf,
}

impl From<CompilerOptions> for GlobalContext {
    fn from(opts: CompilerOptions) -> Self {
        debug!("Bootstrapping with {:?}", opts);

        let CompilerOptions { libstd, input } = opts;

        let libstd = match libstd.canonicalize() {
            Ok(path) => path,
            Err(why) => {
                error!("Failed to canonicalize stdlib path! why={:?}", why);
                unreachable!();
            }
        };

        info!("libstd path is set to => {:?}", libstd);

        let mut ctx = Self::default();

        ctx.libstd = libstd.clone();

        // pre-emptively load in core modules i.e. builtins, ctypes, and typing.

        ctx.preload_module(libstd.join("builtins.py"), |ctx, mref| {
            // The "builtins.py" module currently stubs and forward declares the compiler builtin types.
            //
            // This is necessary as most of the types are actually magical and the compiler decides what
            // implementation the user gets for any given usage i.e. a "str" in a C FFI function is a CString
            // but passing around and reading from a "str" will probably use a Copy-On-Write string slice.

            let module_context = ctx
                .modules
                .get(&mref)
                .cloned()
                .expect("failed to get pre-loaded module.");

            for item in module_context.scope.iter() {
                let object_original = item.object.clone();
                let object_unspanned = item.object.unspanned();

                if let Some(Statement::Class(class_def)) = downcast_ref(object_unspanned.as_ref()) {
                    let dec_name = match class_def.decorator_list.as_slice() {
                        [] => continue,
                        [dec] => dec.reveal(&module_context.source).unwrap(),
                        _ => panic!("Multiple decorators are not supported."),
                    };

                    assert_eq!(
                        dec_name, "@extern",
                        "only `@extern` (opaque type decorators) are supported."
                    );

                    let klass_name = class_def.name.reveal(&module_context.source).unwrap();

                    match klass_name {
                        "int" => ctx.builtins.insert(TypeMap::INTEGER, object_original),
                        "float" => ctx.builtins.insert(TypeMap::FLOAT, object_original),
                        "str" => ctx.builtins.insert(TypeMap::STRING, object_original),
                        "bool" => ctx.builtins.insert(TypeMap::BOOL, object_original),
                        st => panic!("unknown builtin {:?}", st),
                    };

                    trace!(
                        "\tAssociated builtin with class definition! {:?}",
                        klass_name
                    );
                } else if let Some(Statement::Import(import)) =
                    downcast_ref(object_unspanned.as_ref())
                {
                    let this = Rc::new(import.clone());

                    let source = module_context.source.as_ref();

                    for decl in this.decls() {
                        ctx.import_module(decl, source);
                    }
                }

                let local = item.make_local_context(mref.clone(), ctx);

                item.typecheck(local);
            }
        });

        ctx
    }
}

impl Default for GlobalContext {
    fn default() -> Self {
        Self {
            modules: HashMap::new(),
            functions: Vec::new(),
            span_ref: Default::default(),
            type_map: RefCell::new(TypeMap::new()),
            builtins: HashMap::new(),
            libstd: PathBuf::default(),
        }
    }
}

impl GlobalContext {
    fn preload_module(&mut self, path: impl AsRef<Path>, f: impl Fn(&mut Self, ModuleRef)) {
        let path = path.as_ref();

        debug!("Preloading module ({:?})", shorten(path));

        let source = match std::fs::read_to_string(&path) {
            Ok(st) => st,
            Err(why) => {
                error!("Failed to read module contents! why={:?}", why);
                unreachable!();
            }
        };

        let module = self.parse_and_register_module(source, path);

        f(self, module);

        debug!("Finished preloading module ({:?})", shorten(path));
    }

    fn resolve_import_to_path(&self, qualname: Vec<&str>) -> Option<PathBuf> {
        fn search(curdir: &Path, expected: &str) -> Option<PathBuf> {
            curdir
                .read_dir()
                .ok()?
                .filter_map(|maybe_entry| {
                    let entry = maybe_entry.ok()?;
                    let file_type = entry.file_type().ok()?;
                    let path = entry.path();

                    let stem = path.file_stem()?.to_string_lossy();
                    let wellformed_stem = !stem.contains(".");

                    let ok = if file_type.is_file() {
                        let has_py_ext = path.extension()?.to_string_lossy() == "py";

                        has_py_ext && wellformed_stem && (stem == expected)
                    } else if file_type.is_dir() {
                        stem == expected
                    } else {
                        unreachable!();
                    };

                    ok.then_some(path)
                })
                .next()
        }

        if matches!(qualname.as_slice(), ["__monty"]) {
            return Some("__monty".into());
        }

        let paths_to_inspect: &[PathBuf] = &[PathBuf::from("."), self.libstd.clone()] as &[_];

        'outer: for path in paths_to_inspect.iter() {
            let mut root = path.to_owned();

            for (idx, part) in qualname.iter().enumerate() {
                let final_path = match search(&root, part) {
                    Some(p) => p,
                    None => continue 'outer,
                };

                root = final_path.clone();
            }

            return Some(root);
        }

        None
    }

    pub fn import_module(&mut self, decl: ImportDecl, source: &str) -> Option<Vec<ModuleRef>> {
        let qualnames = match decl.parent.as_ref() {
            Import::Names(_) => vec![decl.name.inner.components()],
            Import::From { module, names, .. } => {
                let base = module.inner.components();
                let mut leaves = Vec::with_capacity(names.len());

                for name in names {
                    leaves.push(base.clone());
                    leaves.last_mut().unwrap().extend(name.inner.components());
                }

                leaves
            }
        };

        let mut modules = Vec::with_capacity(qualnames.len());

        for qualname in &qualnames {
            let qualname: Vec<&str> = qualname
                .into_iter()
                .map(|atom| match atom {
                    Atom::Name(n) => self.span_ref.borrow().resolve_ref(*n, source).unwrap(),
                    _ => unreachable!(),
                })
                .collect();

            let path = {
                let module_ref = ModuleRef(self.resolve_import_to_path(qualname)?);

                if self.modules.contains_key(&module_ref) {
                    modules.push(module_ref);
                    continue;
                } else {
                    module_ref.0
                }
            };

            trace!("Importing module ({:?})", shorten(&path));

            let source = match std::fs::read_to_string(&path) {
                Ok(st) => st,
                Err(why) => {
                    error!("Failed to read module contents! why={:?}", why);
                    unreachable!();
                }
            };

            modules.push(self.parse_and_register_module(source, path));
        }

        Some(modules)
    }

    pub fn parse<T, S>(&self, s: S) -> T
    where
        S: AsRef<str>,
        T: Parseable + Clone,
    {
        let Spanned { inner, .. }: Spanned<T> = (s, self.span_ref.clone()).into();
        inner
    }

    pub fn parse_and_register_module<S, P>(&mut self, source: S, path: P) -> ModuleRef
    where
        S: AsRef<str>,
        P: Into<PathBuf>,
    {
        let module: Module = self.parse(&source);
        self.register_module(module, path.into(), source.as_ref().to_string())
    }

    pub fn walk(
        &self,
        module_ref: ModuleRef,
    ) -> impl Iterator<Item = (Rc<dyn AstObject>, LocalContext)> {
        let module_context = self.modules.get(&module_ref).unwrap();
        let mut it = module_context.scope.iter();

        std::iter::from_fn(move || {
            let scoped = it.next()?;

            let object = scoped.object.unspanned();
            let ctx = scoped.make_local_context(module_ref.clone(), self);

            Some((object, ctx))
        })
    }

    pub fn register_module(&mut self, module: Module, path: PathBuf, source: String) -> ModuleRef {
        let module = Rc::new(module);
        let key = ModuleRef::from(path.clone());

        let source = source.into_boxed_str().into();

        let scope = OpaqueScope::from(module.clone() as Rc<dyn AstObject>);
        let scope = Rc::new(scope) as Rc<dyn Scope>;

        if let Some(previous) = self.modules.insert(
            key.clone(),
            ModuleContext {
                module,
                path,
                scope,
                source,
            },
        ) {
            panic!(
                "Overwrote previously registered module {:?} -> {:?}",
                key, previous
            );
        }

        key
    }
}

#[derive(Debug, Clone)]
pub struct ModuleContext {
    pub path: PathBuf,
    pub module: Rc<Module>,
    pub scope: Rc<dyn Scope>,
    pub source: Rc<str>,
}

impl ModuleContext {
    pub fn make_local_context<'a>(&'a self, global_context: &'a GlobalContext) -> LocalContext<'a> {
        LocalContext {
            global_context,
            module_ref: ModuleRef::from(self.path.clone()),
            scope: self.scope.as_ref(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LocalContext<'a> {
    pub global_context: &'a GlobalContext,
    pub module_ref: ModuleRef,
    pub scope: &'a dyn Scope,
}
