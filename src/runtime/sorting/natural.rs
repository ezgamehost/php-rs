// Altered Rust adaptation of PHP's strnatcmp.c, copyright (C) 2000 Martin
// Pool <mbp@humbug.org.au>, modified for PHP by Andrei Zmievski.
// The original license notice is retained in THIRD_PARTY_NOTICES.md.
use std::cmp::Ordering;

pub(super) fn compare(mut a: &[u8], mut b: &[u8], insensitive: bool) -> Ordering {
    if a.is_empty() || b.is_empty() {
        return a.len().cmp(&b.len());
    }
    while a.len() > 1 && a[0] == b'0' && a[1].is_ascii_digit() {
        a = &a[1..];
    }
    while b.len() > 1 && b[0] == b'0' && b[1].is_ascii_digit() {
        b = &b[1..];
    }
    loop {
        while a.first().is_some_and(|c| matches!(c, b' ' | b'\t'..=b'\r')) {
            a = &a[1..];
        }
        while b.first().is_some_and(|c| matches!(c, b' ' | b'\t'..=b'\r')) {
            b = &b[1..];
        }
        let mut ca = a.first().copied().unwrap_or(0);
        let mut cb = b.first().copied().unwrap_or(0);
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let fractional = ca == b'0' || cb == b'0';
            let mut bias = Ordering::Equal;
            loop {
                match (
                    a.first().is_some_and(u8::is_ascii_digit),
                    b.first().is_some_and(u8::is_ascii_digit),
                ) {
                    (false, false) => break,
                    (false, true) => return Ordering::Less,
                    (true, false) => return Ordering::Greater,
                    _ => {}
                }
                let order = a[0].cmp(&b[0]);
                if fractional && order != Ordering::Equal {
                    return order;
                }
                if bias == Ordering::Equal {
                    bias = order;
                }
                a = &a[1..];
                b = &b[1..];
            }
            if bias != Ordering::Equal {
                return bias;
            }
            if a.is_empty() || b.is_empty() {
                return a.len().cmp(&b.len());
            }
            ca = a[0];
            cb = b[0];
        }
        if insensitive {
            ca = ca.to_ascii_uppercase();
            cb = cb.to_ascii_uppercase();
        }
        let order = ca.cmp(&cb);
        if order != Ordering::Equal {
            return order;
        }
        a = a.get(1..).unwrap_or_default();
        b = b.get(1..).unwrap_or_default();
        if a.is_empty() || b.is_empty() {
            return a.len().cmp(&b.len());
        }
    }
}
