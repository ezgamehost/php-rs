use crate::{
    PhpArray,
    ast::{ClassDecl, FunctionDecl, PropertyDecl},
};
use std::{cell::RefCell, rc::Rc};

pub(crate) struct PropertyName<'a> {
    pub scope: Option<&'a [u8]>,
    pub name: &'a [u8],
}

/// Decode property-table keys, including the extra source segment in anonymous
/// class names. Array-to-object casts may also supply malformed encoded keys.
pub(crate) fn property_name(raw: &[u8]) -> Result<PropertyName<'_>, &'static str> {
    if raw.first() != Some(&0) {
        return Ok(PropertyName {
            scope: None,
            name: raw,
        });
    }
    if raw.len() < 3 || raw[1] == 0 {
        return Err("Illegal member variable name");
    }
    let split = raw[1..]
        .iter()
        .position(|b| *b == 0)
        .map(|i| i + 1)
        .filter(|i| *i < raw.len() - 1)
        .ok_or("Corrupt member variable name")?;
    let mut name = &raw[split + 1..];
    if let Some(end) = name.iter().position(|b| *b == 0) {
        name = &name[end + 1..];
    }
    Ok(PropertyName {
        scope: Some(&raw[1..split]),
        name,
    })
}

/// A PHP object handle. Copying a Value shares this instance, including properties.
pub struct PhpObject {
    pub(crate) property_cursor: RefCell<PropertyCursor>,
    pub(crate) property_iterators: RefCell<Vec<std::rc::Weak<RefCell<PropertyCursor>>>>,
    pub(crate) reflection: RefCell<Option<crate::runtime::reflection::ReflectionData>>,
    pub(crate) throwable: RefCell<Option<ThrowableData>>,
    pub(crate) class: Rc<Class>,
    pub(crate) class_name: String,
    pub(crate) properties: RefCell<PhpArray>,
    pub(crate) unset_properties: RefCell<std::collections::HashSet<Vec<u8>>>,
    pub(crate) reinitializable: RefCell<std::collections::HashSet<Vec<u8>>>,
    pub(crate) object_id: usize,
    // Release the handle after fields that may own other PHP objects.
    pub(crate) _handle: ObjectHandle,
}

#[derive(Default)]
pub(crate) struct PropertyCursor {
    pub keys: Vec<crate::ArrayKey>,
    pub position: usize,
}

impl Clone for PropertyCursor {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            position: if self.position < self.keys.len() {
                self.position
            } else {
                0
            },
        }
    }
}

impl PropertyCursor {
    pub(crate) fn remove(&mut self, key: &crate::ArrayKey) {
        if let Some(index) = self.keys.iter().position(|existing| existing == key) {
            self.keys.remove(index);
            if self.position > index {
                self.position -= 1;
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ObjectStore {
    next: usize,
    free: Vec<usize>,
}

#[derive(Debug)]
pub(crate) struct ObjectHandle {
    pub id: usize,
    store: Rc<RefCell<ObjectStore>>,
}

impl ObjectHandle {
    pub fn allocate(store: &Rc<RefCell<ObjectStore>>) -> Self {
        let id = {
            let mut store = store.borrow_mut();
            if let Some(id) = store.free.pop() {
                id
            } else {
                store.next += 1;
                store.next
            }
        };
        Self {
            id,
            store: store.clone(),
        }
    }
}

impl Drop for ObjectHandle {
    fn drop(&mut self) {
        self.store.borrow_mut().free.push(self.id);
    }
}

#[derive(Clone)]
pub(crate) struct ThrowableData {
    pub trace: Rc<PhpArray>,
    pub message: Vec<u8>,
    pub code: i64,
    pub file: Vec<u8>,
    pub line: i64,
    pub severity: i64,
    pub previous: Option<Rc<PhpObject>>,
}
impl Eq for PhpObject {}
impl PhpObject {
    pub(crate) fn remove_property(&self, key: &crate::ArrayKey) {
        self.properties.borrow_mut().remove(key);
        // Declared slots remain in the property table as uninitialized indirect
        // entries. Dynamic slots disappear, moving later cursor positions back.
        if self
            .class
            .properties
            .iter()
            .any(|property| *key == crate::ArrayKey::String(property.storage_name()))
        {
            return;
        }
        self.property_cursor.borrow_mut().remove(key);
        for iterator in self
            .property_iterators
            .borrow()
            .iter()
            .filter_map(std::rc::Weak::upgrade)
        {
            iterator.borrow_mut().remove(key);
        }
    }

    pub(crate) fn sync_property_cursor(&self, cursor: &mut PropertyCursor) {
        let properties = self.properties.borrow();
        let mut keys: Vec<_> = self
            .class
            .properties
            .iter()
            .map(|property| crate::ArrayKey::String(property.storage_name()))
            .collect();
        for (key, _) in properties.entries() {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
        if cursor.keys != keys {
            cursor.position = cursor
                .keys
                .iter()
                .take(cursor.position)
                .filter(|key| keys.contains(key))
                .count();
            cursor.keys = keys;
        }
    }

    pub(crate) fn hides_properties(&self) -> bool {
        self.class.name == b"SensitiveParameterValue"
    }

    pub(crate) fn export_properties(&self) -> PhpArray {
        if self.hides_properties() {
            PhpArray::default()
        } else {
            self.snapshot()
        }
    }

    pub(crate) fn snapshot(&self) -> PhpArray {
        let properties = self.properties.borrow();
        let mut result = PhpArray::default();
        for property in &self.class.properties {
            let key = crate::ArrayKey::String(property.storage_name());
            if let Some(value) = properties.get(&key) {
                result.insert(key, value.copy_array_entry());
            }
        }
        for (key, value) in properties.entries() {
            if result.get(key).is_none() {
                result.insert(key.clone(), value.copy_array_entry());
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
    pub aliases: RefCell<Vec<Vec<u8>>>,
    pub interfaces: Vec<Rc<Class>>,
    pub declaration: Rc<ClassDecl>,
    pub name: Vec<u8>,
    pub parent: Option<Rc<Class>>,
    pub properties: Vec<PropertyDecl>,
    pub methods: Vec<Rc<FunctionDecl>>,
    pub allow_dynamic: bool,
    pub forbid_dynamic: bool,
    pub defaults: RefCell<Option<PhpArray>>,
    pub static_properties: Vec<PropertyDecl>,
    pub static_values: RefCell<std::collections::HashMap<Vec<u8>, Rc<RefCell<crate::Value>>>>,
    pub constant_values: RefCell<std::collections::HashMap<Vec<u8>, crate::Value>>,
    pub evaluating_constants: RefCell<std::collections::HashSet<Vec<u8>>>,
    pub initialized: std::cell::Cell<bool>,
    pub initializing: std::cell::Cell<bool>,
}
impl Class {
    pub(crate) fn display_name(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(self.name.split(|b| *b == 0).next().unwrap())
    }
    pub fn is_a(&self, name: &[u8]) -> bool {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        self.name.eq_ignore_ascii_case(name)
            || self
                .aliases
                .borrow()
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(name))
            || self.parent.as_ref().is_some_and(|p| p.is_a(name))
            || self.interfaces.iter().any(|i| i.is_a(name))
    }
    pub fn constant_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        if self.declaration.constants.iter().any(|c| c.name == name) {
            Some(self.clone())
        } else {
            self.parent
                .as_ref()
                .and_then(|p| p.inheritable_constant_owner(name))
                .or_else(|| self.interfaces.iter().find_map(|i| i.constant_owner(name)))
        }
    }
    pub fn inheritable_constant_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        self.constant_owner(name).filter(|owner| {
            owner.declaration.constants.iter().any(|constant| {
                constant.name == name && constant.visibility != crate::ast::Visibility::Private
            })
        })
    }
    pub fn constant_names(&self) -> Vec<Vec<u8>> {
        let mut names: Vec<_> = self
            .declaration
            .constants
            .iter()
            .map(|c| c.name.clone())
            .collect();
        for ancestor in self.parent.iter().chain(&self.interfaces) {
            for name in ancestor.constant_names() {
                if !names.contains(&name) && ancestor.inheritable_constant_owner(&name).is_some() {
                    names.push(name);
                }
            }
        }
        names
    }
    pub fn static_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        if self
            .declaration
            .static_properties
            .iter()
            .any(|p| p.name == name)
        {
            Some(self.clone())
        } else if self.declaration.properties.iter().any(|p| p.name == name) {
            None
        } else {
            self.parent.as_ref().and_then(|p| p.static_owner(name))
        }
    }
    pub fn property_owner(self: &Rc<Self>, name: &[u8]) -> Option<Rc<Self>> {
        if self.declaration.properties.iter().any(|p| p.name == name) {
            Some(self.clone())
        } else {
            self.parent
                .as_ref()
                .and_then(|p| p.property_owner(name))
                .filter(|owner| {
                    owner
                        .declaration
                        .properties
                        .iter()
                        .any(|p| p.name == name && p.visibility != crate::ast::Visibility::Private)
                })
        }
    }
    pub fn property_slot_owner(self: &Rc<Self>, property: &PropertyDecl) -> Rc<Self> {
        if property.declaring_class == self.name {
            self.clone()
        } else {
            self.parent
                .as_ref()
                .expect("property ancestor")
                .property_slot_owner(property)
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
