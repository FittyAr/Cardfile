//! Fractional indexing for manual card ordering.
//!
//! Keys are strings over the alphabet `0-9a-z` compared lexicographically. A key can
//! always be generated between any two keys without renumbering other cards, which
//! keeps reordering a single-row update (and friendly to future sync).
//!
//! Generated keys never end in `0` (the smallest digit): nothing would fit between
//! `x` and `x0`.
//!
//! Adding at either end of the list is the common case (new cards go on top), so it
//! does not bisect: halving the gap to the edge each time would add a character every
//! few cards. Instead the edge key is stepped by one in its last place, and only when a
//! "level" runs out (the leading digit would become the edge digit) does the key move
//! to the next level, whose keys are one character longer *and* have one more free
//! digit. Level `p` holds about `36^(p+1)` keys, so key length grows with the
//! logarithm of the number of cards added at an edge, not linearly.

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const BASE: usize = DIGITS.len();
const MIN: u8 = DIGITS[0];
const MAX: u8 = DIGITS[BASE - 1];

fn digit(c: u8) -> usize {
    DIGITS.iter().position(|&d| d == c).unwrap_or(0)
}

fn is_valid(key: &[u8]) -> bool {
    !key.is_empty() && key.iter().all(|c| DIGITS.contains(c))
}

fn to_key(digits: impl IntoIterator<Item = usize>) -> String {
    digits.into_iter().map(|d| char::from(DIGITS[d])).collect()
}

/// A key that sorts strictly between `a` and `b`.
///
/// `None` for `a` means "before everything", `None` for `b` means "after everything".
/// If `a >= b` the result sorts after `a`.
pub fn key_between(a: Option<&str>, b: Option<&str>) -> String {
    let a = a.unwrap_or("").as_bytes();
    let b = b.map(str::as_bytes).filter(|b| *b > a);
    let edge = match b {
        Some(b) if a.is_empty() => before(b),
        None if !a.is_empty() => after(a),
        _ => None,
    };
    if let Some(key) = edge {
        return key;
    }
    let mut out = Vec::new();
    midpoint(a, b, &mut out);
    String::from_utf8(out).unwrap_or_default()
}

/// A short key just below `b` (see the module docs), or `None` when `b` is not a key
/// this module could have produced.
fn before(b: &[u8]) -> Option<String> {
    if !is_valid(b) {
        return None;
    }
    let level = b.iter().take_while(|&&c| c == MIN).count();
    let rest = &b[level..];
    if rest.is_empty() {
        return None;
    }
    // Step down the first `level + 1` digits after the shared run of `0`s. Truncating
    // `rest` already sorts at or below it, so the result is strictly below `b`.
    let mut t: Vec<usize> = rest.iter().take(level + 1).map(|&c| digit(c)).collect();
    loop {
        for d in t.iter_mut().rev() {
            if *d > 0 {
                *d -= 1;
                break;
            }
            *d = BASE - 1;
        }
        if t.first().is_none_or(|&d| d == 0) {
            // Level exhausted: one more leading `0`, and a longer run of free digits.
            let zeros = std::iter::repeat_n(0, level + 1);
            let tops = std::iter::repeat_n(BASE - 1, level + 2);
            return Some(to_key(zeros.chain(tops)));
        }
        if t.last() != Some(&0) {
            break;
        }
    }
    let mut key: String = std::iter::repeat_n(char::from(MIN), level).collect();
    key.push_str(&to_key(t));
    Some(key)
}

/// A short key just above `a` (see the module docs), or `None` when `a` is not a key
/// this module could have produced.
fn after(a: &[u8]) -> Option<String> {
    if !is_valid(a) {
        return None;
    }
    let level = a.iter().take_while(|&&c| c == MAX).count();
    let rest = &a[level..];
    if rest.is_empty() {
        return None;
    }
    // Step up the first `level + 1` digits after the shared run of `z`s; any digit
    // that grows makes the key sort above `a`, whatever followed it.
    let mut t: Vec<usize> = rest.iter().take(level + 1).map(|&c| digit(c)).collect();
    loop {
        for d in t.iter_mut().rev() {
            if *d < BASE - 1 {
                *d += 1;
                break;
            }
            *d = 0;
        }
        if t.first().is_none_or(|&d| d == BASE - 1) {
            // Level exhausted: one more leading `z`, then the lowest key of the level.
            let tops = std::iter::repeat_n(BASE - 1, level + 1);
            let zeros = std::iter::repeat_n(0, level + 1);
            return Some(to_key(tops.chain(zeros).chain([1])));
        }
        if t.last() != Some(&0) {
            break;
        }
    }
    let mut key: String = std::iter::repeat_n(char::from(MAX), level).collect();
    key.push_str(&to_key(t));
    Some(key)
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

    fn assert_valid(key: &str) {
        assert!(is_valid(key.as_bytes()), "{key}");
        assert!(!key.ends_with('0'), "{key}");
    }

    #[test]
    fn prepending_keeps_keys_short() {
        let mut first = key_after(None);
        for _ in 0..20_000 {
            let k = key_between(None, Some(&first));
            assert!(k < first, "{k} < {first}");
            assert_valid(&k);
            first = k;
        }
        assert!(first.len() <= 6, "{first}");
    }

    #[test]
    fn appending_keeps_keys_short() {
        let mut last = key_after(None);
        for _ in 0..20_000 {
            let k = key_after(Some(&last));
            assert!(k > last, "{k} > {last}");
            assert_valid(&k);
            last = k;
        }
        assert!(last.len() <= 6, "{last}");
    }

    #[test]
    fn edges_of_legacy_keys() {
        // Keys written by the previous (bisecting) generator stay usable.
        for key in [
            "i", "9", "1", "0i", "00i", "0001", "z", "zi", "zzzi", "4ab", "0z0z",
        ] {
            let lo = key_between(None, Some(key));
            assert!(lo.as_str() < key, "{lo} < {key}");
            assert_valid(&lo);
            let hi = key_between(Some(key), None);
            assert!(hi.as_str() > key, "{hi} > {key}");
            assert_valid(&hi);
        }
    }

    #[test]
    fn random_insertions_keep_order() {
        // Deterministic pseudo-random positions (no extra test dependency).
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut keys: Vec<String> = Vec::new();
        for _ in 0..3_000 {
            let len = keys.len() as u64;
            // Favour the edges, like real use.
            let pos = match next() % 4 {
                0 => 0,
                1 => len,
                _ => next() % (len + 1),
            } as usize;
            let lo = pos.checked_sub(1).map(|i| keys[i].as_str());
            let hi = keys.get(pos).map(String::as_str);
            let k = key_between(lo, hi);
            assert_valid(&k);
            assert!(lo.is_none_or(|lo| lo < k.as_str()), "{lo:?} < {k}");
            assert!(hi.is_none_or(|hi| k.as_str() < hi), "{k} < {hi:?}");
            keys.insert(pos, k);
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
