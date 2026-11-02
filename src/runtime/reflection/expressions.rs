use super::*;
use crate::ast::{ArrayElement, CastKind};

pub(super) fn literal_expression(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal(_) => true,
        ExprKind::Group(inner) | ExprKind::Unary(_, inner) | ExprKind::Cast(_, inner) => {
            literal_expression(inner)
        }
        ExprKind::Binary(_, left, right) | ExprKind::Index(left, Some(right)) => {
            literal_expression(left) && literal_expression(right)
        }
        ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
            elements.iter().all(|e| match e {
                ArrayElement::Entry(key, value) => {
                    key.as_ref().is_none_or(literal_expression) && literal_expression(value)
                }
                ArrayElement::Unpack(value) => literal_expression(value),
                _ => false,
            })
        }
        ExprKind::Conditional(test, yes, no) => {
            literal_expression(test)
                && yes.as_ref().is_none_or(|e| literal_expression(e))
                && literal_expression(no)
        }
        _ => false,
    }
}

pub(super) fn expression(expr: &Expr, out: &mut Vec<u8>) {
    expression_prec(expr, 0, out);
}

fn expression_prec(expr: &Expr, priority: u16, out: &mut Vec<u8>) {
    match &expr.kind {
        ExprKind::Literal(value) => properties::format_value(value, out),
        ExprKind::Constant(name) => out.extend(name),
        ExprKind::NamespacedName(name) => out.extend(&name.constant),
        ExprKind::Group(inner) => expression_prec(inner, priority, out),
        ExprKind::Unary(op, inner) => {
            let precedence = if op == "!" { 220 } else { 240 };
            if priority > precedence {
                out.push(b'(');
            }
            out.extend(op.bytes());
            expression_prec(inner, precedence, out);
            if priority > precedence {
                out.push(b')');
            }
        }
        ExprKind::Cast(kind, inner) => {
            if priority > 240 {
                out.push(b'(');
            }
            out.extend(match kind {
                CastKind::Int => b"(int)".as_slice(),
                CastKind::Float => b"(float)",
                CastKind::Bool => b"(bool)",
                CastKind::String => b"(string)",
                CastKind::Array => b"(array)",
                CastKind::Object => b"(object)",
                CastKind::Void => b"(void)",
            });
            expression_prec(inner, 240, out);
            if priority > 240 {
                out.push(b')');
            }
        }
        ExprKind::Binary(op, left, right) => {
            let (precedence, pl, pr, op) = match op.as_str() {
                "+" | "-" => (200, 200, 201, op.as_str()),
                "*" | "/" | "%" => (210, 210, 211, op.as_str()),
                "<<" | ">>" => (190, 190, 191, op.as_str()),
                "." => (185, 185, 186, "."),
                "|" => (140, 140, 141, "|"),
                "&" => (160, 160, 161, "&"),
                "^" => (150, 150, 151, "^"),
                "===" | "!==" | "==" | "!=" => (170, 171, 171, op.as_str()),
                "<>" => (170, 171, 171, "!="),
                "<" | "<=" | ">" | ">=" | "<=>" => (180, 181, 181, op.as_str()),
                "**" => (250, 251, 250, "**"),
                "xor" => (40, 40, 41, "xor"),
                "&&" | "and" => (130, 130, 131, "&&"),
                "||" | "or" => (120, 120, 121, "||"),
                "??" => (110, 111, 110, "??"),
                _ => unreachable!("nonconstant binary operator"),
            };
            if priority > precedence {
                out.push(b'(');
            }
            expression_prec(left, pl, out);
            out.push(b' ');
            out.extend(op.bytes());
            out.push(b' ');
            expression_prec(right, pr, out);
            if priority > precedence {
                out.push(b')');
            }
        }
        ExprKind::ScopedMember(class, member) => {
            class_name(class, out);
            out.extend(b"::");
            member_name(member, out);
        }
        ExprKind::ClassName(class) => {
            class_name(class, out);
            out.extend(b"::class");
        }
        ExprKind::Property(object, member) | ExprKind::NullsafeProperty(object, member) => {
            expression(object, out);
            out.extend(if matches!(expr.kind, ExprKind::NullsafeProperty(..)) {
                b"?->".as_slice()
            } else {
                b"->"
            });
            member_name(member, out);
        }
        ExprKind::Callable(callee) => {
            if let ExprKind::NamespacedName(name) = &callee.kind {
                out.extend(&name.function);
            } else {
                expression(callee, out);
            }
            out.extend(b"(...)");
        }
        ExprKind::Lambda(lambda) => {
            out.extend(b"Closure(");
            out.extend(&lambda.function.name);
            out.push(b')');
        }
        ExprKind::New(class, arguments) => {
            out.extend(b"new ");
            class_name(class, out);
            out.push(b'(');
            for (i, argument) in arguments.iter().enumerate() {
                if i > 0 {
                    out.extend(b", ");
                }
                let value = match argument {
                    Argument::Positional(value) => value,
                    Argument::Named(name, value) => {
                        out.extend(name);
                        out.extend(b": ");
                        value
                    }
                    Argument::Unpack(value) => {
                        out.extend(b"...");
                        value
                    }
                };
                expression(value, out);
            }
            out.push(b')');
        }
        ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
            out.push(b'[');
            for (i, element) in elements.iter().enumerate() {
                if i > 0 {
                    out.extend(b", ");
                }
                match element {
                    ArrayElement::Entry(key, value) | ArrayElement::Reference(key, value) => {
                        if let Some(key) = key {
                            expression_prec(key, 80, out);
                            out.extend(b" => ");
                        }
                        expression_prec(value, 80, out);
                    }
                    ArrayElement::Unpack(value) => {
                        out.extend(b"...");
                        expression(value, out);
                    }
                    ArrayElement::Hole => {}
                }
            }
            out.push(b']');
        }
        ExprKind::Index(value, index) => {
            expression_prec(value, 260, out);
            out.push(b'[');
            if let Some(index) = index {
                expression(index, out);
            }
            out.push(b']');
        }
        ExprKind::Conditional(test, yes, no) => {
            if priority > 100 {
                out.push(b'(');
            }
            expression_prec(test, 100, out);
            if let Some(yes) = yes {
                out.extend(b" ? ");
                expression_prec(yes, 101, out);
                out.extend(b" : ");
            } else {
                out.extend(b" ?: ");
            }
            expression_prec(no, 101, out);
            if priority > 100 {
                out.push(b')');
            }
        }
        _ => unreachable!("nonconstant parameter default"),
    }
}

fn class_name(class: &Expr, out: &mut Vec<u8>) {
    let name = class.static_class_name().expect("constant class name");
    if ![b"self".as_slice(), b"parent", b"static"]
        .iter()
        .any(|keyword| name.eq_ignore_ascii_case(keyword))
    {
        out.push(b'\\');
    }
    out.extend(name);
}

fn member_name(member: &Expr, out: &mut Vec<u8>) {
    if let ExprKind::Literal(Value::String(name)) = &member.kind {
        out.extend(name.bytes());
    } else {
        out.push(b'{');
        expression(member, out);
        out.push(b'}');
    }
}
