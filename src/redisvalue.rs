use crate::{
    context::call_reply::{CallResult, VerbatimStringFormat},
    CallReply, ValkeyError, ValkeyString,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub enum ValkeyValueKey {
    Integer(i64),
    String(String),
    BulkValkeyString(ValkeyString),
    BulkString(Vec<u8>),
    Bool(bool),
}

#[derive(Debug, PartialEq, Clone)]
pub enum ValkeyValue {
    SimpleStringStatic(&'static str),
    SimpleString(String),
    BulkString(String),
    BulkValkeyString(ValkeyString),
    StringBuffer(Vec<u8>),
    Integer(i64),
    Bool(bool),
    Float(f64),
    BigNumber(String),
    VerbatimString((VerbatimStringFormat, Vec<u8>)),
    Array(Vec<ValkeyValue>),
    StaticError(&'static str),
    Map(HashMap<ValkeyValueKey, ValkeyValue>),
    Set(HashSet<ValkeyValueKey>),
    OrderedMap(BTreeMap<ValkeyValueKey, ValkeyValue>),
    OrderedSet(BTreeSet<ValkeyValueKey>),
    Null,
    NoReply, // No reply at all (as opposed to a Null reply)
}

impl TryFrom<ValkeyValue> for String {
    type Error = ValkeyError;
    fn try_from(val: ValkeyValue) -> Result<Self, ValkeyError> {
        match val {
            ValkeyValue::SimpleStringStatic(s) => Ok(s.to_string()),
            ValkeyValue::SimpleString(s) => Ok(s),
            ValkeyValue::BulkString(s) => Ok(s),
            ValkeyValue::BulkValkeyString(s) => Ok(s.try_as_str()?.to_string()),
            ValkeyValue::StringBuffer(s) => Ok(std::str::from_utf8(&s)?.to_string()),
            _ => Err(ValkeyError::Str("Can not convert result to String")),
        }
    }
}

impl From<String> for ValkeyValueKey {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<i64> for ValkeyValueKey {
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}

impl From<ValkeyString> for ValkeyValueKey {
    fn from(rs: ValkeyString) -> Self {
        Self::BulkValkeyString(rs)
    }
}

impl From<Vec<u8>> for ValkeyValueKey {
    fn from(s: Vec<u8>) -> Self {
        Self::BulkString(s)
    }
}

impl From<&str> for ValkeyValueKey {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<&String> for ValkeyValueKey {
    fn from(s: &String) -> Self {
        s.clone().into()
    }
}

impl From<bool> for ValkeyValueKey {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<()> for ValkeyValue {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<i64> for ValkeyValue {
    fn from(i: i64) -> Self {
        Self::Integer(i)
    }
}

impl From<bool> for ValkeyValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<usize> for ValkeyValue {
    fn from(i: usize) -> Self {
        (i as i64).into()
    }
}

impl From<f64> for ValkeyValue {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<String> for ValkeyValue {
    fn from(s: String) -> Self {
        Self::BulkString(s)
    }
}

impl From<ValkeyString> for ValkeyValue {
    fn from(s: ValkeyString) -> Self {
        Self::BulkValkeyString(s)
    }
}

impl From<Vec<u8>> for ValkeyValue {
    fn from(s: Vec<u8>) -> Self {
        Self::StringBuffer(s)
    }
}

impl From<&ValkeyString> for ValkeyValue {
    fn from(s: &ValkeyString) -> Self {
        s.clone().into()
    }
}

impl From<&str> for ValkeyValue {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

impl From<&String> for ValkeyValue {
    fn from(s: &String) -> Self {
        s.clone().into()
    }
}

impl<T: Into<Self>> From<Option<T>> for ValkeyValue {
    fn from(s: Option<T>) -> Self {
        s.map_or(Self::Null, Into::into)
    }
}

impl<T: Into<Self>> From<Vec<T>> for ValkeyValue {
    fn from(items: Vec<T>) -> Self {
        Self::Array(items.into_iter().map(Into::into).collect())
    }
}

impl<K: Into<ValkeyValueKey>, V: Into<ValkeyValue>> From<HashMap<K, V>> for ValkeyValue {
    fn from(items: HashMap<K, V>) -> Self {
        Self::Map(
            items
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

impl<K: Into<ValkeyValueKey>, V: Into<ValkeyValue>> From<BTreeMap<K, V>> for ValkeyValue {
    fn from(items: BTreeMap<K, V>) -> Self {
        Self::OrderedMap(
            items
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

impl<K: Into<ValkeyValueKey>> From<HashSet<K>> for ValkeyValue {
    fn from(items: HashSet<K>) -> Self {
        Self::Set(items.into_iter().map(Into::into).collect())
    }
}

impl<K: Into<ValkeyValueKey>> From<BTreeSet<K>> for ValkeyValue {
    fn from(items: BTreeSet<K>) -> Self {
        Self::OrderedSet(items.into_iter().map(Into::into).collect())
    }
}

impl<'root> TryFrom<&CallReply<'root>> for ValkeyValueKey {
    type Error = ValkeyError;
    fn try_from(reply: &CallReply<'root>) -> Result<Self, Self::Error> {
        match reply {
            CallReply::I64(reply) => Ok(ValkeyValueKey::Integer(reply.to_i64())),
            CallReply::String(reply) => Ok(reply
                .to_string()
                .map_or(ValkeyValueKey::BulkString(reply.as_bytes().to_vec()), |v| {
                    ValkeyValueKey::String(v)
                })),
            CallReply::Bool(b) => Ok(ValkeyValueKey::Bool(b.to_bool())),
            _ => Err(ValkeyError::String(format!(
                "Given CallReply can not be used as a map key or a set element, {:?}",
                reply
            ))),
        }
    }
}

impl<'root> From<&CallReply<'root>> for ValkeyValue {
    fn from(reply: &CallReply<'root>) -> Self {
        match reply {
            CallReply::Unknown => ValkeyValue::StaticError("Error on method call"),
            CallReply::Array(reply) => {
                ValkeyValue::Array(reply.iter().map(|v| (&v).into()).collect())
            }
            CallReply::I64(reply) => ValkeyValue::Integer(reply.to_i64()),
            CallReply::String(reply) => ValkeyValue::SimpleString(reply.to_string().unwrap()),
            CallReply::Null(_) => ValkeyValue::Null,
            CallReply::Map(reply) => ValkeyValue::Map(
                reply
                    .iter()
                    .map(|(key, val)| {
                        (
                            (&key).try_into().unwrap_or_else(|e| {
                                panic!("Got unhashable map key from Redis, {key:?}, {e}")
                            }),
                            (&val).into(),
                        )
                    })
                    .collect(),
            ),
            CallReply::Set(reply) => ValkeyValue::Set(
                reply
                    .iter()
                    .map(|v| {
                        (&v).try_into().unwrap_or_else(|e| {
                            panic!("Got unhashable set element from Redis, {v:?}, {e}")
                        })
                    })
                    .collect(),
            ),
            CallReply::Bool(reply) => ValkeyValue::Bool(reply.to_bool()),
            CallReply::Double(reply) => ValkeyValue::Float(reply.to_double()),
            CallReply::BigNumber(reply) => ValkeyValue::BigNumber(reply.to_string().unwrap()),
            CallReply::VerbatimString(reply) => {
                ValkeyValue::VerbatimString(reply.to_parts().unwrap())
            }
        }
    }
}

impl<'root> From<&CallResult<'root>> for ValkeyValue {
    fn from(reply: &CallResult<'root>) -> Self {
        reply.as_ref().map_or_else(
            |e| {
                // [ValkeyValue] does not support error, we can change that but to avoid
                // drastic changes and try to keep backword compatability, currently
                // we will stansform the error into a String buffer.
                ValkeyValue::StringBuffer(e.as_bytes().to_vec())
            },
            |v| (v).into(),
        )
    }
}

impl<'root> TryFrom<&CallResult<'root>> for ValkeyValueKey {
    type Error = ValkeyError;
    fn try_from(reply: &CallResult<'root>) -> Result<Self, Self::Error> {
        reply.as_ref().map_or_else(
            |e| {
                Err(ValkeyError::String(
                    format!("Got an error reply which can not be translated into a map key or set element, {:?}", e),
                ))
            },
            |v| v.try_into(),
        )
    }
}

//////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::ValkeyValue;

    #[test]
    fn from_vec_string() {
        assert_eq!(
            ValkeyValue::from(vec!["foo".to_string()]),
            ValkeyValue::Array(vec![ValkeyValue::BulkString("foo".to_owned())])
        );
    }

    #[test]
    fn from_vec_str() {
        assert_eq!(
            ValkeyValue::from(vec!["foo"]),
            ValkeyValue::Array(vec![ValkeyValue::BulkString("foo".to_owned())])
        );
    }

    #[test]
    fn from_vec_string_ref() {
        assert_eq!(
            ValkeyValue::from(vec![&"foo".to_string()]),
            ValkeyValue::Array(vec![ValkeyValue::BulkString("foo".to_owned())])
        );
    }

    #[test]
    fn from_option_str() {
        assert_eq!(
            ValkeyValue::from(Some("foo")),
            ValkeyValue::BulkString("foo".to_owned())
        );
    }

    #[test]
    fn from_vec() {
        let v: Vec<u8> = vec![0, 3, 5, 21, 255];
        assert_eq!(
            ValkeyValue::from(v),
            ValkeyValue::StringBuffer(vec![0, 3, 5, 21, 255])
        );
    }

    #[test]
    fn from_option_none() {
        assert_eq!(ValkeyValue::from(None::<()>), ValkeyValue::Null,);
    }
}
