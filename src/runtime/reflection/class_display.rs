use super::*;

impl Runtime {
    pub(super) fn class_extension(&self, class: &Rc<Class>) -> Option<&'static str> {
        self.builtin_classes
            .borrow()
            .values()
            .any(|c| Rc::ptr_eq(c, class))
            .then(|| class_modules::extension(&class.name.to_ascii_lowercase()))
            .flatten()
    }

    pub(super) fn reflection_class_string(
        &mut self,
        class: &Rc<Class>,
        object: Option<&Rc<PhpObject>>,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        let decl = &class.declaration;
        let mut out = Vec::new();
        if let Some(comment) = &decl.doc_comment {
            out.extend(comment);
            out.push(b'\n');
        }
        out.extend(if object.is_some() {
            b"Object of class [ ".as_slice()
        } else if decl.is_interface {
            b"Interface [ "
        } else if decl.is_trait {
            b"Trait [ "
        } else if decl.is_enum {
            b"Enum [ "
        } else {
            b"Class [ ".as_slice()
        });
        let internal = self
            .builtin_classes
            .borrow()
            .values()
            .any(|c| Rc::ptr_eq(c, class));
        out.extend(if internal {
            b"<internal".as_slice()
        } else {
            b"<user"
        });
        if let Some(extension) = self.class_extension(class) {
            out.push(b':');
            out.extend(extension.bytes());
        }
        out.extend(b"> ");
        if !decl.is_interface && !decl.is_trait && class.is_a(b"Traversable") {
            out.extend(b"<iterateable> ");
        }
        if decl.is_interface {
            out.extend(b"interface ");
        } else if decl.is_trait {
            out.extend(b"trait ");
        } else if decl.is_enum {
            out.extend(b"enum ");
        } else {
            if decl.is_abstract {
                out.extend(b"abstract ");
            }
            if decl.is_final {
                out.extend(b"final ");
            }
            if decl.is_readonly {
                out.extend(b"readonly ");
            }
            out.extend(b"class ");
        }
        out.extend(&class.name);
        if let Some(parent) = &class.parent {
            out.extend(b" extends ");
            out.extend(&parent.name);
        }
        if let Some(backing) = &decl.enum_backing {
            out.extend(b": ");
            out.extend(backing.bytes());
        }
        let mut interfaces = Vec::new();
        collect_interfaces(class, &mut interfaces);
        for (i, interface) in interfaces.iter().enumerate() {
            if i == 0 {
                out.extend(if decl.is_interface {
                    b" extends ".as_slice()
                } else {
                    b" implements "
                });
            } else {
                out.extend(b", ");
            }
            out.extend(&interface.name);
        }
        out.extend(b" ] {\n");
        if !internal {
            out.extend(b"  @@ ");
            out.extend(&decl.source.name);
            out.extend(format!(" {}-{}\n", decl.line, decl.end_line).bytes());
        }
        let mut constants = Vec::new();
        let mut cases = Vec::new();
        for name in class.constant_names() {
            let owner = class.constant_owner(&name).unwrap();
            let constant = owner
                .declaration
                .constants
                .iter()
                .find(|c| c.name == name)
                .unwrap();
            let value = self.evaluate_class_constant(&owner, &name, line)?;
            if constant.is_enum_case {
                let mut row = Vec::new();
                if let Some(comment) = &constant.doc_comment {
                    row.extend(b"    ");
                    row.extend(comment);
                    row.push(b'\n');
                }
                row.extend(b"    Case ");
                row.extend(&name);
                if decl.enum_backing.is_some() {
                    let Value::Object(case) = &value else {
                        unreachable!()
                    };
                    let backing = case
                        .properties
                        .borrow()
                        .get(&ArrayKey::String(b"value".to_vec()))
                        .unwrap();
                    row.extend(b" = ");
                    row.extend(backing.to_bytes().split(|b| *b == 0).next().unwrap());
                }
                row.push(b'\n');
                cases.push(row);
            } else {
                constants.push(
                    self.reflected_constant_string(&owner, &name, &name, &value, b"    ", line)?,
                );
            }
        }
        if !cases.is_empty() {
            section(&mut out, "Enum cases", &cases, false);
        }
        section(&mut out, "Constants", &constants, false);
        let properties = properties::properties(class);
        let mut static_properties = Vec::new();
        for property in properties.iter().filter(|p| p.is_static) {
            static_properties.push(
                self.reflected_property_string(property, b"    ")?
                    .to_bytes(),
            );
        }
        section(&mut out, "Static properties", &static_properties, false);
        let methods = methods(class);
        let mut static_methods = Vec::new();
        for method in methods.iter().filter(|m| m.is_static) {
            let name = method.name.rsplit(|b| *b == b':').next().unwrap();
            let reflected = self.reflected_method(class, name, line)?;
            static_methods
                .push(self.reflection_function_string_indented(&reflected, line, b"    ")?);
        }
        section(&mut out, "Static methods", &static_methods, true);
        let mut instance_properties = Vec::new();
        for property in properties.iter().filter(|p| !p.is_static) {
            instance_properties.push(
                self.reflected_property_string(property, b"    ")?
                    .to_bytes(),
            );
        }
        section(&mut out, "Properties", &instance_properties, false);
        if let Some(object) = object {
            let mut dynamic = Vec::new();
            for (key, _) in object.properties.borrow().entries().iter() {
                let ArrayKey::String(name) = key else {
                    continue;
                };
                if name.is_empty()
                    || name[0] == 0
                    || class
                        .properties
                        .iter()
                        .chain(&class.static_properties)
                        .any(|p| p.name == *name)
                {
                    continue;
                }
                let mut row = b"    Property [ <dynamic> public $".to_vec();
                row.extend(name.split(|b| *b == 0).next().unwrap());
                row.extend(b" ]\n");
                dynamic.push(row);
            }
            section(&mut out, "Dynamic properties", &dynamic, false);
        }
        let mut instance_methods = Vec::new();
        for method in methods.iter().filter(|m| !m.is_static) {
            let name = method.name.rsplit(|b| *b == b':').next().unwrap();
            let reflected = self.reflected_method(class, name, line)?;
            instance_methods
                .push(self.reflection_function_string_indented(&reflected, line, b"    ")?);
        }
        section(&mut out, "Methods", &instance_methods, true);
        out.extend(b"}\n");
        Ok(out)
    }
}

fn section(out: &mut Vec<u8>, label: &str, rows: &[Vec<u8>], methods: bool) {
    out.extend(format!("\n  - {label} [{}] {{", rows.len()).bytes());
    if !methods || rows.is_empty() {
        out.push(b'\n');
    }
    for row in rows {
        if methods {
            out.push(b'\n');
        }
        out.extend(row);
    }
    out.extend(b"  }\n");
}

pub(super) fn methods(class: &Rc<Class>) -> Vec<Rc<FunctionDecl>> {
    let mut methods = class.declaration.methods.clone();
    for inherited in &class.methods {
        let short = inherited.name.rsplit(|b| *b == b':').next().unwrap();
        if inherited.visibility != Visibility::Private
            && !methods.iter().any(|m| {
                m.name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(short)
            })
        {
            methods.push(inherited.clone());
        }
    }
    methods
}
