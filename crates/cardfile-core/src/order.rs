//! Fractional indexing for manual card ordering.
//!
//! Keys are strings over the alphabet `0-9a-z` compared lexicographically. A key can
//! always be generated between any two keys without renumbering other cards, which
//! keeps reordering a single-row update (and friendly to future sync).

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: usize = DIGITS.len();

fn digit(c: u8) -> usize {
    DIGITS.iter().position(|&d| d == c).unwrap_or(0)
}

/// A key that sorts strictly between `a` and `b`.
///
/// `None` for `a` means "before everything", `None` for `b` means "after everything".
/// If `a >= b` the result sorts after `a`.
pub fn key_between(a: Option<&str>, b: Option<&str>) -> String {
    let a = a.unwrap_or("").as_bytes();
    let b = b.map(str::as_bytes).filter(|b| *b > a);
    let mut out = Vec::new();
    midpoint(a, b, &mut out);
    String::from_utf8(out).unwrap_or_default()
}

/// Append to `out` a digit string strictly between `a` and `b` (`b = None` is +∞).
/// Missing digits of `a` count as `0`, so generated keys never end in `0`.
fn midpoint(a: &[u8], b: Option<&[u8]>, out: &mut Vec<u8>) {
    if let Some(b) = b {
        // Copy the shared prefix (padding `a` with zeros).
        let mut n = 0;
        while n < b.len() && a.get(n).copied().unwrap_or(DIGITS[0]) == b[n] {
            n += 1;
        }
        if n > 0 {
            out.extend_from_slice(&b[..n]);
            return midpoint(a.get(n..).unwrap_or(&[]), Some(&b[n..]), out);
        }
    }
    let da = a.first().map_or(0, |&c| digit(c));
    let db = b.and_then(|b| b.first()).map_or(BASE, |&c| digit(c));
    if db - da > 1 {
        out.push(DIGITS[(da + db) / 2]);
    } else if let Some(b) = b.filter(|b| b.len() > 1) {
        // `b[0]` alone sorts above `a` and below the longer `b`.
        out.push(b[0]);
    } else {
        out.push(DIGITS[da]);
        midpoint(a.get(1..).unwrap_or(&[]), None, out);
    }
}

/// The key for a card appended after `last`.
pub fn key_after(last: Option<&str>) -> String {
    key_between(last, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_ordered_keys() {
        let first = key_after(None);
        let second = key_after(Some(&first));
        assert!(first < second);
        let mid = key_between(Some(&first), Some(&second));
        assert!(first < mid && mid < second, "{first} < {mid} < {second}");
        let before = key_between(None, Some(&first));
        assert!(before < first);
    }

    #[test]
    fn repeated_insertion_keeps_order() {
        let a = key_after(None);
        let mut b = key_after(Some(&a));
        for _ in 0..200 {
            let m = key_between(Some(&a), Some(&b));
            assert!(a < m && m < b, "{a} < {m} < {b}");
            b = m;
        }
        let mut lo = a.clone();
        let hi = key_after(Some(&a));
        for _ in 0..200 {
            let m = key_between(Some(&lo), Some(&hi));
            assert!(lo < m && m < hi, "{lo} < {m} < {hi}");
            lo = m;
        }
    }

    #[test]
    fn adjacent_digits() {
        let m = key_between(Some("a"), Some("b"));
        assert!("a" < m.as_str() && m.as_str() < "b", "{m}");
        let m = key_between(Some("az"), Some("b"));
        assert!("az" < m.as_str() && m.as_str() < "b", "{m}");
    }
}
