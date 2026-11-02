use super::*;
use crate::{
    ast::{PropertyDecl, Visibility},
    object::Class,
};

fn hook(property: &PropertyDecl, get: bool) -> Option<&Rc<FunctionDecl>> {
    property
        .hooks
        .as_ref()
        .and_then(|h| if get { h.get.as_ref() } else { h.set.as_ref() })
}

fn hook_name(function: &FunctionDecl) -> &[u8] {
    let offset = function
        .name
        .windows(2)
        .position(|p| p == b"::")
        .expect("qualified hook")
        + 2;
    &function.name[offset..]
}

fn implicit_operation(property: &PropertyDecl, get: bool) -> bool {
    property.hooks.as_ref().is_none_or(|h| !h.is_virtual) && (get || !property.is_readonly)
}

fn collect_interfaces(class: &Class, interfaces: &mut Vec<Rc<Class>>) {
    for interface in &class.interfaces {
        if !interfaces.iter().any(|i| Rc::ptr_eq(i, interface)) {
            interfaces.push(interface.clone());
            collect_interfaces(interface, interfaces);
        }
    }
    if let Some(parent) = &class.parent {
        collect_interfaces(parent, interfaces);
    }
}

impl Runtime {
    pub(super) fn validate_hook_overrides(&mut self, class: &Class) -> Result<(), Error> {
        let mut interfaces = Vec::new();
        collect_interfaces(class, &mut interfaces);
        for property in &class.declaration.properties {
            for get in [true, false] {
                let Some(function) = hook(property, get) else {
                    continue;
                };
                if !method_visibility::declaring_name(function).eq_ignore_ascii_case(&class.name)
                    || !function
                        .attributes
                        .iter()
                        .any(|a| a.name.eq_ignore_ascii_case(b"Override"))
                {
                    continue;
                }
                let matches = |p: &PropertyDecl| {
                    p.name == property.name
                        && p.visibility != Visibility::Private
                        && (hook(p, get).is_some() || implicit_operation(p, get))
                };
                if class
                    .parent
                    .as_ref()
                    .is_some_and(|p| p.properties.iter().any(matches))
                    || interfaces.iter().any(|i| i.properties.iter().any(matches))
                {
                    continue;
                }
                return Err(self.compile_fatal_at(
                    &format!(
                        "{}() has #[\\Override] attribute, but no matching parent method exists",
                        String::from_utf8_lossy(&function.name)
                    ),
                    &function.source,
                    function.line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_property_contracts(&mut self, class: &Class) -> Result<(), Error> {
        self.validate_setter_types(class)?;
        let mut missing: Vec<Rc<FunctionDecl>> = class
            .properties
            .iter()
            .flat_map(|p| {
                p.hooks
                    .iter()
                    .flat_map(|h| h.get.iter().chain(h.set.iter()))
            })
            .filter(|f| f.is_abstract)
            .cloned()
            .collect();
        let mut interfaces = Vec::new();
        collect_interfaces(class, &mut interfaces);
        for interface in interfaces {
            for required in &interface.properties {
                let actual = class.properties.iter().find(|p| p.name == required.name);
                if let Some(actual) = actual {
                    let bad_access = if actual.visibility != Visibility::Public {
                        Some("Access")
                    } else if hook(required, false).is_some()
                        && actual.set_visibility != Visibility::Public
                    {
                        Some("Set access")
                    } else {
                        None
                    };
                    if let Some(label) = bad_access {
                        return Err(self.compile_fatal_at(
                            &format!(
                                "{label} level of {}::${} must be omitted (as in class {})",
                                class.display_name(),
                                String::from_utf8_lossy(&actual.name),
                                interface.display_name()
                            ),
                            &class.declaration.source,
                            class.declaration.line,
                        ));
                    }
                }
                for get in [true, false] {
                    let Some(required_hook) = hook(required, get) else {
                        continue;
                    };
                    if let Some(actual) = actual {
                        if let Some(actual_hook) = hook(actual, get) {
                            if !self.compatible_method(actual_hook, required_hook, class) {
                                return Err(self.hook_signature_error(actual_hook, required_hook));
                            }
                            // Its own abstract hook was already collected above.
                            continue;
                        }
                        if implicit_operation(actual, get) {
                            continue;
                        }
                    }
                    if !missing
                        .iter()
                        .any(|f| hook_name(f) == hook_name(required_hook))
                    {
                        missing.push(required_hook.clone());
                    }
                }
                if let Some(actual) = actual {
                    let owner = self.find_class(&actual.declaring_class);
                    self.validate_hook_property_type(
                        actual,
                        owner.as_deref().unwrap_or(class),
                        required,
                        &interface,
                    )?;
                }
            }
        }
        if let Some(parent) = &class.parent {
            for property in &class.declaration.properties {
                let Some(inherited) = parent
                    .properties
                    .iter()
                    .find(|p| p.name == property.name && p.visibility != Visibility::Private)
                else {
                    continue;
                };
                for get in [true, false] {
                    if let (Some(actual), Some(required)) =
                        (hook(property, get), hook(inherited, get))
                        && !self.compatible_method(actual, required, class)
                    {
                        return Err(self.hook_signature_error(actual, required));
                    }
                }
            }
        }
        if !class.declaration.is_abstract && !missing.is_empty() {
            let mut methods: Vec<_> = class
                .methods
                .iter()
                .filter(|f| f.is_abstract)
                .cloned()
                .collect();
            methods.extend(missing);
            let missing = methods;
            let count = missing.len();
            let plural = if count == 1 { "" } else { "s" };
            let mut names = missing
                .iter()
                .take(3)
                .map(|f| String::from_utf8_lossy(&f.name).into_owned())
                .collect::<Vec<_>>()
                .join(", ");
            if count > 3 {
                names.push_str(", ...");
            }
            let message = if class.declaration.is_enum
                || class.name.windows(10).any(|p| p == b"@anonymous")
            {
                let kind = if class.declaration.is_enum {
                    "Enum"
                } else {
                    "Class"
                };
                format!(
                    "{kind} {} must implement {count} abstract method{plural} ({names})",
                    class.display_name()
                )
            } else {
                format!(
                    "Class {} contains {count} abstract method{plural} and must therefore be declared abstract or implement the remaining method{plural} ({names})",
                    class.display_name()
                )
            };
            return Err(self.compile_fatal_at(
                &message,
                &class.declaration.source,
                class.declaration.line,
            ));
        }
        Ok(())
    }

    fn hook_signature_error(&mut self, actual: &FunctionDecl, required: &FunctionDecl) -> Error {
        self.compile_fatal_at(
            &format!(
                "Declaration of {} must be compatible with {}",
                interfaces::method_signature(actual),
                interfaces::method_signature(required)
            ),
            &actual.source,
            actual.line,
        )
    }
}

impl Runtime {
    pub(super) fn validate_hook_property_type(
        &mut self,
        actual: &PropertyDecl,
        owner: &Class,
        required: &PropertyDecl,
        parent: &Class,
    ) -> Result<(), Error> {
        let get_only = required
            .hooks
            .as_ref()
            .is_some_and(|h| h.is_virtual && h.set.is_none());
        let set_only = required
            .hooks
            .as_ref()
            .is_some_and(|h| h.is_virtual && h.get.is_none());
        let child_type = actual
            .ty
            .as_ref()
            .map(|ty| inheritance::class_type(ty, owner));
        let parent_type = required
            .ty
            .as_ref()
            .map(|ty| inheritance::class_type(ty, parent));
        let compatible = child_type.is_some() == parent_type.is_some()
            && (set_only || self.type_subset(child_type.as_ref(), parent_type.as_ref(), owner))
            && (get_only || self.type_subset(parent_type.as_ref(), child_type.as_ref(), owner));
        if !compatible {
            let relation = if required.ty.is_none() {
                ""
            } else if get_only {
                "subtype of "
            } else if set_only {
                "supertype of "
            } else {
                ""
            };
            let ty = parent_type
                .as_ref()
                .map_or_else(|| "omitted".to_owned(), |ty| ty.to_string());
            return Err(self.compile_fatal_at(
                &format!(
                    "Type of {}::${} must be {relation}{ty} (as in class {})",
                    owner.display_name(),
                    String::from_utf8_lossy(&actual.name),
                    parent.display_name()
                ),
                &owner.declaration.source,
                owner.declaration.line,
            ));
        }
        if required.is_abstract
            && hook(actual, false).is_none()
            && let Some(set) = hook(required, false)
        {
            let set_type = set.parameters[0]
                .ty
                .as_ref()
                .map(|ty| inheritance::class_type(ty, parent));
            if !self.type_subset(set_type.as_ref(), child_type.as_ref(), owner) {
                let ty = set_type.as_ref().map_or_else(
                    || "mixed".to_owned(),
                    |ty| typed_properties::type_label(ty, parent),
                );
                let kind = if parent.declaration.is_interface {
                    "interface"
                } else {
                    "class"
                };
                return Err(self.compile_fatal_at(
                    &format!(
                        "Set type of {}::${} must be supertype of {ty} (as in {kind} {})",
                        owner.display_name(),
                        String::from_utf8_lossy(&actual.name),
                        parent.display_name()
                    ),
                    &owner.declaration.source,
                    owner.declaration.line,
                ));
            }
        }
        Ok(())
    }
}

fn class_members(ty: &crate::ast::TypeDecl) -> Vec<&str> {
    ty.members
        .iter()
        .map(String::as_str)
        .filter(|name| {
            !matches!(
                *name,
                "int"
                    | "float"
                    | "string"
                    | "bool"
                    | "true"
                    | "false"
                    | "null"
                    | "array"
                    | "iterable"
                    | "object"
                    | "mixed"
                    | "callable"
                    | "void"
                    | "never"
            )
        })
        .collect()
}

impl Runtime {
    pub(super) fn hook_variance_unresolved(&self, class: &Class) -> bool {
        class.declaration.properties.iter().any(|property| {
            let Some(set) = hook(property, false) else {
                return false;
            };
            let (Some(property_type), Some(parameter_type)) = (&property.ty, &set.parameters[0].ty)
            else {
                return false;
            };
            let property_type = inheritance::class_type(property_type, class);
            let parameter_type = inheritance::class_type(parameter_type, class);
            if self.type_subset(Some(&property_type), Some(&parameter_type), class) {
                return false;
            }
            let narrow = class_members(&property_type);
            let wide = class_members(&parameter_type);
            !narrow.is_empty()
                && !wide.is_empty()
                && narrow.into_iter().chain(wide).any(|name| {
                    !class.name.eq_ignore_ascii_case(name.as_bytes())
                        && self.find_class(name.as_bytes()).is_none()
                })
        })
    }

    fn validate_setter_types(&mut self, class: &Class) -> Result<(), Error> {
        for property in &class.declaration.properties {
            let Some(set) = hook(property, false) else {
                continue;
            };
            let (Some(property_type), Some(parameter_type)) = (&property.ty, &set.parameters[0].ty)
            else {
                continue;
            };
            let property_type = inheritance::class_type(property_type, class);
            let offset = set.name.windows(2).position(|p| p == b"::").unwrap();
            let declaring = self.find_class(&set.name[..offset]);
            let parameter_type =
                inheritance::class_type(parameter_type, declaring.as_deref().unwrap_or(class));
            if self.type_subset(Some(&property_type), Some(&parameter_type), class) {
                continue;
            }
            let narrow = class_members(&property_type);
            let wide = class_members(&parameter_type);
            if !narrow.is_empty() && !wide.is_empty() {
                let mut loaded = HashSet::new();
                for name in narrow.into_iter().chain(wide) {
                    if loaded.insert(name.to_ascii_lowercase()) {
                        self.load_class(name.as_bytes(), set.line)?;
                    }
                }
            }
            if !self.type_subset(Some(&property_type), Some(&parameter_type), class) {
                return Err(self.compile_fatal_at(&format!("Type of parameter ${} of hook {}::${}::set must be compatible with property type", String::from_utf8_lossy(&set.parameters[0].name), class.display_name(), String::from_utf8_lossy(&property.name)), &set.source, set.line));
            }
        }
        Ok(())
    }
}
