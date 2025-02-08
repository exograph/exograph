use std::collections::HashMap;

use super::Val;
use async_graphql_value::{ConstValue, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnresolvedVal {
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    List(Vec<UnresolvedVal>),
    Object(HashMap<String, UnresolvedVal>),
    Binary(bytes::Bytes),
    Enum(String),
    Null,
    Variable(String),
}

impl UnresolvedVal {
    pub fn resolve<E>(
        self,
        resolve_variable: &impl Fn(&str) -> Result<ConstValue, E>,
    ) -> Result<Val, E> {
        match self {
            UnresolvedVal::Null => Ok(Val::Null),
            UnresolvedVal::Bool(b) => Ok(Val::Bool(b)),
            UnresolvedVal::Number(n) => Ok(Val::Number(n.try_into().unwrap())),
            UnresolvedVal::String(s) => Ok(Val::String(s)),
            UnresolvedVal::List(l) => Ok(Val::List(
                l.into_iter()
                    .map(|v| v.resolve(resolve_variable))
                    .collect::<Result<_, _>>()?,
            )),
            UnresolvedVal::Object(o) => Ok(Val::Object(
                o.into_iter()
                    .map(|(k, v)| Ok((k, v.resolve(resolve_variable)?)))
                    .collect::<Result<_, _>>()?,
            )),
            UnresolvedVal::Binary(b) => Ok(Val::Binary(b)),
            UnresolvedVal::Enum(e) => Ok(Val::Enum(e)),
            UnresolvedVal::Variable(name) => Ok(resolve_variable(&name)?.try_into().unwrap()),
        }
    }
}

impl From<Value> for UnresolvedVal {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => UnresolvedVal::Null,
            Value::Boolean(b) => UnresolvedVal::Bool(b),
            Value::Number(n) => UnresolvedVal::Number(n),
            Value::String(s) => UnresolvedVal::String(s),
            Value::List(l) => UnresolvedVal::List(l.into_iter().map(Into::into).collect()),
            Value::Object(o) => UnresolvedVal::Object(
                o.into_iter()
                    .map(|(k, v)| (k.to_string(), v.into()))
                    .collect::<HashMap<_, _>>(),
            ),
            Value::Binary(b) => UnresolvedVal::Binary(b),
            Value::Enum(e) => UnresolvedVal::Enum(e.to_string()),
            Value::Variable(name) => UnresolvedVal::Variable(name.to_string()),
        }
    }
}
