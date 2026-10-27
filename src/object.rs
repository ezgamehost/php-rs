use crate::{
    PhpArray,
    ast::{ClassDecl, Expr, FunctionDecl},
};
use std::{cell::RefCell, rc::Rc};

/// A PHP object handle. Copying a Value shares this instance, including properties.
pub struct PhpObject {
    pub(crate) class: Rc<Class>,
    pub(crate) class_name: String,
    pub(crate) properties: RefCell<PhpArray>,
    pub(crate) object_id: usize,
}
impl PhpObject {
    pub(crate) fn snapshot(&self) -> PhpArray {
        let properties = self.properties.borrow();
        let mut result = PhpArray::default();
        for (name, _) in &self.class.properties {
            let key = crate::ArrayKey::String(name.clone());
            if let Some(value) = properties.get(&key) {
                result.insert(key, value.clone());
            }
        }
        for (key, value) in properties.entries() {
            if result.get(key).is_none() {
                result.insert(key.clone(), value.clone());
            }
        }
        result
    }
}
impl PartialEq for PhpObject {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl std::fmt::Debug for PhpObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhpObject")
            .field("class", &self.class_name)
            .field("id", &self.object_id)
            .finish_non_exhaustive()
    }
}

/// Linked class layout. Inherited methods retain their declaring function identity.
#[derive(Debug)]
pub(crate) struct Class {
    pub declaration: Rc<ClassDecl>,
    pub name: Vec<u8>,
    pub parent: Option<Rc<Class>>,
    pub properties: Vec<(Vec<u8>, Option<Expr>)>,
    pub methods: Vec<Rc<FunctionDecl>>,
    pub allow_dynamic: bool,
    pub defaults: RefCell<Option<PhpArray>>,
    pub static_properties: Vec<(Vec<u8>, Option<Expr>)>,
    pub static_values: RefCell<std::collections::HashMap<Vec<u8>, Rc<RefCell<crate::Value>>>>,
    pub constant_values: RefCell<std::collections::HashMap<Vec<u8>, crate::Value>>,
    pub evaluating_constants: RefCell<std::collections::HashSet<Vec<u8>>>,
    pub initialized: std::cell::Cell<bool>,
    pub initializing: std::cell::Cell<bool>,
}
impl Class {
    pub fn is_a(&self, name: &[u8]) -> bool {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        self.name.eq_ignore_ascii_case(name) || self.parent.as_ref().is_some_and(|p| p.is_a(name))
    }
    pub fn constant_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        if self.declaration.constants.iter().any(|c| c.name == name) {
            Some(self.clone())
        } else {
            self.parent.as_ref().and_then(|p| p.constant_owner(name))
        }
    }
    pub fn static_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        if self
            .declaration
            .static_properties
            .iter()
            .any(|(n, _)| n == name)
        {
            Some(self.clone())
        } else {
            self.parent.as_ref().and_then(|p| p.static_owner(name))
        }
    }
    pub fn method_is_final(&self, name: &[u8]) -> bool {
        self.declaration
            .final_methods
            .iter()
            .any(|n| n.eq_ignore_ascii_case(name))
            || self
                .parent
                .as_ref()
                .is_some_and(|p| p.method_is_final(name))
    }
}

#[derive(Clone)]
pub(crate) struct ClassContext {
    pub scope: Rc<Class>,
    pub called: Rc<Class>,
}
impl PartialEq for ClassContext {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.scope, &other.scope) && Rc::ptr_eq(&self.called, &other.called)
    }
}

impl std::fmt::Debug for ClassContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClassContext")
            .field("scope", &String::from_utf8_lossy(&self.scope.name))
            .field("called", &String::from_utf8_lossy(&self.called.name))
            .finish()
    }
}
