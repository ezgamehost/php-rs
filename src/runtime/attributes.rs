use super::*;
use crate::{ast::NativeMethod, object::Class};

impl Runtime {
    pub(super) fn builtin_attribute_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, target, body, forbid_dynamic) = match name {
            b"attribute" => (
                "Attribute",
                1,
                "
                const int TARGET_CLASS=1, TARGET_FUNCTION=2, TARGET_METHOD=4,
                    TARGET_PROPERTY=8, TARGET_CLASS_CONSTANT=16, TARGET_PARAMETER=32,
                    TARGET_CONSTANT=64, TARGET_ALL=127, IS_REPEATABLE=128;
                public int $flags;
                public function __construct(int $flags=127) {}",
                false,
            ),
            b"allowdynamicproperties" => (
                "AllowDynamicProperties",
                1,
                "public function __construct() {}",
                false,
            ),
            b"returntypewillchange" => (
                "ReturnTypeWillChange",
                4,
                "public function __construct() {}",
                false,
            ),
            b"override" => ("Override", 12, "public function __construct() {}", true),
            b"sensitiveparameter" => (
                "SensitiveParameter",
                32,
                "public function __construct() {}",
                true,
            ),
            b"sensitiveparametervalue" => (
                "SensitiveParameterValue",
                0,
                "private readonly mixed $value;
                public function __construct(mixed $value) {}
                public function getValue(): mixed {}
                public function __debugInfo(): array {}",
                true,
            ),
            b"deprecated" => (
                "Deprecated",
                87,
                "
                public readonly ?string $message;
                public readonly ?string $since;
                public function __construct(?string $message=null, ?string $since=null) {}",
                true,
            ),
            b"nodiscard" => (
                "NoDiscard",
                6,
                "
                public readonly ?string $message;
                public function __construct(?string $message=null) {}",
                true,
            ),
            b"delayedtargetvalidation" => ("DelayedTargetValidation", 127, "", true),
            _ => return None,
        };
        let marker = if target == 0 {
            String::new()
        } else {
            format!("#[Attribute({target})]")
        };
        let code = format!("<?php {marker} final class {canonical} {{ {body} }}");
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("native attribute source"))
                .expect("native attribute declaration");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            Rc::make_mut(method).native = Some(if target == 0 {
                NativeMethod::SensitiveValue
            } else {
                NativeMethod::AttributeConstruct
            });
        }
        let mut properties = declaration.properties.clone();
        for property in &mut properties {
            property.declaring_class = declaration.name.clone();
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: canonical.as_bytes().to_vec(),
            methods: declaration.methods.clone(),
            declaration,
            properties,
            parent: None,
            interfaces: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    pub(super) fn native_attribute_constructor(&mut self, line: usize) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let names: &[&[u8]] = match object.class_name.as_str() {
            "Attribute" => &[b"flags"],
            "SensitiveParameterValue" => &[b"value"],
            "Deprecated" => &[b"message", b"since"],
            "NoDiscard" => &[b"message"],
            _ => &[],
        };
        for name in names {
            let value = self.lookup(name).expect("bound constructor argument");
            let mut location = Location {
                object: Some(object.clone()),
                static_class: None,
                overloaded: None,
                global: false,
                name: name.to_vec(),
                keys: Vec::new(),
            };
            self.write_location(&mut location, value, line)?;
        }
        Ok(Value::Null)
    }
}
