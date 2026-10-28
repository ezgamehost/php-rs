use super::*;
use crate::{
    ast::{ClassDecl, TraitAdaptation},
    object::{Class, ClassContext},
};

#[derive(Clone)]
struct Candidate {
    order: usize,
    is_alias: bool,
    source: Vec<u8>,
    name: Vec<u8>,
    function: Rc<FunctionDecl>,
    is_final: bool,
}

fn method_name(function: &FunctionDecl) -> &[u8] {
    function.name.rsplit(|byte| *byte == b':').next().unwrap()
}

fn rebound(function: &Rc<FunctionDecl>, class: &[u8], name: &[u8]) -> Rc<FunctionDecl> {
    let mut function = (**function).clone();
    function.name = class.to_vec();
    function.name.extend(b"::");
    function.name.extend(name);
    Rc::new(function)
}

fn composition_error(decl: &ClassDecl, message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Error, message, decl.line)
}

impl Runtime {
    pub(super) fn compose_traits(
        &self,
        declaration: Rc<ClassDecl>,
    ) -> Result<Rc<ClassDecl>, Error> {
        if declaration.trait_uses.is_empty() {
            return Ok(declaration);
        }
        let mut decl = (*declaration).clone();
        let mut traits = Vec::<Rc<Class>>::new();
        for name in declaration.trait_uses.iter().flat_map(|use_| &use_.names) {
            let class = self.find_class(name).ok_or_else(|| {
                composition_error(
                    &decl,
                    format!("Trait \"{}\" not found", String::from_utf8_lossy(name)),
                )
            })?;
            if !class.declaration.is_trait {
                return Err(composition_error(
                    &decl,
                    "Only traits can be used in trait composition",
                ));
            }
            if !traits.iter().any(|previous| Rc::ptr_eq(previous, &class)) {
                traits.push(class);
            }
        }
        let find_trait = |name: &[u8]| traits.iter().find(|t| t.name.eq_ignore_ascii_case(name));
        let mut excluded = HashSet::new();
        for adaptation in declaration
            .trait_uses
            .iter()
            .flat_map(|use_| &use_.adaptations)
        {
            if let TraitAdaptation::InsteadOf {
                source,
                method,
                excluded: others,
            } = adaptation
            {
                let source_class = find_trait(source).ok_or_else(|| {
                    composition_error(&decl, "Required trait was not added to the class")
                })?;
                if objects::method(source_class, method).is_none() {
                    return Err(composition_error(
                        &decl,
                        "A precedence rule references a missing trait method",
                    ));
                }
                for other in others {
                    if find_trait(other).is_none() {
                        return Err(composition_error(
                            &decl,
                            "Excluded trait was not added to the class",
                        ));
                    }
                    if other.eq_ignore_ascii_case(source) {
                        return Err(composition_error(
                            &decl,
                            "Inconsistent insteadof definition",
                        ));
                    }
                    if !excluded.insert((other.to_ascii_lowercase(), method.to_ascii_lowercase())) {
                        return Err(composition_error(
                            &decl,
                            "A trait method was excluded multiple times",
                        ));
                    }
                }
            }
        }
        for adaptation in declaration
            .trait_uses
            .iter()
            .flat_map(|use_| &use_.adaptations)
        {
            if let TraitAdaptation::InsteadOf { source, method, .. } = adaptation
                && excluded.contains(&(source.to_ascii_lowercase(), method.to_ascii_lowercase()))
            {
                return Err(composition_error(
                    &decl,
                    "Inconsistent insteadof definition",
                ));
            }
        }
        let mut originals: Vec<_> = traits
            .iter()
            .flat_map(|class| {
                class.methods.iter().map(|function| Candidate {
                    order: 0,
                    is_alias: false,
                    source: class.name.clone(),
                    name: method_name(function).to_vec(),
                    function: function.clone(),
                    is_final: class
                        .declaration
                        .final_methods
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(method_name(function))),
                })
            })
            .collect();
        for (order, original) in originals.iter_mut().enumerate() {
            original.order = order;
        }
        let mut candidates: Vec<_> = originals
            .iter()
            .filter(|method| {
                !excluded.contains(&(
                    method.source.to_ascii_lowercase(),
                    method.name.to_ascii_lowercase(),
                ))
            })
            .cloned()
            .collect();
        for adaptation in declaration
            .trait_uses
            .iter()
            .flat_map(|use_| &use_.adaptations)
        {
            let TraitAdaptation::Alias {
                source,
                method,
                alias,
                visibility,
                is_final,
            } = adaptation
            else {
                continue;
            };
            let matches: Vec<_> = originals
                .iter()
                .filter(|candidate| {
                    candidate.name.eq_ignore_ascii_case(method)
                        && source.as_ref().map_or_else(
                            || {
                                !excluded.contains(&(
                                    candidate.source.to_ascii_lowercase(),
                                    method.to_ascii_lowercase(),
                                ))
                            },
                            |source| candidate.source.eq_ignore_ascii_case(source),
                        )
                })
                .collect();
            if matches.len() != 1 {
                return Err(composition_error(
                    &decl,
                    "Trait alias references a missing or ambiguous method",
                ));
            }
            let original = matches[0];
            if let Some(alias) = alias {
                let mut candidate = original.clone();
                candidate.name = alias.clone();
                candidate.is_alias = true;
                if let Some(visibility) = visibility {
                    Rc::make_mut(&mut candidate.function).visibility = *visibility;
                }
                candidate.is_final |= is_final;
                candidates.push(candidate);
            } else {
                for candidate in &mut candidates {
                    if candidate.source.eq_ignore_ascii_case(&original.source)
                        && candidate.name.eq_ignore_ascii_case(method)
                    {
                        if let Some(visibility) = visibility {
                            Rc::make_mut(&mut candidate.function).visibility = *visibility;
                        }
                        candidate.is_final |= is_final;
                    }
                }
            }
        }
        // Zend installs each method's aliases in adaptation order before the
        // original method, including aliases of excluded originals.
        candidates.sort_by_key(|candidate| (candidate.order, !candidate.is_alias));
        let own_names: HashSet<_> = decl
            .methods
            .iter()
            .map(|f| method_name(f).to_ascii_lowercase())
            .collect();
        let mut imported = Vec::<Candidate>::new();
        for candidate in candidates {
            let function = rebound(&candidate.function, &decl.name, &candidate.name);
            if function.is_abstract {
                decl.trait_requirements.push(function);
            }
            if own_names.contains(&candidate.name.to_ascii_lowercase()) {
                continue;
            }
            if let Some(previous) = imported
                .iter_mut()
                .find(|previous| previous.name.eq_ignore_ascii_case(&candidate.name))
            {
                if previous.function.is_abstract && !candidate.function.is_abstract {
                    *previous = candidate;
                } else if !previous.function.is_abstract
                    && !candidate.function.is_abstract
                    && previous.function.trait_origin != candidate.function.trait_origin
                {
                    return Err(composition_error(
                        &decl,
                        format!(
                            "Trait method {}::{} has not been applied as {}::{}, because of collision with {}::{}",
                            String::from_utf8_lossy(&candidate.source),
                            String::from_utf8_lossy(&candidate.name),
                            String::from_utf8_lossy(&decl.name),
                            String::from_utf8_lossy(&candidate.name),
                            String::from_utf8_lossy(&previous.source),
                            String::from_utf8_lossy(&previous.name)
                        ),
                    ));
                }
            } else {
                imported.push(candidate);
            }
        }
        for candidate in imported {
            if candidate.function.is_abstract
                && decl
                    .parent
                    .as_ref()
                    .and_then(|name| self.find_class(name))
                    .and_then(|parent| objects::method(&parent, &candidate.name))
                    .is_some_and(|method| !method.is_abstract)
            {
                continue;
            }
            if candidate.is_final {
                decl.final_methods.push(candidate.name.to_ascii_lowercase());
            }
            decl.methods
                .push(rebound(&candidate.function, &decl.name, &candidate.name));
        }
        for class in traits {
            let used = &class.declaration;
            for required in &used.trait_requirements {
                decl.trait_requirements
                    .push(rebound(required, &decl.name, method_name(required)));
            }
            decl.trait_property_checks
                .extend(used.trait_property_checks.iter().cloned());
            decl.trait_constant_checks
                .extend(used.trait_constant_checks.iter().cloned());
            let property_order = decl
                .properties
                .iter()
                .chain(&decl.static_properties)
                .map(|p| p.order + 1)
                .max()
                .unwrap_or(0);
            for (properties, is_static) in
                [(&used.properties, false), (&used.static_properties, true)]
            {
                for property in properties {
                    if decl.is_readonly && !property.is_readonly {
                        return Err(composition_error(
                            &decl,
                            "Readonly classes cannot use non-readonly trait properties",
                        ));
                    }
                    let other = if is_static {
                        &decl.properties
                    } else {
                        &decl.static_properties
                    };
                    if other.iter().any(|other| other.name == property.name) {
                        return Err(composition_error(
                            &decl,
                            "Trait property has incompatible staticness",
                        ));
                    }
                    let target = if is_static {
                        &mut decl.static_properties
                    } else {
                        &mut decl.properties
                    };
                    if let Some(existing) = target
                        .iter()
                        .find(|existing| existing.name == property.name)
                    {
                        decl.trait_property_checks
                            .push((existing.clone(), property.clone()));
                    } else {
                        let mut property = property.clone();
                        property.order += property_order;
                        target.push(property);
                    }
                }
            }
            for constant in &used.constants {
                if let Some(existing) = decl
                    .constants
                    .iter()
                    .find(|existing| existing.name == constant.name)
                {
                    decl.trait_constant_checks
                        .push((existing.clone(), constant.clone()));
                } else {
                    decl.constants.push(constant.clone());
                }
            }
        }
        Ok(Rc::new(decl))
    }

    pub(super) fn validate_trait_requirements(&self, class: &Class) -> Result<(), Error> {
        for required in &class.declaration.trait_requirements {
            let Some(actual) = objects::method(class, method_name(required)) else {
                continue;
            };
            if actual.is_abstract && actual.visibility == crate::ast::Visibility::Private {
                return Err(composition_error(
                    &class.declaration,
                    "Private abstract trait methods must be implemented by the using class",
                ));
            }
            if actual.is_static != required.is_static
                || actual.visibility > required.visibility
                || !self.compatible_method(&actual, required, class)
            {
                return Err(composition_error(
                    &class.declaration,
                    format!(
                        "Declaration of {}() must be compatible with {}()",
                        String::from_utf8_lossy(&actual.name),
                        String::from_utf8_lossy(&required.name)
                    ),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_trait_values(&mut self, class: &Rc<Class>) -> Result<(), Error> {
        let previous = self.class_context.replace(ClassContext {
            scope: class.clone(),
            called: class.clone(),
        });
        let previous_check = std::mem::replace(&mut self.checking_trait_values, true);
        let result = self.trait_values(class);
        self.checking_trait_values = previous_check;
        self.class_context = previous;
        result
    }

    fn trait_values(&mut self, class: &Rc<Class>) -> Result<(), Error> {
        for (left, right) in &class.declaration.trait_property_checks {
            let same_type = same_type(left.ty.as_ref(), right.ty.as_ref(), class);
            if left.visibility != right.visibility
                || left.set_visibility != right.set_visibility
                || left.is_readonly != right.is_readonly
                || left.is_final != right.is_final
                || !same_type
            {
                return Err(composition_error(
                    &class.declaration,
                    "Incompatible trait property declaration",
                ));
            }
            let value = |runtime: &mut Self,
                         property: &crate::ast::PropertyDecl|
             -> Result<Option<Value>, Error> {
                match &property.default {
                    Some(expr) => runtime.eval_default(expr).map(Some),
                    None if property.ty.is_none() => Ok(Some(Value::Null)),
                    None => Ok(None),
                }
            };
            if value(self, left)? != value(self, right)? {
                return Err(composition_error(
                    &class.declaration,
                    "Incompatible trait property default value",
                ));
            }
        }
        for (left, right) in &class.declaration.trait_constant_checks {
            if left.visibility != right.visibility
                || left.is_final != right.is_final
                || !same_type(left.ty.as_ref(), right.ty.as_ref(), class)
                || self.eval_default(&left.value)? != self.eval_default(&right.value)?
            {
                return Err(composition_error(
                    &class.declaration,
                    "Incompatible trait constant declaration",
                ));
            }
        }
        Ok(())
    }
}

fn same_type(
    left: Option<&crate::ast::TypeDecl>,
    right: Option<&crate::ast::TypeDecl>,
    class: &Class,
) -> bool {
    let normalized = |ty: Option<&crate::ast::TypeDecl>| {
        ty.map(|ty| {
            let mut names: Vec<_> = inheritance::class_type(ty, class)
                .members
                .iter()
                .map(|member| member.to_ascii_lowercase())
                .collect();
            names.sort();
            names.dedup();
            names
        })
    };
    normalized(left) == normalized(right)
}
