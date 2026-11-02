// Altered Rust adaptation of PHP 8.5.0's call and constant attribute diagnostics.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{ast::NativeMethod, object::Class};

impl Runtime {
    pub(super) fn deprecated_constant(
        &mut self,
        attributes: &[crate::ast::Attribute],
        name: &[u8],
        scope: Option<Rc<Class>>,
        enum_case: bool,
        line: usize,
    ) -> Result<(), Error> {
        let Some(attribute) = attributes
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
        else {
            return Ok(());
        };
        let guard = (
            scope.as_ref().map_or(0, |class| Rc::as_ptr(class) as usize),
            name.to_vec(),
        );
        if !self.deprecated_constants.insert(guard.clone()) {
            return Ok(());
        }
        // Protect both attribute evaluation and the error handler: either may
        // read this same constant, including through an alias of its class.
        let result = (|| {
            let mut message = if enum_case {
                b"Enum case ".to_vec()
            } else {
                b"Constant ".to_vec()
            };
            if let Some(class) = &scope {
                message.extend(&class.name);
                message.extend(b"::");
            }
            message.extend(name);
            message.extend(b" is deprecated");
            message.extend(self.diagnostic_attribute_suffix(attribute, scope, line)?);
            self.attribute_diagnostic(&message, 16384, line, false)
        })();
        self.deprecated_constants.remove(&guard);
        result
    }

    pub(super) fn call_attribute_diagnostics(
        &mut self,
        target: &functions::CallTarget,
        requested: Option<&[u8]>,
        line: usize,
        internal: bool,
    ) -> Result<(), Error> {
        let Some(function) = &target.function else {
            if matches!(target.builtin, Some(builtins::Builtin::AssertOptions)) {
                self.attribute_diagnostic(
                    b"Function assert_options() is deprecated since 8.3",
                    8192,
                    line,
                    internal,
                )?;
            }
            if matches!(target.builtin, Some(builtins::Builtin::LcgValue)) {
                self.attribute_diagnostic(
                    b"Function lcg_value() is deprecated since 8.4, use \\Random\\Randomizer::getFloat() instead",
                    8192,
                    line,
                    internal,
                )?;
            }
            return Ok(());
        };
        // Zend reports Deprecated first, and stops if its handler throws.
        for (attribute_name, discarded) in [(b"Deprecated".as_slice(), false), (b"NoDiscard", true)]
        {
            if discarded && target.result_used {
                continue;
            }
            let Some(attribute) = function
                .attributes
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case(attribute_name))
            else {
                continue;
            };
            let scope = target.context.as_ref().map(|c| c.scope.clone());
            let suffix = self.diagnostic_attribute_suffix(attribute, scope.clone(), line)?;
            let mut message = if let Some(scope) = scope {
                let name = requested.unwrap_or_else(|| {
                    if function.name.starts_with(b"{closure:") {
                        &function.name
                    } else {
                        function.name.rsplit(|b| *b == b':').next().unwrap()
                    }
                });
                let mut message = if discarded {
                    b"The return value of method ".to_vec()
                } else {
                    b"Method ".to_vec()
                };
                message.extend(&scope.name);
                message.extend(b"::");
                message.extend(name);
                message
            } else {
                let mut message = if discarded {
                    b"The return value of function ".to_vec()
                } else {
                    b"Function ".to_vec()
                };
                message.extend(&function.name);
                message
            };
            message.extend(if discarded {
                b"() should either be used or intentionally ignored by casting it as (void)"
                    .as_slice()
            } else {
                b"() is deprecated"
            });
            message.extend(suffix);
            let level = match (discarded, function.native.is_some()) {
                (true, true) => 2,
                (true, false) => 512,
                (false, true) => 8192,
                (false, false) => 16384,
            };
            self.attribute_diagnostic(&message, level, line, internal)?;
        }
        Ok(())
    }

    pub(super) fn diagnostic_attribute_suffix(
        &mut self,
        attribute: &crate::ast::Attribute,
        scope: Option<Rc<Class>>,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        if attribute.arguments.is_empty() {
            return Ok(Vec::new());
        }
        // Attribute constants use the declaration's class scope. Construction
        // uses the current call site and its strictness, as Zend does here.
        let outer = std::mem::replace(
            &mut self.class_context,
            scope.map(|class| crate::object::ClassContext {
                scope: class.clone(),
                called: class,
            }),
        );
        let outer_line = self
            .attribute_evaluation_line
            .replace((self.trace_frames.len(), line));
        let arguments = (|| {
            let mut result = Vec::new();
            for argument in &attribute.arguments {
                let expr = match argument {
                    crate::ast::Argument::Positional(expr)
                    | crate::ast::Argument::Named(_, expr) => expr,
                    _ => unreachable!(),
                };
                let value = self.eval_default(expr)?;
                let literal = Expr {
                    kind: ExprKind::Literal(value),
                    line,
                };
                result.push(match argument {
                    crate::ast::Argument::Named(name, _) => {
                        crate::ast::Argument::Named(name.clone(), literal)
                    }
                    _ => crate::ast::Argument::Positional(literal),
                });
            }
            Ok::<_, Error>(result)
        })();
        self.attribute_evaluation_line = outer_line;
        self.class_context = outer;
        let arguments = arguments?;
        let class = self
            .find_class(&attribute.name)
            .expect("built-in attribute");
        let Value::Object(object) = self.instantiate_class(class, &arguments, true, line)? else {
            unreachable!()
        };
        let properties = object.properties.borrow();
        let mut suffix = Vec::new();
        for (name, prefix) in [
            (b"since".as_slice(), b" since ".as_slice()),
            (b"message".as_slice(), b", ".as_slice()),
        ] {
            if let Some(Value::String(value)) = properties.get(&ArrayKey::String(name.to_vec()))
                && !value.is_empty()
            {
                suffix.extend(prefix);
                suffix.extend(value.bytes());
            }
        }
        Ok(suffix)
    }

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
                offset_reference: None,
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
