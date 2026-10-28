use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

/// A PHP byte-string value. Copies have independent value slots sharing a
/// buffer; ordinary mutation separates the buffer before changing its bytes.
#[derive(PartialEq, Eq)]
pub struct PhpString(RefCell<Rc<RefCell<Vec<u8>>>>);

/// Retains storage while a PHP conversion may change the destination value.
pub(crate) struct StringBuffer(Rc<RefCell<Vec<u8>>>);

impl Clone for PhpString {
    fn clone(&self) -> Self {
        Self(RefCell::new(self.0.borrow().clone()))
    }
}
impl From<Vec<u8>> for PhpString {
    fn from(bytes: Vec<u8>) -> Self {
        Self(RefCell::new(Rc::new(RefCell::new(bytes))))
    }
}
impl std::fmt::Debug for PhpString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.borrow().borrow().fmt(f)
    }
}
impl PhpString {
    /// Return an owned snapshot, without holding a borrow across PHP execution.
    pub fn bytes(&self) -> Vec<u8> {
        self.0.borrow().borrow().clone()
    }
    pub fn to_vec(&self) -> Vec<u8> {
        self.bytes()
    }
    pub fn len(&self) -> usize {
        self.0.borrow().borrow().len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.borrow().borrow().is_empty()
    }
    pub(crate) fn separate(&mut self) {
        if Rc::strong_count(self.0.get_mut()) != 1 {
            *self = self.to_vec().into();
        }
    }
    pub(crate) fn make_mut(&mut self) -> RefMut<'_, Vec<u8>> {
        self.separate();
        self.0.get_mut().borrow_mut()
    }
    pub(crate) fn retain(&self) -> StringBuffer {
        StringBuffer(self.0.borrow().clone())
    }
    pub(crate) fn replace_buffer(&self, buffer: &StringBuffer) {
        *self.0.borrow_mut() = buffer.0.clone();
    }
}
impl StringBuffer {
    pub(crate) fn retained(&self) -> bool {
        Rc::strong_count(&self.0) > 1
    }
    pub(crate) fn pending_bytes(&self) -> RefMut<'_, Vec<u8>> {
        self.0.borrow_mut()
    }
}
impl FromIterator<u8> for PhpString {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}
