use super::*;

impl Runtime {
    pub(in crate::runtime) fn builtin_reflection_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent_name, body) = match name {
            b"reflectionclass" => (
                "ReflectionClass",
                None,
                "
                public const int IS_IMPLICIT_ABSTRACT=16, IS_EXPLICIT_ABSTRACT=64,
                    IS_FINAL=32, IS_READONLY=65536;
                public readonly string $name;
                private function __clone(): void {}
                public function __construct(object|string $objectOrClass) {}
                public function getName(): string {}
                public function getShortName(): string {}
                public function getNamespaceName(): string {}
                public function inNamespace(): bool {}
                public function isInternal(): bool {}
                public function isUserDefined(): bool {}
                public function isAnonymous(): bool {}
                public function isInstantiable(): bool {}
                public function isCloneable(): bool {}
                public function isInterface(): bool {}
                public function isTrait(): bool {}
                public function isEnum(): bool {}
                public function isAbstract(): bool {}
                public function isFinal(): bool {}
                public function isReadOnly(): bool {}
                public function getModifiers(): int {}
                public function isInstance(object $object): bool {}
                public function getParentClass(): ReflectionClass|false {}
                public function isSubclassOf(ReflectionClass|string $class): bool {}
                public function implementsInterface(ReflectionClass|string $interface): bool {}
                public function getInterfaceNames(): array {}
                public function getInterfaces(): array {}
                public function getTraitNames(): array {}
                public function getTraits(): array {}
                public function getTraitAliases(): array {}
                public function hasMethod(string $name): bool {}
                public function getMethod(string $name): ReflectionMethod {}
                public function getMethods(?int $filter=null): array {}
                public function getConstructor(): ?ReflectionMethod {}
                public function hasProperty(string $name): bool {}
                public function hasConstant(string $name): bool {}
                public function getConstant(string $name): mixed {}
                public function getConstants(?int $filter=null): array {}
                public function getFileName(): string|false {}
                public function getStartLine(): int|false {}
                public function getEndLine(): int|false {}
                public function getStaticProperties(): array {}
                public function getStaticPropertyValue(string $name, mixed $default=null): mixed {}
                public function setStaticPropertyValue(string $name, mixed $value): void {}
                public function getDefaultProperties(): array {}
                public function getAttributes(?string $name=null, int $flags=0): array {}
                public function newInstance(mixed ...$args): object {}
                public function newInstanceArgs(array $args=[]): ?object {}
                public function newInstanceWithoutConstructor(): object {}
            ",
            ),
            b"reflectionobject" => (
                "ReflectionObject",
                Some("ReflectionClass"),
                "
                public function __construct(object $object) {}
            ",
            ),
            b"reflectionattribute" => (
                "ReflectionAttribute",
                None,
                "
                public const int IS_INSTANCEOF=2;
                private function __construct() {}
                private function __clone(): void {}
                public function getName(): string {}
                public function getTarget(): int {}
                public function isRepeated(): bool {}
                public function getArguments(): array {}
                public function newInstance(): object {}
            ",
            ),
            b"reflectionfunctionabstract" => ("ReflectionFunctionAbstract", None, "
                public readonly string $name;
                private function __clone(): void {}
                public function getName(): string {}
                public function getShortName(): string {}
                public function getNamespaceName(): string {}
                public function inNamespace(): bool {}
                public function isClosure(): bool {}
                public function isDeprecated(): bool {}
                public function isInternal(): bool {}
                public function isUserDefined(): bool {}
                public function isGenerator(): bool {}
                public function isVariadic(): bool {}
                public function isStatic(): bool {}
                public function getClosureThis(): ?object {}
                public function getClosureScopeClass(): ?ReflectionClass {}
                public function getClosureCalledClass(): ?ReflectionClass {}
                public function getClosureUsedVariables(): array {}
                public function getFileName(): string|false {}
                public function getStartLine(): int|false {}
                public function getEndLine(): int|false {}
                public function getParameters(): array {}
                public function getNumberOfParameters(): int {}
                public function getNumberOfRequiredParameters(): int {}
                public function returnsReference(): bool {}
                public function hasReturnType(): bool {}
                public function getReturnType(): ?ReflectionType {}
                public function hasTentativeReturnType(): bool {}
                public function getTentativeReturnType(): ?ReflectionType {}
                public function getAttributes(?string $name=null, int $flags=0): array {}
            "),
            b"reflectionfunction" => ("ReflectionFunction", Some("ReflectionFunctionAbstract"), "
                public const int IS_DEPRECATED=2048;
                public function __construct(Closure|string $function) {}
                public function isAnonymous(): bool {}
                public function getClosure(): Closure {}
                public function invoke(mixed ...$args): mixed {}
                public function invokeArgs(array $args): mixed {}
            "),
            b"reflectionmethod" => ("ReflectionMethod", Some("ReflectionFunctionAbstract"), "
                public const int IS_STATIC=16, IS_PUBLIC=1, IS_PROTECTED=2, IS_PRIVATE=4, IS_ABSTRACT=64, IS_FINAL=32;
                public readonly string $class;
                public function __construct(object|string $objectOrMethod, ?string $method=null) {}
                public static function createFromMethodName(string $method): static {}
                public function isPublic(): bool {}
                public function isProtected(): bool {}
                public function isPrivate(): bool {}
                public function isAbstract(): bool {}
                public function isFinal(): bool {}
                public function isConstructor(): bool {}
                public function isDestructor(): bool {}
                public function getModifiers(): int {}
                public function getDeclaringClass(): ReflectionClass {}
                public function getClosure(?object $object=null): Closure {}
                public function invoke(?object $object, mixed ...$args): mixed {}
                public function invokeArgs(?object $object, array $args): mixed {}
                public function setAccessible(bool $accessible): void {}
            "),
            b"reflectionparameter" => ("ReflectionParameter", None, "
                public readonly string $name;
                private function __clone(): void {}
                public function __construct($function, int|string $param) {}
                public function getName(): string {}
                public function isPassedByReference(): bool {}
                public function canBePassedByValue(): bool {}
                public function getDeclaringFunction(): ReflectionFunctionAbstract {}
                public function getDeclaringClass(): ?ReflectionClass {}
                public function hasType(): bool {}
                public function getType(): ?ReflectionType {}
                public function allowsNull(): bool {}
                public function getPosition(): int {}
                public function isOptional(): bool {}
                public function isDefaultValueAvailable(): bool {}
                public function getDefaultValue(): mixed {}
                public function isDefaultValueConstant(): bool {}
                public function getDefaultValueConstantName(): ?string {}
                public function isVariadic(): bool {}
                public function isPromoted(): bool {}
                public function getAttributes(?string $name=null, int $flags=0): array {}
            "),
            b"reflectiontype" => ("ReflectionType", None, "
                private function __clone(): void {}
                public function allowsNull(): bool {}
                public function __toString(): string {}
            "),
            b"reflectionnamedtype" => ("ReflectionNamedType", Some("ReflectionType"), "
                public function getName(): string {}
                public function isBuiltin(): bool {}
            "),
            b"reflectionuniontype" => ("ReflectionUnionType", Some("ReflectionType"), "
                public function getTypes(): array {}
            "),
            _ => return None,
        };
        let abstract_ = if matches!(canonical, "ReflectionFunctionAbstract" | "ReflectionType") {
            "abstract "
        } else {
            ""
        };
        let implements = if canonical == "ReflectionType" {
            " implements Stringable"
        } else {
            ""
        };
        let code = format!("<?php {abstract_}class {canonical}{implements} {{ {body} }}");
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("reflection source"))
                .expect("reflection declarations");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            Rc::make_mut(method).native = Some(NativeMethod::Reflection);
        }
        let parent = parent_name.and_then(|name| self.find_class(name.as_bytes()));
        let mut methods = parent.as_ref().map_or_else(Vec::new, |p| p.methods.clone());
        for own in &declaration.methods {
            let name = own.name.rsplit(|b| *b == b':').next().unwrap();
            methods.retain(|method| {
                !method
                    .name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(name)
            });
            methods.push(own.clone());
        }
        let mut properties = parent
            .as_ref()
            .map_or_else(Vec::new, |p| p.properties.clone());
        for property in &declaration.properties {
            let mut property = property.clone();
            property.declaring_class = declaration.name.clone();
            properties.push(property);
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: canonical.as_bytes().to_vec(),
            methods,
            declaration,
            properties,
            parent,
            interfaces: if canonical == "ReflectionType" {
                vec![self.find_class(b"Stringable").unwrap()]
            } else {
                Vec::new()
            },
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: false,
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
}
