use super::*;
use crate::{
    PhpObject,
    ast::{Argument, NativeMethod, Parameter, Visibility},
};

impl Runtime {
    pub(super) fn call_parent_property_hook(
        &mut self,
        name: &[u8],
        get: bool,
        args: &[Argument],
        line: usize,
        result_used: bool,
    ) -> Result<Value, Error> {
        // Parsing has already checked the lexical hook/property context.
        let parent = self
            .class_context
            .as_ref()
            .and_then(|c| c.scope.parent.clone())
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    "Cannot use \"parent\" when current class scope has no parent",
                    line,
                )
            })?;
        let property = parent
            .properties
            .iter()
            .rev()
            .find(|p| p.name == name)
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    format!(
                        "Undefined property {}::${}",
                        String::from_utf8_lossy(&parent.name),
                        String::from_utf8_lossy(name)
                    ),
                    line,
                )
            })?;
        if property.visibility == Visibility::Private {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot access private property {}::${}",
                    String::from_utf8_lossy(&parent.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            return Err(Error::new(
                ErrorKind::Error,
                "Using $this when not in object context",
                line,
            ));
        };
        let function = property
            .hooks
            .as_ref()
            .and_then(|h| if get { h.get.clone() } else { h.set.clone() });
        let function = match function {
            Some(function) => function,
            None => {
                let mut function_name = property.declaring_class.clone();
                function_name.extend(b"::$");
                function_name.extend(name);
                function_name.extend(if get { b"::get" } else { b"::set" });
                Rc::new(FunctionDecl {
                    doc_comment: None,
                    tentative_return_type: None,
                    end_line: line,
                    attributes: Vec::new(),
                    returns_reference: false,
                    trait_origin: None,
                    native: Some(NativeMethod::ParentPropertyHook(get)),
                    visibility: Visibility::Public,
                    is_abstract: false,
                    is_static: false,
                    name: function_name,
                    parameters: if get {
                        Vec::new()
                    } else {
                        vec![Parameter {
                            attributes: Vec::new(),
                            by_reference: false,
                            promotion: None,
                            name: b"value".to_vec(),
                            ty: None,
                            implicit_nullable: false,
                            default: None,
                            variadic: false,
                        }]
                    },
                    body: Vec::new(),
                    line,
                    return_type: None,
                    strict_types: false,
                    source: self.source.clone(),
                })
            }
        };
        self.invoke_property_hook_arguments(&object, name, function, args, line, result_used)
    }

    pub(super) fn invoke_property_hook_arguments(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        function: Rc<FunctionDecl>,
        args: &[Argument],
        line: usize,
        result_used: bool,
    ) -> Result<Value, Error> {
        let mut target = self.unchecked_method_target(
            function.clone(),
            object.class.clone(),
            Some(object.clone()),
            line,
        )?;
        target.result_used = result_used;
        self.active_property_hooks
            .push((object.clone(), name.to_vec(), function.name.clone()));
        let result = self.invoke(target, args, line, false);
        self.active_property_hooks.pop();
        result
    }

    pub(super) fn native_parent_property_hook(
        &mut self,
        get: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let (object, name, _) = self
            .active_property_hooks
            .last()
            .expect("parent property accessor")
            .clone();
        if get {
            self.read_property_value(&object, &name, line, Access::Read)
        } else {
            let value = self.lookup(b"value").unwrap();
            self.write_property_value(&object, &name, value, line)
        }
    }
}
