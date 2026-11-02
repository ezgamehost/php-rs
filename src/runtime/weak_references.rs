// Altered Rust adaptation of PHP 8.5.0's Zend/zend_weakrefs.c.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::NativeMethod,
    object::{Class, ObjectStore},
};
use std::rc::Weak;

type Cache = RefCell<HashMap<usize, Weak<PhpObject>>>;

pub(crate) struct WeakReferenceData {
    target: WeakTarget,
    key: usize,
    cache: Weak<Cache>,
}

impl WeakReferenceData {
    pub(super) fn get(&self) -> Option<Value> {
        self.target.get()
    }
}

impl Drop for WeakReferenceData {
    fn drop(&mut self) {
        if let Some(cache) = self.cache.upgrade() {
            cache.borrow_mut().remove(&self.key);
        }
    }
}

enum WeakTarget {
    Object {
        identity: Weak<()>,
        store: Weak<RefCell<ObjectStore>>,
        id: usize,
    },
    Closure(Weak<crate::Closure>),
}

impl WeakTarget {
    fn get(&self) -> Option<Value> {
        match self {
            Self::Object {
                identity,
                store,
                id,
            } => {
                let identity = identity.upgrade()?;
                let store = store.upgrade()?;
                let object = store.borrow().objects.get(id)?.upgrade()?;
                // Handles survive destructor-queue moves, but reused numeric IDs
                // must never redirect an old weak reference to a new object.
                Rc::ptr_eq(&identity, &object._handle.identity).then_some(Value::Object(object))
            }
            Self::Closure(closure) => closure.upgrade().map(Value::Closure),
        }
    }
}

impl Runtime {
    pub(super) fn builtin_weak_reference_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if name != b"weakreference" {
            return None;
        }
        let source = b"<?php final class WeakReference {
            public function __construct() {}
            public static function create(object $object): WeakReference {}
            public function get(): ?object {}
        }";
        let (mut statements, _, _) =
            parser::parse(lexer::lex(source).expect("weak reference source"))
                .expect("weak reference declaration");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        let decl = Rc::make_mut(&mut declaration);
        decl.line = 0;
        decl.end_line = 0;
        for method in &mut decl.methods {
            Rc::make_mut(method).native = Some(NativeMethod::WeakReference);
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            properties: Vec::new(),
            declaration,
            parent: None,
            interfaces: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: true,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    pub(super) fn native_weak_reference_method(&mut self, line: usize) -> Result<Value, Error> {
        let name = self.current_function.as_ref().unwrap();
        if name.ends_with(b"::__construct") {
            return Err(Error::new(
                ErrorKind::Error,
                "Direct instantiation of WeakReference is not allowed, use WeakReference::create instead",
                line,
            ));
        }
        if name.ends_with(b"::get") {
            let Some(Value::Object(object)) = self.lookup(b"this") else {
                unreachable!()
            };
            return Ok(object
                .weak_reference
                .borrow()
                .as_ref()
                .and_then(WeakReferenceData::get)
                .unwrap_or(Value::Null));
        }
        let (key, target) = match self.lookup(b"object").unwrap() {
            Value::Object(object) => (
                Rc::as_ptr(&object._handle.identity) as usize,
                WeakTarget::Object {
                    identity: Rc::downgrade(&object._handle.identity),
                    store: object._handle.store.clone(),
                    id: object.object_id,
                },
            ),
            Value::Closure(closure) => (
                Rc::as_ptr(&closure._handle.identity) as usize,
                WeakTarget::Closure(Rc::downgrade(&closure)),
            ),
            _ => unreachable!(),
        };
        let existing = self
            .weak_reference_cache
            .borrow()
            .get(&key)
            .and_then(Weak::upgrade);
        if let Some(object) = existing {
            return Ok(Value::Object(object));
        }
        let class = self.find_class(b"WeakReference").unwrap();
        let object = self.allocate_class_object(class, line)?;
        *object.weak_reference.borrow_mut() = Some(WeakReferenceData {
            target,
            key,
            cache: Rc::downgrade(&self.weak_reference_cache),
        });
        self.weak_reference_cache
            .borrow_mut()
            .insert(key, Rc::downgrade(&object));
        Ok(Value::Object(object))
    }
}
