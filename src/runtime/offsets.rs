use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Access {
    Read,
    Coalesce,
    Probe,
    Empty,
    ProbeValue,
}

impl Access {
    pub(super) fn intermediate(self) -> Self {
        if matches!(self, Self::Probe | Self::Empty | Self::ProbeValue) {
            Self::ProbeValue
        } else {
            self
        }
    }
}

/// Keep the evaluated key and cache array coercion across read/modify/write.
/// String offsets use different conversion rules from array keys.
pub(super) struct Offset {
    value: Option<Value>,
    array_key: Option<ArrayKey>,
}
impl Offset {
    pub(super) fn is_append(&self) -> bool {
        self.value.is_none()
    }
    pub(super) fn new(value: Option<Value>) -> Self {
        Self {
            value,
            array_key: None,
        }
    }
    pub(super) fn value(&self, line: usize) -> Result<&Value, Error> {
        self.value
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::Error, "Cannot use [] for reading", line))
    }
    pub(super) fn array_key(
        &mut self,
        line: usize,
        unsetting: bool,
        warnings: &mut dyn WarningSink,
    ) -> Result<ArrayKey, Error> {
        if let Some(key) = &self.array_key {
            return Ok(key.clone());
        }
        let value = self.value(line)?;
        let key = if unsetting && *value == Value::Null {
            ArrayKey::String(Vec::new())
        } else {
            ArrayKey::from_value(value, line, warnings)?
        };
        self.array_key = Some(key.clone());
        Ok(key)
    }
}
pub(super) fn read_offset(
    value: Value,
    offset: &mut Offset,
    line: usize,
    access: Access,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    match value.dereferenced() {
        Value::Object(object) => Err(Error::new(
            ErrorKind::Error,
            format!("Cannot use object of type {} as array", object.class_name),
            line,
        )),
        Value::Closure(_) => Err(Error::new(
            ErrorKind::Error,
            "Cannot use object of type Closure as array",
            line,
        )),
        Value::Array(array) => {
            let key = offset.array_key(line, false, warnings)?;
            if let Some(value) = array.get(&key) {
                return Ok(value.dereferenced());
            }
            if access == Access::Read {
                warnings.push(format!(
                    "Undefined array key {} on line {line}",
                    String::from_utf8_lossy(&key.to_value().to_bytes())
                ));
            }
            Ok(Value::Null)
        }
        Value::String(bytes) => {
            let bytes = bytes.bytes();
            let Some(index) = string_index(offset.value(line)?, line, access, warnings)? else {
                return Ok(Value::Null);
            };
            if let Some(position) = position(index, bytes.len())
                && let Some(byte) = bytes.get(position)
            {
                return Ok(Value::String(vec![*byte].into()));
            }
            if access == Access::Read {
                warnings.push(format!(
                    "Uninitialized string offset {index} on line {line}"
                ));
                Ok(Value::String(Vec::new().into()))
            } else {
                Ok(Value::Null)
            }
        }
        _ => {
            if access == Access::Read {
                warnings.push(format!(
                    "Trying to access array offset on a scalar on line {line}"
                ));
            }
            Ok(Value::Null)
        }
    }
}
pub(super) fn string_index(
    key: &Value,
    line: usize,
    access: Access,
    warnings: &mut dyn WarningSink,
) -> Result<Option<i64>, Error> {
    match key {
        Value::Int(n) => return Ok(Some(*n)),
        Value::Float(_) | Value::Bool(_) | Value::Null => {
            if access == Access::Read {
                warnings.push(format!("String offset cast occurred on line {line}"));
            }
            if matches!(access, Access::Probe | Access::Empty | Access::ProbeValue)
                && matches!(key, Value::Float(_))
            {
                return Ok(Some(key.integer(line, warnings)?));
            }
            return Ok(Some(match key {
                Value::Float(n) => crate::value::float_to_int(*n),
                Value::Bool(b) => i64::from(*b),
                _ => 0,
            }));
        }
        Value::String(bytes) => {
            let bytes = bytes.bytes();
            if let Some((Value::Int(n), complete)) = crate::value::numeric_prefix(&bytes) {
                if !complete {
                    if matches!(access, Access::Probe | Access::Empty | Access::ProbeValue) {
                        return Ok(None);
                    }
                    warnings.push(format!(
                        "Illegal string offset \"{}\" on line {line}",
                        String::from_utf8_lossy(&bytes)
                    ));
                }
                return Ok(Some(n));
            }
        }
        _ => {}
    }
    if matches!(access, Access::Probe | Access::Empty | Access::ProbeValue)
        || (access == Access::Coalesce && !matches!(key, Value::Array(_)))
    {
        Ok(None)
    } else {
        Err(Error::new(
            ErrorKind::TypeError,
            "Cannot access offset of this type on string",
            line,
        ))
    }
}
pub(super) fn position(index: i64, length: usize) -> Option<usize> {
    if index < 0 {
        length.checked_sub(index.unsigned_abs().try_into().ok()?)
    } else {
        index.try_into().ok()
    }
}
pub(super) fn write_string(
    bytes: &mut Vec<u8>,
    key: &Value,
    new_value: Value,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    let index = string_index(key, line, Access::Read, warnings)?.unwrap();
    let Some(position) = position(index, bytes.len()) else {
        warnings.push(format!("Illegal string offset {index} on line {line}"));
        return Ok(Value::Null);
    };
    let replacement = string_bytes(&new_value, line, warnings)?;
    let Some(byte) = replacement.first().copied() else {
        return Err(Error::new(
            ErrorKind::Error,
            "Cannot assign an empty string to a string offset",
            line,
        ));
    };
    if replacement.len() > 1 {
        warnings.push(format!(
            "Only the first byte will be assigned to the string offset on line {line}"
        ));
    }
    assign_character(bytes, position, byte, line)?;
    Ok(Value::String(vec![byte].into()))
}

pub(super) fn assign_character(
    bytes: &mut Vec<u8>,
    position: usize,
    byte: u8,
    line: usize,
) -> Result<(), Error> {
    if position >= bytes.len() {
        let length = position
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
        bytes
            .try_reserve(length - bytes.len())
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        bytes.resize(length, b' ');
    }
    bytes[position] = byte;
    Ok(())
}
