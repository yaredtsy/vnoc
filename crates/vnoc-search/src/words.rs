//! A name split into words.
//!
//! `checkout_order`, `checkoutOrder` and `CheckoutOrder` are all `checkout` `order`; the fuzzy tier of `names.rs` matches the letters of a
//! query against the starts of these words. The words come back as byte ranges into the name, so the match can point at the name as written.

/// What a char is, for the purpose of cutting words.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Lower,
    Upper,
    Digit,
    /// A letter with no case (`日`), or any other char that is not a separator. It never starts a cut by itself.
    Other,
    Separator,
}

fn class(c: char) -> Class {
    if matches!(c, '_' | '.' | '-') || c.is_whitespace() {
        Class::Separator
    } else if c.is_uppercase() {
        Class::Upper
    } else if c.is_lowercase() {
        Class::Lower
    } else if c.is_numeric() {
        Class::Digit
    } else {
        Class::Other
    }
}

/// The words of `name` as `(start, end)` byte ranges, in order. Cut at `_` `.` `-` and white space (dropped), between a lower-case letter and an
/// upper-case one, before the last capital of a run of capitals that a lower-case letter follows (`HTTPClient` is `HTTP` `Client`), and between
/// a digit and a letter. Empty words are dropped.
pub(crate) fn name_words(name: &str) -> Vec<(usize, usize)> {
    let chars: Vec<(usize, char, Class)> = name
        .char_indices()
        .map(|(at, c)| (at, c, class(c)))
        .collect();
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &(at, _, now)) in chars.iter().enumerate() {
        if now == Class::Separator {
            if let Some(s) = start.take() {
                words.push((s, at));
            }
            continue;
        }
        if let Some(s) = start {
            let before = chars[i - 1].2;
            let next = chars.get(i + 1).map(|c| c.2);
            let cut = match (before, now) {
                (Class::Lower, Class::Upper) => true,
                (Class::Upper, Class::Upper) => next == Some(Class::Lower),
                (Class::Digit, other) => other != Class::Digit,
                (other, Class::Digit) => other != Class::Digit,
                _ => false,
            };
            if cut {
                words.push((s, at));
                start = Some(at);
            }
        } else {
            start = Some(at);
        }
    }
    if let Some(s) = start {
        words.push((s, name.len()));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::name_words;

    fn split(name: &str) -> Vec<&str> {
        name_words(name)
            .into_iter()
            .map(|(s, e)| &name[s..e])
            .collect()
    }

    #[test]
    fn rows() {
        let rows: &[(&str, &[&str])] = &[
            ("checkout_order", &["checkout", "order"]),
            ("checkoutOrder", &["checkout", "Order"]),
            ("HTTPClient", &["HTTP", "Client"]),
            ("sha256sum", &["sha", "256", "sum"]),
            ("__init__", &["init"]),
            ("_private", &["private"]),
            ("a.b-c d", &["a", "b", "c", "d"]),
            ("", &[]),
        ];
        for (name, want) in rows {
            assert_eq!(&split(name), want, "{name:?}");
        }
    }

    #[test]
    fn ranges_are_bytes_of_the_name() {
        assert_eq!(name_words("café_order"), vec![(0, 5), (6, 11)]);
    }
}
