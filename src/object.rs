use crate::{PhpArray, ast::ClassDecl};
use std::{cell::RefCell, rc::Rc};

/// A PHP object handle. Copying a Value shares this instance, including properties.
pub struct PhpObject {
    pub(crate) class: Rc<ClassDecl>,
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
