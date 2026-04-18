// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Serde helpers for types that contain `HashMap<NodeId, _>`.
//!
//! JSON requires object keys to be strings, but `NodeId` (wrapping `indextree::NodeId`)
//! serializes as an integer. When `serde_json` serializes a `HashMap<NodeId, V>`,
//! it converts the integer key to a string (e.g., `42` → `"42"`), but on deserialization
//! it cannot parse the string back to `NodeId` because the deserializer expects a number.
//!
//! This module provides a `#[serde(with = "node_id_map")]` adapter that serializes
//! `HashMap<NodeId, V>` as a `Vec<(NodeId, V)>` instead, which is JSON-safe.
//!
//! # Usage
//!
//! ```rust,ignore
//! use serde::{Serialize, Deserialize};
//! use std::collections::HashMap;
//! use crate::document::serde_helpers::node_id_map;
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyStruct {
//!     #[serde(with = "node_id_map")]
//!     entries: HashMap<NodeId, String>,
//! }
//! ```

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::node::NodeId;

/// Serialize `HashMap<NodeId, V>` as `Vec<(NodeId, V)>` (sorted by key for determinism).
pub fn serialize<V, S>(map: &HashMap<NodeId, V>, serializer: S) -> Result<S::Ok, S::Error>
where
    V: Serialize,
    S: Serializer,
{
    let mut pairs: Vec<_> = map.iter().map(|(k, v)| (*k, v)).collect();
    pairs.sort_by_key(|(id, _)| usize::from(id.0));
    pairs.serialize(serializer)
}

/// Deserialize `Vec<(NodeId, V)>` back into `HashMap<NodeId, V>`.
///
/// Also accepts `{}` (empty JSON object) for backward compatibility with
/// data serialized before this helper was introduced, when `hot_nodes` etc.
/// were empty and serialized as `{}`.
pub fn deserialize<'de, V, D>(deserializer: D) -> Result<HashMap<NodeId, V>, D::Error>
where
    V: DeserializeOwned,
    D: Deserializer<'de>,
{
    use serde::de;

    // Try to deserialize as either a Vec of pairs or an empty object.
    struct VecOrEmptyMap<V>(std::marker::PhantomData<V>);

    impl<'de, V> de::Visitor<'de> for VecOrEmptyMap<V>
    where
        V: DeserializeOwned,
    {
        type Value = HashMap<NodeId, V>;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a list of (NodeId, value) pairs or an empty object")
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let pairs: Vec<(NodeId, V)> =
                Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
            Ok(pairs.into_iter().collect())
        }

        fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
        where
            A: de::MapAccess<'de>,
        {
            // Consume the map (should be empty for backward compat)
            let _: de::value::MapAccessDeserializer<A> =
                de::value::MapAccessDeserializer::new(map);
            Ok(HashMap::new())
        }
    }

    deserializer.deserialize_any(VecOrEmptyMap(std::marker::PhantomData))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentTree;

    /// Wrapper struct to test `#[serde(with)]` through serde_json round-trip.
    #[derive(Serialize, Deserialize, Debug)]
    struct Wrap<V: Serialize + serde::de::DeserializeOwned> {
        #[serde(with = "super")]
        map: HashMap<NodeId, V>,
    }

    #[test]
    fn test_empty_map_roundtrip() {
        let original = Wrap {
            map: HashMap::<NodeId, String>::new(),
        };
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains("\"map\":[]"));

        let restored: Wrap<String> = serde_json::from_str(&json).unwrap();
        assert!(restored.map.is_empty());
    }

    #[test]
    fn test_single_entry_roundtrip() {
        let tree = DocumentTree::new("Root", "content");
        let root = tree.root();

        let original = Wrap {
            map: {
                let mut m = HashMap::new();
                m.insert(root, "root data".to_string());
                m
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: Wrap<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.map.get(&root), Some(&"root data".to_string()));
    }

    #[test]
    fn test_multiple_entries_roundtrip() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "C1", "c1");
        let c2 = tree.add_child(root, "C2", "c2");

        let original = Wrap {
            map: {
                let mut m = HashMap::new();
                m.insert(root, 0u32);
                m.insert(c1, 1u32);
                m.insert(c2, 2u32);
                m
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: Wrap<u32> = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.map.len(), 3);
        assert_eq!(restored.map[&root], 0);
        assert_eq!(restored.map[&c1], 1);
        assert_eq!(restored.map[&c2], 2);
    }

    #[test]
    fn test_backward_compat_empty_object() {
        // Old data serialized hot_nodes as {} before node_id_map was used.
        let json = r#"{"map": {}}"#;
        let restored: Wrap<String> = serde_json::from_str(json).unwrap();
        assert!(restored.map.is_empty());
    }

    #[test]
    fn test_backward_compat_nonempty_object_rejected() {
        // A non-empty JSON object with string keys like {"1": "data"} should
        // fail because the string key "1" cannot be deserialized as NodeId.
        let json = r#"{"map": {"1": "data"}}"#;
        let result: Result<Wrap<String>, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialized_json_shape() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let child = tree.add_child(root, "Child", "c");

        let original = Wrap {
            map: {
                let mut m = HashMap::new();
                m.insert(root, "a".to_string());
                m.insert(child, "b".to_string());
                m
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        // Verify deterministic ordering: root (id 0) before child (id 1)
        let root_pos = json.find("\"a\"").unwrap_or(usize::MAX);
        let child_pos = json.find("\"b\"").unwrap_or(usize::MAX);
        assert!(root_pos < child_pos, "root entry should come first: {}", json);
    }

    #[test]
    fn test_roundtrip_with_complex_value() {
        // Test with a non-trivial value type (not just String/u32)
        let tree = DocumentTree::new("Root", "");
        let root = tree.root();

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Entry {
            count: u32,
            label: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct ComplexWrap {
            #[serde(with = "super")]
            data: HashMap<NodeId, Entry>,
        }

        let original = ComplexWrap {
            data: {
                let mut m = HashMap::new();
                m.insert(
                    root,
                    Entry {
                        count: 42,
                        label: "test".to_string(),
                    },
                );
                m
            },
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: ComplexWrap = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.data[&root].count, 42);
        assert_eq!(restored.data[&root].label, "test");
    }
}
