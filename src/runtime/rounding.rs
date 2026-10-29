use super::*;
use crate::object::Class;

pub(super) fn default_mode() -> Expr {
    Expr {
        kind: ExprKind::ScopedMember(
            Box::new(Expr {
                kind: ExprKind::Constant(b"RoundingMode".to_vec()),
                line: 0,
            }),
            Box::new(Expr {
                kind: ExprKind::Literal(Value::String(b"HalfAwayFromZero".to_vec().into())),
                line: 0,
            }),
        ),
        line: 0,
    }
}

impl Runtime {
    pub(super) fn rounding_mode_class(&self) -> Rc<Class> {
        let source = b"<?php enum RoundingMode {
            case HalfAwayFromZero; case HalfTowardsZero; case HalfEven; case HalfOdd;
            case TowardsZero; case AwayFromZero; case NegativeInfinity; case PositiveInfinity;
        }";
        let (mut statements, _, _) =
            parser::parse(lexer::lex(source).expect("RoundingMode source"))
                .expect("RoundingMode declaration");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        Rc::make_mut(&mut declaration).line = 0;
        Rc::make_mut(&mut declaration).end_line = 0;
        let declaration = self.prepare_enum(declaration).expect("native enum");
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            properties: declaration.properties.clone(),
            declaration,
            parent: None,
            interfaces: vec![self.find_class(b"UnitEnum").expect("UnitEnum")],
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: true,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(b"roundingmode".to_vec(), class.clone());
        class
    }

    pub(super) fn round(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        let value = self.native_number(args[0].as_ref().unwrap(), "round", 1, "int|float", line)?;
        let precision = args[1]
            .as_ref()
            .map_or(Ok(0), |n| self.native_int(n, "round", 2, line))?;
        let mode = match args[2].as_ref() {
            None => 1,
            Some(Value::Object(object)) if object.class.name == b"RoundingMode" => {
                let properties = object.properties.borrow();
                let Some(Value::String(name)) = properties.get(&ArrayKey::String(b"name".to_vec()))
                else {
                    unreachable!()
                };
                match name.bytes().as_slice() {
                    b"HalfAwayFromZero" => 1,
                    b"HalfTowardsZero" => 2,
                    b"HalfEven" => 3,
                    b"HalfOdd" => 4,
                    b"PositiveInfinity" => 5,
                    b"NegativeInfinity" => 6,
                    b"TowardsZero" => 7,
                    b"AwayFromZero" => 8,
                    _ => unreachable!(),
                }
            }
            Some(n) => self.native_int_type(n, "round", 3, line, "RoundingMode|int")?,
        };
        if !(1..=8).contains(&mode) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "round(): Argument #3 ($mode) must be a valid rounding mode (RoundingMode::*)",
                line,
            ));
        }
        let places = precision.clamp(i64::from(i32::MIN + 1), i64::from(i32::MAX)) as i32;
        if matches!(value, Value::Int(_)) && places >= 0 {
            return Ok(Value::Float(value.as_float()));
        }
        Ok(Value::Float(decimal_round(value.as_float(), places, mode)))
    }
}

pub(super) fn decimal_round(value: f64, places: i32, mode: i64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let places = places.max(i32::MIN + 1);
    let powers = [
        1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
        1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
    ];
    let exponent = powers
        .get(places.unsigned_abs() as usize)
        .copied()
        .unwrap_or_else(|| 10.0_f64.powf(f64::from(places.abs())));
    let scaled = if places > 0 {
        value * exponent
    } else {
        value / exponent
    };
    let mut integral = if value >= 0.0 {
        scaled.floor()
    } else {
        scaled.ceil()
    };
    let corrected = integral + 1.0_f64.copysign(integral);
    let unscaled = |n| {
        if places > 0 {
            n / exponent
        } else {
            n * exponent
        }
    };
    // Correct multiplication/division drift only when reversing the neighboring
    // integral value recovers the original input exactly.
    if unscaled(corrected) == value {
        integral = corrected;
    }
    if integral.abs() >= 1e16 {
        return value;
    }
    let half = unscaled(integral + 0.5_f64.copysign(integral)).abs();
    let edge = unscaled(integral).abs();
    let magnitude = value.abs();
    let increase = match mode {
        1 => magnitude >= half,
        2 => magnitude > half,
        3 => magnitude > half || (magnitude == half && integral % 2.0 != 0.0),
        4 => magnitude > half || (magnitude == half && integral % 2.0 == 0.0),
        5 => value > 0.0 && magnitude > edge,
        6 => value < 0.0 && magnitude > edge,
        7 => false,
        8 => magnitude > edge,
        _ => unreachable!(),
    };
    if increase {
        integral += 1.0_f64.copysign(integral);
    }
    if places.abs() < 23 {
        return unscaled(integral);
    }
    // Decimal parsing avoids a second binary rounding step for large powers.
    let result = format!("{integral:.6}e{}", -places)
        .parse::<f64>()
        .unwrap_or(f64::NAN);
    if result.is_finite() { result } else { value }
}
