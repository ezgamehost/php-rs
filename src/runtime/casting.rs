use super::*;
use crate::{
    ast::CastKind,
    value::{float_to_int, numeric_prefix},
};

pub(super) fn cast(
    kind: CastKind,
    value: Value,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    if matches!(value, Value::Closure(_) | Value::Object(_))
        && matches!(kind, CastKind::Int | CastKind::Float)
    {
        let name = if kind == CastKind::Int {
            "int"
        } else {
            "float"
        };
        warnings.warn(format!(
            "Object of class {} could not be converted to {name} on line {line}",
            builtins::debug_type(&value)
        ))?;
    }
    Ok(match kind {
        CastKind::Object => unreachable!("object allocation is handled by Runtime"),
        CastKind::Void => Value::Null,
        CastKind::Bool => Value::Bool(value.to_bool(line, warnings)?),
        CastKind::String => Value::String(string_bytes(&value, line, warnings)?.into()),
        CastKind::Array => match value {
            Value::Array(_) => value,
            Value::Object(object) => {
                let mut array = PhpArray::default();
                for (key, value) in object.export_properties().entries().iter() {
                    array.insert(
                        ArrayKey::from_value(&key.to_value(), line, warnings)?,
                        value.clone(),
                    );
                }
                Value::Array(Rc::new(array))
            }
            value => {
                value.warn_nan("array", line, warnings)?;
                let mut array = PhpArray::default();
                if value != Value::Null {
                    array.append(value, line)?;
                }
                Value::Array(Rc::new(array))
            }
        },
        CastKind::Int => {
            Value::Int(match &value {
                Value::Int(number) => *number,
                Value::Resource(resource) => resource.id,
                Value::Float(number) => {
                    if out_of_range(*number) {
                        warnings.warn(format!("The float {} is not representable as an int, cast occurred on line {line}", String::from_utf8_lossy(&value.to_bytes())))?;
                    }
                    float_to_int(*number)
                }
                Value::String(bytes) => match numeric_prefix(&bytes.bytes()) {
                    Some((Value::Int(number), _)) => number,
                    Some((Value::Float(number), _)) => {
                        if out_of_range(number) {
                            warnings.warn(format!("The float-string \"{}\" is not representable as an int, cast occurred on line {line}", String::from_utf8_lossy(&bytes.bytes())))?;
                        }
                        if number.is_finite() { number as i64 } else { 0 }
                    }
                    _ => 0,
                },
                _ => i64::from(value.is_truthy()),
            })
        }
        CastKind::Float => Value::Float(match &value {
            Value::Float(number) => *number,
            Value::Int(number) => *number as f64,
            Value::Resource(resource) => resource.id as f64,
            Value::String(bytes) => match numeric_prefix(&bytes.bytes()) {
                Some((number, _)) => {
                    let number = number.as_float();
                    if number == 0.0
                        && bytes
                            .bytes()
                            .iter()
                            .find(|b| !crate::value::numeric_whitespace(b))
                            == Some(&b'-')
                    {
                        -0.0
                    } else {
                        number
                    }
                }
                _ => 0.0,
            },
            _ => {
                if value.is_truthy() {
                    1.0
                } else {
                    0.0
                }
            }
        }),
    })
}

impl Runtime {
    pub(super) fn cast_object(&mut self, value: Value, line: usize) -> Result<Value, Error> {
        if matches!(value, Value::Object(_) | Value::Closure(_)) {
            return Ok(value);
        }
        let mut properties = PhpArray::default();
        match value {
            Value::Null => {}
            Value::Array(array) => {
                for (key, value) in array.entries().iter() {
                    properties.insert(ArrayKey::String(key.to_value().to_bytes()), value.clone());
                }
            }
            value => {
                value.warn_nan("object", line, self)?;
                properties.insert(ArrayKey::String(b"scalar".to_vec()), value);
            }
        }
        Ok(self.object_from_properties(properties))
    }

    pub(super) fn object_from_properties(&mut self, properties: PhpArray) -> Value {
        let handle = self.allocate_object_handle();
        Value::Object(self.create_object(crate::PhpObject {
            class: self.find_class(b"stdClass").expect("built-in stdClass"),
            class_name: "stdClass".to_owned(),
            properties: RefCell::new(properties),
            linked_list: RefCell::default(),
            object_storage: RefCell::default(),
            heap: RefCell::default(),
            dual_iterator: RefCell::default(),
            fixed_array: RefCell::default(),
            internal_iterator: RefCell::default(),
            recursive_iterator: RefCell::default(),
            array_iterator: RefCell::default(),
            properties_reordered: std::cell::Cell::new(false),
            property_cursor: RefCell::default(),
            property_iterators: RefCell::default(),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            reflection: RefCell::default(),
            object_id: handle.id,
            _handle: handle,
        }))
    }
}

fn out_of_range(number: f64) -> bool {
    !number.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&number)
}
