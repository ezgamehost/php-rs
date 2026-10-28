use crate::ast::FunctionDecl;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

/// An opaque PHP closure or first-class callable. Clones of its `Rc` share identity.
#[derive(Debug)]
pub struct Closure {
    pub(crate) magic_name: Option<Vec<u8>>,
    pub(crate) name: Vec<u8>,
    pub(crate) this: Option<Rc<crate::PhpObject>>,
    pub(crate) context: Option<crate::object::ClassContext>,
    pub(crate) function: Option<Rc<FunctionDecl>>,
    pub(crate) object_id: usize,
    pub(crate) environment: Option<Environment>,
    pub(crate) _handle: crate::object::ObjectHandle,
}

#[derive(Debug)]
pub(crate) struct Environment {
    pub captures: Vec<(Vec<u8>, CapturedValue)>,
    pub statics: RefCell<HashMap<Vec<u8>, Rc<RefCell<crate::Value>>>>,
}

#[derive(Debug)]
pub(crate) enum CapturedValue {
    Value(crate::Value),
    Reference(Rc<RefCell<crate::Value>>),
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Closure {
    pub(crate) fn same_function(&self, other: &Self) -> bool {
        if self.environment.is_some() || other.environment.is_some() {
            return std::ptr::eq(self, other);
        }
        if self.this != other.this
            || self.context != other.context
            || self.magic_name != other.magic_name
        {
            return false;
        }
        match (&self.function, &other.function) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => self.name == other.name,
            _ => false,
        }
    }
}
