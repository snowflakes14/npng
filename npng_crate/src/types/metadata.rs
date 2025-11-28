use std::collections::{BTreeMap, HashMap};
use bincode::{Decode, Encode};

#[repr(C)]
#[derive(Debug, Clone, Encode, Decode)]
pub struct Metadata {
    pub created_in: String,
    pub width: u16,
    pub height: u16,
    pub extra: HashMap<String, String>,
}

impl Metadata {
    pub fn new_string(created_in: String, extra: HashMap<String, String>) -> Self {
        Metadata {
            created_in,
            width: 0,
            height: 0,
            extra,
        }
    }

    pub fn new<C, K, V>(created_in: C, extra: HashMap<K, V>) -> Self
    where
        C: Into<String>,
        K: Into<String>,
        V: Into<String>,
    {
        Metadata {
            created_in: created_in.into(),
            width: 0,
            height: 0,
            extra: extra
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    pub fn from_btree_map<C, K, V>(created_in: C, extra: BTreeMap<K, V>) -> Self
    where
        C: Into<String>,
        K: Into<String> + Ord,
        V: Into<String>,
    {
        Metadata {
            created_in: created_in.into(),
            width: 0,
            height: 0,
            extra: extra
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    pub fn new_str(created_in: &str, extra: HashMap<&str, &str>) -> Self {
        Metadata {
            created_in: created_in.to_string(),
            width: 0,
            height: 0,
            extra: extra
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}