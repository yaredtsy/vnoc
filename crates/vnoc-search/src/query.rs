//! What the user typed, as a [`Query`] (`docs/20-search/01-query.md`).
//!
//! [`parse`] is pure and never fails: a filter whose value means nothing is not an error (the box is being typed in), it keeps no node and says
//! why in `notes`. A token that is not a filter is a word, so a half-typed `kind:` finds names until its value arrives.

/// A kind a `kind:` filter names. [`KindFilter::Function`] keeps every function, methods included; [`KindFilter::Method`] only those in a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KindFilter {
    Folder,
    File,
    Class,
    Function,
    Method,
}

/// The parse of a query string.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// Lower case, none empty.
    pub words: Vec<String>,
    /// Lower case, inner white space single, none empty.
    pub phrases: Vec<String>,
    /// The union of every `kind:`; none when the query has no `kind:`.
    pub kinds: Option<Vec<KindFilter>>,
    pub not_kinds: Vec<KindFilter>,
    /// `has:doc` is `Some(true)`, `no:doc` is `Some(false)`.
    pub docs: Option<bool>,
    /// A filter value that means nothing, or `has:doc` with `no:doc`: no node passes.
    pub keeps_nothing: bool,
    /// Paths a node must be inside, each: lower case, `/`, no leading `./`, no trailing `/`.
    pub inside: Vec<String>,
    pub not_inside: Vec<String>,
    /// Tags a doc must carry, each (`tag:plan`): lower case. A doc passes with every one of them; a node passes when ONE of its docs does.
    pub tags: Vec<String>,
    /// Tags a doc must not carry (`-tag:draft`).
    pub not_tags: Vec<String>,
    /// The author a doc must have (`author:claude-code`), each: lower case, compared ignoring case. Two different ones keep nothing.
    pub authors: Vec<String>,
    pub not_authors: Vec<String>,
    /// The token (less a call's `()` or `(`), when the whole query is one token shaped like a reference. It is a word too; the core resolves it.
    pub reference: Option<String>,
    /// What was wrong with a filter, each line once.
    pub notes: Vec<String>,
}

impl Query {
    /// Whether the query has a filter that narrows nodes. `keeps_nothing` is not one: it is checked first.
    pub(crate) fn has_filters(&self) -> bool {
        self.kinds.is_some()
            || !self.not_kinds.is_empty()
            || self.docs.is_some()
            || !self.inside.is_empty()
            || !self.not_inside.is_empty()
            || self.has_doc_filters()
    }

    /// Whether the query has a filter about a doc itself (`tag:`, `author:`): the nodes that have such a doc pass, and a query of filters alone lists
    /// those docs (`docs/21-agent-cli/04-write-commands.md` §4).
    pub fn has_doc_filters(&self) -> bool {
        !self.tags.is_empty()
            || !self.not_tags.is_empty()
            || !self.authors.is_empty()
            || !self.not_authors.is_empty()
    }

    /// A value that means nothing: no node passes, and `line` says why (once).
    fn note(&mut self, line: String) {
        self.keeps_nothing = true;
        if !self.notes.contains(&line) {
            self.notes.push(line);
        }
    }

    /// Reads `token` as a filter; false when it is not one (it is a word then).
    fn filter(&mut self, token: &str) -> bool {
        let (negated, rest) = match token.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, token),
        };
        let Some((name, value)) = rest.split_once(':') else {
            return false;
        };
        let value = value.trim();
        let name = lower(name);
        if value.is_empty()
            || !matches!(
                name.as_str(),
                "kind" | "has" | "no" | "in" | "tag" | "author"
            )
        {
            return false;
        }
        match name.as_str() {
            "kind" => self.kind(value, negated),
            "in" => {
                let path = path_of(value);
                if negated {
                    self.not_inside.push(path);
                } else {
                    self.inside.push(path);
                }
            }
            // a value is never wrong: a tag or an author nothing has finds nothing, and says nothing
            "tag" => {
                let list = if negated {
                    &mut self.not_tags
                } else {
                    &mut self.tags
                };
                add_once(list, lower(value));
            }
            "author" => {
                let list = if negated {
                    &mut self.not_authors
                } else {
                    &mut self.authors
                };
                add_once(list, lower(value));
            }
            _ => self.doc(&name, value, negated),
        }
        true
    }

    fn kind(&mut self, value: &str, negated: bool) {
        let items: Vec<String> = value
            .split(',')
            .map(|item| lower(item.trim()))
            .filter(|item| !item.is_empty())
            .collect();
        if items.is_empty() {
            self.note(not_a_kind(&lower(value)));
        }
        for item in items {
            let kind = match item.as_str() {
                "folder" => KindFilter::Folder,
                "file" => KindFilter::File,
                "class" | "cls" => KindFilter::Class,
                "function" | "fn" => KindFilter::Function,
                "method" => KindFilter::Method,
                _ => {
                    self.note(not_a_kind(&item));
                    continue;
                }
            };
            let list = if negated {
                &mut self.not_kinds
            } else {
                self.kinds.get_or_insert_with(Vec::new)
            };
            if !list.contains(&kind) {
                list.push(kind);
            }
        }
    }

    /// `has:` and `no:`; `-` turns either round.
    fn doc(&mut self, name: &str, value: &str, negated: bool) {
        let value = lower(value);
        if value != "doc" {
            self.note(format!(
                "`{name}:{value}` is not a filter value: has: and no: take doc"
            ));
            return;
        }
        let wants = (name == "has") != negated;
        match self.docs {
            Some(had) if had != wants => {
                self.note("`has:doc` and `no:doc` together keep nothing".to_string());
            }
            _ => self.docs = Some(wants),
        }
    }
}

fn add_once(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

fn not_a_kind(item: &str) -> String {
    format!("`kind:{item}` is not a kind (folder, file, class, function, method)")
}

/// Lower case, each char on its own as the names are matched (a few letters change length, and a whole-string lower-casing reads some in context).
fn lower(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// An `in:` value as a path: lower case, `/` for `\`, no leading `./`, no trailing `/`.
fn path_of(value: &str) -> String {
    let path = lower(value).replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    path.trim_end_matches('/').to_string()
}

/// A token as split: a `"…"` that is the whole token, or anything else (quotes taken out, a quoted space kept).
enum Token {
    Phrase(String),
    Plain(String),
}

/// Splits at white space, except inside double quotes. An unclosed quote is dropped, and the rest is read as it comes; `""` is dropped.
fn split(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    // a token is begun, and (`whole`) so far it is one quoted span and nothing else
    let (mut begun, mut whole) = (false, false);
    let mut flush = |current: &mut String, begun: &mut bool, whole: bool| {
        if *begun {
            let text = std::mem::take(current);
            if whole {
                let phrase = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !phrase.is_empty() {
                    tokens.push(Token::Phrase(lower(&phrase)));
                }
            } else {
                tokens.push(Token::Plain(text));
            }
        }
        *begun = false;
    };
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            flush(&mut current, &mut begun, whole);
        } else if c == '"' {
            // with no closing quote the quote is dropped, and the rest is read as it comes
            if let Some(len) = chars[at + 1..].iter().position(|&q| q == '"') {
                whole = !begun;
                begun = true;
                current.extend(&chars[at + 1..at + 1 + len]);
                at += len + 1;
            }
        } else {
            whole = false;
            begun = true;
            current.push(c);
        }
        at += 1;
    }
    flush(&mut current, &mut begun, whole);
    tokens
}

/// Whether `token` has the shape of a reference (01 §5): `::`, a `/`, a line (`:<digits>`, `:L<digits>`, `#L<digits>`, `#L<digits>-L<digits>`),
/// or 26 letters and digits with no `U`. Whether it names a node is the core's.
fn is_reference(token: &str) -> bool {
    if token.contains(char::is_whitespace) {
        return false;
    }
    let digits = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    let l_digits = |n: &str| n.strip_prefix(['L', 'l']).is_some_and(digits);
    let hash = token
        .rsplit_once('#')
        .is_some_and(|(_, f)| match f.split_once('-') {
            Some((a, b)) => l_digits(a) && l_digits(b),
            None => l_digits(f),
        });
    let line = hash
        || token
            .rsplit_once(':')
            .is_some_and(|(_, n)| digits(n) || l_digits(n));
    let id = token.len() == 26
        && token.bytes().all(|b| b.is_ascii_alphanumeric())
        && !token.bytes().any(|b| b.eq_ignore_ascii_case(&b'u'));
    token.contains("::") || token.contains('/') || line || id
}

/// A word as pasted from code: without the `()` or `(` of a call, which is no part of a name.
fn without_call(word: &str) -> &str {
    word.strip_suffix("()")
        .or_else(|| word.strip_suffix('('))
        .unwrap_or(word)
}

/// A traceback line, `File "<path>", line <n>` and anything after it, as the reference `<path>:<n>`.
fn traceback(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix("File \"")?;
    let (path, rest) = rest.split_once('"')?;
    let rest = rest.strip_prefix(", line ")?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    (!path.trim().is_empty() && digits > 0).then(|| format!("{path}:{}", &rest[..digits]))
}

/// Reads what the user typed (01). Pure, and never fails.
pub fn parse(text: &str) -> Query {
    if let Some(token) = traceback(text) {
        return Query {
            words: vec![lower(&token)],
            reference: Some(token),
            ..Query::default()
        };
    }
    let tokens = split(text);
    let mut query = Query::default();
    for token in &tokens {
        match token {
            Token::Phrase(phrase) => query.phrases.push(phrase.clone()),
            Token::Plain(text) => {
                if !query.filter(text) {
                    query.words.extend(
                        text.split_whitespace()
                            .map(without_call)
                            .filter(|w| !w.is_empty())
                            .map(lower),
                    );
                }
            }
        }
    }
    if let [Token::Plain(token)] = &tokens[..]
        && query.words.len() == 1
        && is_reference(without_call(token))
    {
        query.reference = Some(without_call(token).to_string());
    }
    query
}
