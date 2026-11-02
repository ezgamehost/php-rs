// Altered Rust adaptation of PHP 8.5.0 ext/random/engine_mt19937.c and random.c.
// See THIRD_PARTY_NOTICES.md for PHP and Mersenne Twister license notices.
use super::*;

#[derive(Default)]
pub(super) struct State {
    engine: Option<Box<Engine>>,
    legacy: bool,
}

struct Engine {
    words: [u32; 624],
    index: usize,
}

impl State {
    fn seed(&mut self, seed: u32) {
        let mut engine = Box::new(Engine {
            words: [0; 624],
            index: 624,
        });
        engine.words[0] = seed;
        for index in 1..624 {
            let previous = engine.words[index - 1];
            engine.words[index] = 1812433253u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(index as u32);
        }
        engine.reload(self.legacy);
        self.engine = Some(engine);
    }

    fn ensure_seeded(&mut self) {
        if self.engine.is_none() {
            self.legacy = false;
            self.seed(default_seed());
        }
    }

    fn generate(&mut self) -> u32 {
        self.ensure_seeded();
        let engine = self.engine.as_mut().unwrap();
        if engine.index == 624 {
            engine.reload(self.legacy);
        }
        let mut value = engine.words[engine.index];
        engine.index += 1;
        value ^= value >> 11;
        value ^= (value << 7) & 0x9d2c5680;
        value ^= (value << 15) & 0xefc60000;
        value ^ (value >> 18)
    }

    fn range(&mut self, min: i64, max: i64, line: usize) -> Result<i64, Error> {
        self.ensure_seeded();
        if self.legacy {
            let value = self.generate() >> 1;
            let offset = (max as f64 - min as f64 + 1.0) * (value as f64 / 2147483648.0);
            return Ok((offset as u64).wrapping_add(min as u64) as i64);
        }
        self.unbiased_range(min, max, line)
    }

    fn unbiased_range(&mut self, min: i64, max: i64, line: usize) -> Result<i64, Error> {
        let difference = (max as u64).wrapping_sub(min as u64);
        let wide = difference > u32::MAX as u64;
        let maximum = if wide { u64::MAX } else { u32::MAX as u64 };
        let range = difference.wrapping_add(1);
        for _ in 0..=50 {
            let mut value = self.generate() as u64;
            if wide {
                value |= (self.generate() as u64) << 32;
            }
            let offset = if difference == maximum {
                value
            } else if range.is_power_of_two() {
                value & (range - 1)
            } else if value < maximum - (maximum % range) {
                value % range
            } else {
                continue;
            };
            return Ok(offset.wrapping_add(min as u64) as i64);
        }
        Err(Error::new(
            ErrorKind::BrokenRandomEngineError,
            "Failed to generate an acceptable random number in 50 attempts",
            line,
        ))
    }
}

impl Engine {
    fn reload(&mut self, legacy: bool) {
        for index in 0..624 {
            let current = self.words[index];
            let next = self.words[(index + 1) % 624];
            let mixed = (current & 0x80000000) | (next & 0x7fffffff);
            let low = if legacy { current } else { next } & 1;
            self.words[index] = self.words[(index + 397) % 624]
                ^ (mixed >> 1)
                ^ (0u32.wrapping_sub(low) & 0x9908b0df);
        }
        self.index = 0;
    }
}

fn default_seed() -> u32 {
    let mut bytes = [0; 4];
    if random::fill(&mut bytes).is_ok() {
        return u32::from_ne_bytes(bytes);
    }
    // MT is not a cryptographic generator. Like PHP, it remains available when
    // OS entropy fails. Include a counter so rapid reseeds have distinct input.
    use std::hash::{Hash, Hasher};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hash);
    std::process::id().hash(&mut hash);
    COUNTER.fetch_add(1, Ordering::Relaxed).hash(&mut hash);
    hash.finish() as u32
}

impl Runtime {
    pub(super) fn shuffle_array(
        &mut self,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
    ) -> Result<Value, Error> {
        let first = args.into_iter().next().unwrap().unwrap();
        builtins::require_array(&first, "shuffle", 1, line)?;
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        sorting::with_array(&mut source.borrow_mut(), |array| {
            let array = Rc::make_mut(array);
            self.mt_state.ensure_seeded();
            array.shuffle(|last| {
                self.mt_state
                    .unbiased_range(0, last as i64, line)
                    .map(|n| n as usize)
            })
        })
        .unwrap()?;
        Ok(Value::Bool(true))
    }

    pub(super) fn shuffle_string(&mut self, value: &Value, line: usize) -> Result<Value, Error> {
        let mut bytes = self.native_string(value, "str_shuffle", 1, line)?;
        for last in (1..bytes.len()).rev() {
            let other = self.mt_state.unbiased_range(0, last as i64, line)? as usize;
            bytes.swap(last, other);
        }
        Ok(Value::String(bytes.into()))
    }

    pub(super) fn array_rand(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let array = builtins::require_array(args[0].as_ref().unwrap(), "array_rand", 1, line)?;
        let count = args[1]
            .as_ref()
            .map(|value| self.native_int(value, "array_rand", 2, line))
            .transpose()?
            .unwrap_or(1);
        self.mt_state.ensure_seeded();
        let length = array.entries().len();
        if length == 0 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "array_rand(): Argument #1 ($array) must not be empty",
                line,
            ));
        }
        if count < 1 || count as u64 > length as u64 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "array_rand(): Argument #2 ($num) must be between 1 and the number of elements in argument #1 ($array)",
                line,
            ));
        }
        if count == 1 {
            // PhpArray currently retains live insertion order, not Zend's
            // physical bucket holes. Sampling remains uniform, but a sparse
            // table's seeded sequence can differ until bucket history is kept.
            let index = self.mt_state.unbiased_range(0, length as i64 - 1, line)? as usize;
            return Ok(array.entries()[index].0.to_value());
        }
        let count = count as usize;
        let complement = count > length / 2;
        let wanted = if complement { length - count } else { count };
        let mut selected = HashSet::new();
        let mut failures = 0;
        while selected.len() < wanted {
            let index = self.mt_state.unbiased_range(0, length as i64 - 1, line)? as usize;
            if selected.insert(index) {
                failures = 0;
            } else {
                failures += 1;
                if failures > 50 {
                    return Err(Error::new(
                        ErrorKind::BrokenRandomEngineError,
                        "Failed to generate an acceptable random number in 50 attempts",
                        line,
                    ));
                }
            }
        }
        let mut result = PhpArray::with_capacity(count, line)?;
        for (index, (key, _)) in array.entries().iter().enumerate() {
            if selected.contains(&index) != complement {
                result.append(key.to_value(), line)?;
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn lcg_value(&mut self) -> Value {
        let state = self
            .lcg_state
            .get_or_insert_with(|| [default_seed() as i32, default_seed() as i32]);
        for (index, a, b, c, modulus) in [
            (0, 53668i64, 40014, 12211, 2147483563),
            (1, 52774i64, 40692, 3791, 2147483399),
        ] {
            let current = state[index] as i64;
            let quotient = current / a;
            let mut value = b * (current - a * quotient) - c * quotient;
            if value < 0 {
                value += modulus;
            }
            state[index] = value as i32;
        }
        let mut value = state[0] as i64 - state[1] as i64;
        if value < 1 {
            value += 2147483562;
        }
        Value::Float(value as f64 * 4.656613e-10)
    }

    pub(super) fn mt_seed(
        &mut self,
        args: &[Option<Value>],
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let seed = match args[0].as_ref() {
            None | Some(Value::Null) => None,
            Some(value) => Some(self.native_int(value, name, 1, line)? as u32),
        };
        let mode = args[1]
            .as_ref()
            .map(|v| self.native_int(v, name, 2, line))
            .transpose()?
            .unwrap_or(0);
        self.mt_state.legacy = mode == 1;
        if mode == 1 {
            self.warn(format!(
                "Deprecated: The MT_RAND_PHP variant of Mt19937 is deprecated on line {line}"
            ))?;
        }
        self.mt_state.seed(seed.unwrap_or_else(default_seed));
        Ok(Value::Null)
    }

    pub(super) fn mt_rand(
        &mut self,
        args: &[Option<Value>],
        name: &str,
        inverted: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if args[0].is_none() {
            return Ok(Value::Int((self.mt_state.generate() >> 1) as i64));
        }
        let mut min = self.native_int(args[0].as_ref().unwrap(), name, 1, line)?;
        let mut max = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
        if max < min {
            if !inverted {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "{name}(): Argument #2 ($max) must be greater than or equal to argument #1 ($min)"
                    ),
                    line,
                ));
            }
            std::mem::swap(&mut min, &mut max);
        }
        self.mt_state.range(min, max, line).map(Value::Int)
    }
}
