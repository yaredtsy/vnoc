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
#[cfg(test)]
pub(crate) fn name_words(name: &str) -> Vec<(usize, usize)> {
    let mut words = Vec::new();
    words_into(name, &mut words);
    words
        .into_iter()
        .map(|(start, end)| (start as usize, end as usize))
        .collect()
}

/// [`name_words`], appended to `out`. The name is walked once, a char ahead of the one being looked at, with no list of its chars.
pub(crate) fn words_into(name: &str, out: &mut Vec<(u32, u32)>) {
    let mut chars = name.char_indices().map(|(at, c)| (at, class(c))).peekable();
    let mut start: Option<usize> = None;
    // the class of the char before the one being looked at: only read while a word is open, when that char was not a separator
    let mut before = Class::Separator;
    while let Some((at, now)) = chars.next() {
        if now == Class::Separator {
            if let Some(s) = start.take() {
                out.push((s as u32, at as u32));
            }
            continue;
        }
        if let Some(s) = start {
            let next = chars.peek().map(|c| c.1);
            let cut = match (before, now) {
                (Class::Lower, Class::Upper) => true,
                (Class::Upper, Class::Upper) => next == Some(Class::Lower),
                (Class::Digit, other) => other != Class::Digit,
                (other, Class::Digit) => other != Class::Digit,
                _ => false,
            };
            if cut {
                out.push((s as u32, at as u32));
                start = Some(at);
            }
        } else {
            start = Some(at);
        }
        before = now;
    }
    if let Some(s) = start {
        out.push((s as u32, name.len() as u32));
    }
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
