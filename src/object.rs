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
}
impl Class {
    pub fn is_a(&self, name: &[u8]) -> bool {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        self.name.eq_ignore_ascii_case(name) || self.parent.as_ref().is_some_and(|p| p.is_a(name))
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
