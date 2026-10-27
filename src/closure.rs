use crate::ast::FunctionDecl;
use std::rc::Rc;

/// An opaque first-class function callable. Clones of its `Rc` share identity.
#[derive(Debug)]
pub struct Closure {
    pub(crate) name: Vec<u8>,
    pub(crate) function: Option<Rc<FunctionDecl>>,
    pub(crate) object_id: usize,
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Closure {
    pub(crate) fn same_function(&self, other: &Self) -> bool {
        match (&self.function, &other.function) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => self.name == other.name,
            _ => false,
        }
    }
}
