//! `NodeId`: a 128-bit, deterministic node ID written as 26 characters of Crockford base32. It is **not** a ULID: it
//! carries no time and no randomness, it is only printed in the same alphabet and bit layout.
//!
//! # The format (final from the MVP on)
//!
//! **Making an ID.** Hash with BLAKE3 in key-derivation mode, context string `"v-noc node id v1"`. The input is the
//! UTF-8 bytes of
//!
//! ```text
//! repo_path  0x00  qualname  0x00  occurrence
//! ```
//!
//! and, only after a collision, `0x00` followed by the attempt number. Take the **first 16 bytes** of the output and read
//! them as a **big-endian 128-bit integer**.
//!
//! * `repo_path`: relative to the git root, `/` separators, **exactly as git's index records it** (on macOS that is
//!   precomposed Unicode, NFC; the file system returns decomposed names, so read it from git, never from the disk).
//! * `qualname`: the language's own qualified name. For Python, `__qualname__`, **including `<locals>`**
//!   (`outer.<locals>.inner`). Empty for a file.
//! * `occurrence`: decimal, **1-based**, counted among definitions with the same `qualname` in the same file, in source order.
//! * **Collision:** if the ID is already taken (VS-D71's check), append `0x00` and the attempt number in decimal,
//!   **starting at 2**, and hash again until a free ID comes out. Attempt 1 has no counter.
//!
//! **Writing an ID.** Crockford base32, **26 characters, uppercase**, from the 128-bit number padded to 130 bits (two
//! zero bits in front), so the **first character is `0`-`7`**. Alphabet: `0123456789ABCDEFGHJKMNPQRSTVWXYZ`.
//! **Reading an ID** is case-insensitive and forgiving the way Crockford is: `I` and `L` read as `1`, `O` reads as `0`.
//! `U` is not in the alphabet and is rejected.
//!
//! **Shard** (the `.vnoc/nodes/<shard>/<id>/` folder): the **last two characters**, uppercase. The last characters carry
//! five bits each, so the shards fill evenly (the first character carries only three bits).
//!
//! **In code** (Python): a line `ID: <id>` in the def's or class's docstring. Nothing else; there is no comment form.
//!
//! # Who may create one
//!
//! A `NodeId` exists only if it was **read** (from code, from a `.vnoc/` folder name, from `node.toml`, from a doc's
//! frontmatter) or made by [`persist`](crate::persist), which writes it into the code before returning it. A node that has
//! not been persisted is a [`LocationKey`](crate::LocationKey). If code needs the hash of a node that has no ID yet (to
//! de-duplicate a canvas card, say), it uses a [`ProvisionalId`], which cannot be stored or serialized.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::location::LocationKey;

/// The key-derivation context. Changing it changes every ID: that is a new format version, not an edit.
const CONTEXT: &str = "v-noc node id v1";
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const LEN: usize = 26;

/// A node's permanent identity. See the module docs for the format and for who may create one.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u128);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NodeIdError {
    #[error("a node ID is {LEN} characters, got {0}")]
    WrongLength(usize),
    #[error("`{0}` is not a Crockford base32 character")]
    BadChar(char),
    #[error("the first character of a node ID is 0-7 (it carries only three bits)")]
    FirstCharTooLarge,
}

impl NodeId {
    /// Reads an ID written in code, in a folder name, in `node.toml` or in a doc's frontmatter.
    ///
    /// Case-insensitive; `I` and `L` read as `1`, `O` as `0`. The canonical text ([`Display`](fmt::Display)) is uppercase.
    pub fn parse(text: &str) -> Result<NodeId, NodeIdError> {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() != LEN {
            return Err(NodeIdError::WrongLength(chars.len()));
        }
        let mut value: u128 = 0;
        for (i, c) in chars.iter().enumerate() {
            let digit = digit_of(*c).ok_or(NodeIdError::BadChar(*c))?;
            if i == 0 && digit > 7 {
                return Err(NodeIdError::FirstCharTooLarge);
            }
            value = (value << 5) | u128::from(digit);
        }
        Ok(NodeId(value))
    }

    /// The shard folder name: the last two characters of the canonical (uppercase) text.
    pub fn shard(&self) -> String {
        let s = self.to_string();
        s[LEN - 2..].to_string()
    }

    /// The 128-bit number behind the text.
    pub fn as_u128(&self) -> u128 {
        self.0
    }

    /// Makes the ID of a location. **Crate-private on purpose:** the public way to make one is [`persist`](crate::persist),
    /// which writes the ID into the code before handing it out. `attempt` 1 has no counter; from 2 on it is appended.
    pub(crate) fn derive(location: &LocationKey, attempt: u32) -> NodeId {
        let mut input: Vec<u8> = Vec::new();
        input.extend_from_slice(location.path().as_str().as_bytes());
        input.push(0);
        input.extend_from_slice(location.qualname().as_bytes());
        input.push(0);
        input.extend_from_slice(location.occurrence().to_string().as_bytes());
        if attempt >= 2 {
            input.push(0);
            input.extend_from_slice(attempt.to_string().as_bytes());
        }
        let out = blake3::derive_key(CONTEXT, &input);
        NodeId(u128::from_be_bytes(out[..16].try_into().expect("a blake3 output has 32 bytes")))
    }

    #[cfg(test)]
    pub(crate) fn from_u128(n: u128) -> NodeId {
        NodeId(n)
    }
}

fn digit_of(c: char) -> Option<u8> {
    let c = match c.to_ascii_uppercase() {
        'I' | 'L' => '1',
        'O' => '0',
        other => other,
    };
    ALPHABET.iter().position(|&a| a as char == c).map(|p| p as u8)
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = [0u8; LEN];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = ALPHABET[((self.0 >> (5 * (LEN - 1 - i))) & 31) as usize];
        }
        f.write_str(std::str::from_utf8(&out).expect("the alphabet is ASCII"))
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({self})")
    }
}

impl Serialize for NodeId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for NodeId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        NodeId::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// The hash a node **would** get, for a node that has no ID yet. It exists so that in-memory code can tell two cards
/// apart without inventing an identity.
///
/// It **cannot be stored or serialized**: no `Display`, no serde, and its `Debug` hides the value. It ignores collisions (it
/// never looks at what is taken), so it may differ from the ID that [`persist`](crate::persist) finally makes.
///
/// ```compile_fail
/// fn must_serialize<T: serde::Serialize>() {}
/// must_serialize::<vnoc_domain::ProvisionalId>(); // does not compile: a provisional ID is never saved
/// ```
///
/// ```compile_fail
/// fn must_display<T: std::fmt::Display>() {}
/// must_display::<vnoc_domain::ProvisionalId>(); // does not compile: it has no text form
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProvisionalId(u128);

impl ProvisionalId {
    pub fn of(location: &LocationKey) -> ProvisionalId {
        ProvisionalId(NodeId::derive(location, 1).0)
    }

    /// Is this the ID that was (or will be) made for `id`'s location, first attempt? (An in-memory comparison only.)
    pub fn is_first_attempt_of(&self, id: &NodeId) -> bool {
        self.0 == id.0
    }
}

impl fmt::Debug for ProvisionalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProvisionalId(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::ENCODE_CASES;

    #[test]
    fn encodes_to_the_oracles_text_and_back() {
        for (n, text) in ENCODE_CASES {
            assert_eq!(NodeId::from_u128(*n).to_string(), *text, "encode {n}");
            assert_eq!(NodeId::parse(text).unwrap().as_u128(), *n, "decode {text}");
        }
    }

    #[test]
    fn the_first_character_is_zero_to_seven() {
        assert_eq!(NodeId::from_u128(u128::MAX).to_string().chars().next(), Some('7'));
        assert_eq!(NodeId::parse("80000000000000000000000000"), Err(NodeIdError::FirstCharTooLarge));
        assert_eq!(NodeId::parse("ZZZZZZZZZZZZZZZZZZZZZZZZZZ"), Err(NodeIdError::FirstCharTooLarge));
    }

    #[test]
    fn reading_is_case_insensitive_and_maps_look_alikes() {
        let canonical = NodeId::parse("3RGJ87CMS6TK775XRJV8BW977Y").unwrap();
        assert_eq!(NodeId::parse("3rgj87cms6tk775xrjv8bw977y").unwrap(), canonical, "lowercase");
        // I and L read as 1, O reads as 0
        assert_eq!(NodeId::parse("0I00000000000000000000000L").unwrap(), NodeId::parse("01000000000000000000000001").unwrap());
        assert_eq!(NodeId::parse("0O00000000000000000000000o").unwrap(), NodeId::parse("00000000000000000000000000").unwrap());
        assert_eq!(canonical.to_string(), "3RGJ87CMS6TK775XRJV8BW977Y", "the canonical text is uppercase");
    }

    #[test]
    fn rejects_what_is_not_an_id() {
        assert_eq!(NodeId::parse(""), Err(NodeIdError::WrongLength(0)));
        assert_eq!(NodeId::parse("3RGJ87CMS6TK775XRJV8BW977"), Err(NodeIdError::WrongLength(25)));
        assert_eq!(NodeId::parse("3RGJ87CMS6TK775XRJV8BW977YY"), Err(NodeIdError::WrongLength(27)));
        assert_eq!(NodeId::parse("3RGJ87CMS6TK775XRJV8BW977U"), Err(NodeIdError::BadChar('U')), "U is not in the alphabet");
        assert_eq!(NodeId::parse("3RGJ87CMS6TK775XRJV8BW977-"), Err(NodeIdError::BadChar('-')));
        // a UUID (the old V-NOC format) is not a node ID
        assert!(NodeId::parse("123e4567-e89b-12d3-a456-426614174000").is_err());
    }

    #[test]
    fn the_shard_is_the_last_two_characters_uppercase() {
        assert_eq!(NodeId::parse("3rgj87cms6tk775xrjv8bw977y").unwrap().shard(), "7Y");
        assert_eq!(NodeId::parse("00000000000000000000000001").unwrap().shard(), "01");
    }

    #[test]
    fn serde_writes_the_canonical_text_and_reads_leniently() {
        let id = NodeId::parse("3RGJ87CMS6TK775XRJV8BW977Y").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"3RGJ87CMS6TK775XRJV8BW977Y\"");
        assert_eq!(serde_json::from_str::<NodeId>("\"3rgj87cms6tk775xrjv8bw977y\"").unwrap(), id);
        assert!(serde_json::from_str::<NodeId>("\"nope\"").is_err());
    }

    #[test]
    fn a_provisional_id_hides_its_value() {
        use crate::location::{LocationKey, RepoPath};
        let loc = LocationKey::new(RepoPath::new("src/a.py").unwrap(), "f", 1).unwrap();
        assert_eq!(format!("{:?}", ProvisionalId::of(&loc)), "ProvisionalId(..)");
    }
}
