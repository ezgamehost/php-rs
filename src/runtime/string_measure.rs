use super::*;

impl Runtime {
    pub(super) fn string_compare(
        &mut self,
        insensitive: bool,
        bounded: bool,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = match (insensitive, bounded) {
            (false, false) => "strcmp",
            (true, false) => "strcasecmp",
            (false, true) => "strncmp",
            (true, true) => "strncasecmp",
        };
        let left = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let right = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
        let length = if bounded {
            let length = self.native_int(args[2].as_ref().unwrap(), name, 3, line)?;
            if length < 0 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!("{name}(): Argument #3 ($length) must be greater than or equal to 0"),
                    line,
                ));
            }
            length as usize
        } else {
            usize::MAX
        };
        Ok(Value::Int(compare_bytes(
            &left,
            &right,
            length,
            insensitive,
        )))
    }

    pub(super) fn substring_compare(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let left = self.native_string(args[0].as_ref().unwrap(), "substr_compare", 1, line)?;
        let right = self.native_string(args[1].as_ref().unwrap(), "substr_compare", 2, line)?;
        let offset = self.native_int(args[2].as_ref().unwrap(), "substr_compare", 3, line)?;
        let length = args[3]
            .as_ref()
            .filter(|value| !matches!(value, Value::Null))
            .map(|value| self.native_int_type(value, "substr_compare", 4, line, "?int"))
            .transpose()?;
        let insensitive = args[4]
            .as_ref()
            .map(|value| self.native_bool(value, "substr_compare", 5, line))
            .transpose()?
            .unwrap_or(false);
        // PHP returns zero for an explicit zero length before checking the offset.
        if let Some(length) = length {
            if length == 0 {
                return Ok(Value::Int(0));
            }
            if length < 0 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "substr_compare(): Argument #4 ($length) must be greater than or equal to 0",
                    line,
                ));
            }
        }
        let offset = if offset < 0 {
            (left.len() as u64).saturating_sub(offset.unsigned_abs()) as usize
        } else {
            if offset as u64 > left.len() as u64 {
                return Err(range_error("substr_compare", 3, "offset", line));
            }
            offset as usize
        };
        Ok(Value::Int(compare_bytes(
            &left[offset..],
            &right,
            length.map_or(usize::MAX, |length| length as usize),
            insensitive,
        )))
    }

    pub(super) fn string_span(
        &mut self,
        included: bool,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = if included { "strspn" } else { "strcspn" };
        let bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let characters = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
        let offset = args[2]
            .as_ref()
            .map(|value| self.native_int(value, name, 3, line))
            .transpose()?
            .unwrap_or(0);
        let length = args[3]
            .as_ref()
            .filter(|value| !matches!(value, Value::Null))
            .map(|value| self.native_int_type(value, name, 4, line, "?int"))
            .transpose()?;
        let (start, end) = substrings::substring_range(bytes.len(), offset, length);
        let mut mask = [false; 256];
        for byte in characters {
            mask[byte as usize] = true;
        }
        let count = bytes[start..end]
            .iter()
            .take_while(|byte| mask[**byte as usize] == included)
            .count();
        Ok(Value::Int(count as i64))
    }

    pub(super) fn substring_count(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), "substr_count", 1, line)?;
        let needle = self.native_string(args[1].as_ref().unwrap(), "substr_count", 2, line)?;
        let offset = args[2]
            .as_ref()
            .map(|value| self.native_int(value, "substr_count", 3, line))
            .transpose()?
            .unwrap_or(0);
        let length = args[3]
            .as_ref()
            .filter(|value| !matches!(value, Value::Null))
            .map(|value| self.native_int_type(value, "substr_count", 4, line, "?int"))
            .transpose()?;
        if needle.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "substr_count(): Argument #2 ($needle) must not be empty",
                line,
            ));
        }
        if offset.unsigned_abs() > bytes.len() as u64 {
            return Err(range_error("substr_count", 3, "offset", line));
        }
        let start = if offset < 0 {
            bytes.len() - offset.unsigned_abs() as usize
        } else {
            offset as usize
        };
        let remaining = bytes.len() - start;
        if length.is_some_and(|length| length.unsigned_abs() > remaining as u64) {
            return Err(range_error("substr_count", 4, "length", line));
        }
        let length = match length {
            Some(length) if length < 0 => remaining - length.unsigned_abs() as usize,
            Some(length) => length as usize,
            None => remaining,
        };
        let bytes = &bytes[start..start + length];
        let mut cursor = 0;
        let mut count = 0;
        while let Some(offset) = bytes[cursor..]
            .windows(needle.len())
            .position(|window| window == needle)
        {
            count += 1;
            cursor += offset + needle.len();
        }
        Ok(Value::Int(count))
    }
}

fn range_error(function: &str, position: usize, parameter: &str, line: usize) -> Error {
    Error::new(
        ErrorKind::ValueError,
        format!(
            "{function}(): Argument #{position} (${parameter}) must be contained in argument #1 ($haystack)"
        ),
        line,
    )
}

fn compare_bytes(left: &[u8], right: &[u8], length: usize, insensitive: bool) -> i64 {
    for (&left, &right) in left.iter().zip(right).take(length) {
        let (left, right) = if insensitive {
            (left.to_ascii_lowercase(), right.to_ascii_lowercase())
        } else {
            (left, right)
        };
        if left != right {
            return i64::from(left) - i64::from(right);
        }
    }
    match left.len().min(length).cmp(&right.len().min(length)) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}
