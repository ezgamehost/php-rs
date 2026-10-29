// Rust adaptation of PHP 8.5.0 Zend/zend_sort.c. Copyright Zend Technologies
// Ltd.; authors Xinchen Hui and Sterling Hughes. Includes LLVM libc++ code.
// See THIRD_PARTY_NOTICES.md for the original notices and licenses.
use std::cmp::Ordering;

// Preserve PHP's comparison order even for non-transitive PHP comparisons.
// Rust's slice sorting requires a total order and cannot be used here.
pub(super) fn sort<T>(values: &mut [T], compare: &mut impl FnMut(&T, &T) -> Ordering) {
    let mut values = values;
    loop {
        let len = values.len();
        if len <= 16 {
            insertion(values, compare);
            return;
        }
        let middle = len / 2;
        if len >= 1024 {
            small(
                values,
                &[0, middle / 2, middle, middle + middle / 2, len - 1],
                compare,
            );
        } else {
            small(values, &[0, middle, len - 1], compare);
        }
        values.swap(1, middle);
        let mut i = 2;
        let mut j = len - 1;
        'partition: loop {
            while compare(&values[1], &values[i]).is_gt() {
                i += 1;
                if i == j {
                    break 'partition;
                }
            }
            j -= 1;
            if j == i {
                break;
            }
            while compare(&values[j], &values[1]).is_gt() {
                j -= 1;
                if j == i {
                    break 'partition;
                }
            }
            values.swap(i, j);
            i += 1;
            if i == j {
                break;
            }
        }
        values.swap(1, i - 1);
        let (left, right) = values.split_at_mut(i);
        let left = &mut left[..i - 1];
        if left.len() < right.len() {
            sort(left, compare);
            values = right;
        } else {
            sort(right, compare);
            values = left;
        }
    }
}

fn small<T>(v: &mut [T], indices: &[usize], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    if indices.len() < 2 {
        return;
    }
    let a = indices[0];
    let b = indices[1];
    if indices.len() == 2 {
        if cmp(&v[a], &v[b]).is_gt() {
            v.swap(a, b);
        }
        return;
    }
    let c = indices[2];
    if !cmp(&v[a], &v[b]).is_gt() {
        if cmp(&v[b], &v[c]).is_gt() {
            v.swap(b, c);
            if cmp(&v[a], &v[b]).is_gt() {
                v.swap(a, b);
            }
        }
    } else if !cmp(&v[c], &v[b]).is_gt() {
        v.swap(a, c);
    } else {
        v.swap(a, b);
        if cmp(&v[b], &v[c]).is_gt() {
            v.swap(b, c);
        }
    }
    for n in 3..indices.len() {
        for k in (1..=n).rev() {
            let (a, b) = (indices[k - 1], indices[k]);
            if !cmp(&v[a], &v[b]).is_gt() {
                break;
            }
            v.swap(a, b);
        }
    }
}

fn insertion<T>(v: &mut [T], cmp: &mut impl FnMut(&T, &T) -> Ordering) {
    if v.len() <= 5 {
        let indices: Vec<_> = (0..v.len()).collect();
        small(v, &indices, cmp);
        return;
    }
    for i in 1..6 {
        let mut j = i - 1;
        if !cmp(&v[j], &v[i]).is_gt() {
            continue;
        }
        while j != 0 {
            j -= 1;
            if !cmp(&v[j], &v[i]).is_gt() {
                j += 1;
                break;
            }
        }
        for k in (j + 1..=i).rev() {
            v.swap(k, k - 1);
        }
    }
    for i in 6..v.len() {
        let mut j = i - 1;
        if !cmp(&v[j], &v[i]).is_gt() {
            continue;
        }
        loop {
            j -= 2;
            if !cmp(&v[j], &v[i]).is_gt() {
                j += 1;
                if !cmp(&v[j], &v[i]).is_gt() {
                    j += 1;
                }
                break;
            }
            if j == 0 {
                break;
            }
            if j == 1 {
                j = 0;
                if cmp(&v[i], &v[j]).is_gt() {
                    j += 1;
                }
                break;
            }
        }
        for k in (j + 1..=i).rev() {
            v.swap(k, k - 1);
        }
    }
}
