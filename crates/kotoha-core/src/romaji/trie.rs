//! Prefix-match trie for romaji-to-kana rule lookup.
//!
//! Built once from [`crate::romaji::rules::RULES`] at construction time.
//! Supports three lookup outcomes:
//! - exact terminal match (`Lookup::Match`)
//! - proper prefix of some key (`Lookup::Partial`)
//! - no match and no prefix (`Lookup::None`)
//!
//! Consumed by [`crate::romaji::state::StateMachine`].

use std::collections::HashMap;

use crate::romaji::rules::RULES;

/// Outcome of a prefix lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lookup {
    /// The input is a complete key. Contains the mapped kana output.
    Match(&'static str),
    /// The input is a proper prefix of at least one key, but not itself a key.
    Partial,
    /// The input is neither a key nor a prefix of any key.
    None,
}

/// A compact trie over ASCII byte keys.
#[derive(Debug)]
pub(crate) struct Trie {
    root: Node,
}

#[derive(Debug, Default)]
struct Node {
    /// Present iff this node terminates a key.
    value: Option<&'static str>,
    children: HashMap<u8, Node>,
}

impl Trie {
    /// Build a trie from the static [`RULES`] table.
    pub(crate) fn from_rules() -> Self {
        let mut root = Node::default();
        for (key, value) in RULES {
            insert(&mut root, key.as_bytes(), value);
        }
        Self { root }
    }

    /// Look up `input` (ASCII bytes expected).
    ///
    /// Non-ASCII input always returns [`Lookup::None`].
    pub(crate) fn lookup(&self, input: &str) -> Lookup {
        let bytes = input.as_bytes();
        let mut node = &self.root;
        for &b in bytes {
            match node.children.get(&b) {
                Some(child) => node = child,
                None => return Lookup::None,
            }
        }
        match node.value {
            Some(v) => Lookup::Match(v),
            None => {
                if node.children.is_empty() {
                    Lookup::None
                } else {
                    Lookup::Partial
                }
            }
        }
    }
}

fn insert(node: &mut Node, key: &[u8], value: &'static str) {
    let mut current = node;
    for &b in key {
        current = current.children.entry(b).or_default();
    }
    current.value = Some(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_single_char_match() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("a"), Lookup::Match("あ"));
    }

    #[test]
    fn lookup_three_char_yoon() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("kya"), Lookup::Match("きゃ"));
    }

    #[test]
    fn lookup_two_char_prefix_of_yoon_is_partial() {
        let trie = Trie::from_rules();
        // "ky" itself is not a key (ky* expands to kya/kyi/kyu/kye/kyo),
        // so the lookup is Partial.
        assert_eq!(trie.lookup("ky"), Lookup::Partial);
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("qx"), Lookup::None);
    }

    #[test]
    fn lookup_empty_string_is_partial() {
        // The empty input is a prefix of every key, so it is Partial
        // as long as RULES is non-empty.
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup(""), Lookup::Partial);
    }

    #[test]
    fn lookup_non_ascii_is_none() {
        let trie = Trie::from_rules();
        assert_eq!(trie.lookup("あ"), Lookup::None);
    }
}
