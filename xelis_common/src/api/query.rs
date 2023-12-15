use indexmap::IndexMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use super::{DataValue, DataElement, DataType};


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryNumber {
    // >
    Above(usize),
    // >=
    AboveOrEqual(usize),
    // <
    Below(usize),
    // <=
    BelowOrEqual(usize),
}

impl QueryNumber {
    pub fn verify(&self, v: &DataValue) -> bool {
        match self {
            Self::Above(value) => match v {
                DataValue::U128(v) => *v > *value as u128,
                DataValue::U64(v) => *v > *value as u64,
                DataValue::U32(v) => *v > *value as u32,
                DataValue::U16(v) => *v > *value as u16,
                DataValue::U8(v) => *v > *value as u8,
                _ => false
            },
            Self::AboveOrEqual(value) => match v {
                DataValue::U128(v) => *v >= *value as u128,
                DataValue::U64(v) => *v >= *value as u64,
                DataValue::U32(v) => *v >= *value as u32,
                DataValue::U16(v) => *v >= *value as u16,
                DataValue::U8(v) => *v >= *value as u8,
                _ => false
            },
            Self::Below(value) => match v {
                DataValue::U128(v) => *v < *value as u128,
                DataValue::U64(v) => *v < *value as u64,
                DataValue::U32(v) => *v < *value as u32,
                DataValue::U16(v) => *v < *value as u16,
                DataValue::U8(v) => *v < *value as u8,
                _ => false
            },
            Self::BelowOrEqual(value) => match v {
                DataValue::U128(v) => *v <= *value as u128,
                DataValue::U64(v) => *v <= *value as u64,
                DataValue::U32(v) => *v <= *value as u32,
                DataValue::U16(v) => *v <= *value as u16,
                DataValue::U8(v) => *v <= *value as u8,
                _ => false
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryValue {
    // ==
    Equal(DataValue),
    // Following are transformed to string and compared
    StartsWith(DataValue),
    EndsWith(DataValue),
    ContainsValue(DataValue),
    // Regex pattern on DataValue only
    #[serde(with = "serde_regex")]
    Pattern(Regex),
    #[serde(untagged)]
    NumberOp(QueryNumber)
}

impl QueryValue {
    pub fn verify(&self, v: &DataValue) -> bool {
        match self {
            Self::Equal(expected) => *v == *expected,
            Self::StartsWith(value) => v.to_string().starts_with(&value.to_string()),
            Self::EndsWith(value) => v.to_string().starts_with(&value.to_string()),
            Self::ContainsValue(value) => v.to_string().contains(&value.to_string()),
            Self::Pattern(pattern) => pattern.is_match(&v.to_string()),
            Self::NumberOp(query) => query.verify(v)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    // !
    Not(Box<Query>),
    // &&
    And(Vec<Box<Query>>),
    // ||
    Or(Vec<Box<Query>>),
    // Check value type
    Type(DataType),
    #[serde(untagged)]
    Element(QueryElement),
    #[serde(untagged)]
    Value(QueryValue)
}

impl Query {
    pub fn verify_element(&self, element: &DataElement) -> bool {
        match self {
            Self::Element(query) => query.verify(element),
            Self::Value(query) => if let DataElement::Value(Some(value)) = element {
                query.verify(value)
            } else {
                false
            },
            Self::Not(op) => !op.verify_element(element),
            Self::Or(operations) => {
                for op in operations {
                    if op.verify_element(element) {
                        return true
                    }
                }
                false
            }
            Self::And(operations) => {
                for op in operations {
                    if !op.verify_element(element) {
                        return false
                    }
                }
                true
            },
            Self::Type(expected) => element.kind() == *expected,
        }
    }

    pub fn verify_value(&self, value: &DataValue) -> bool {
        match self {
            Self::Element(_) => false,
            Self::Value(query) => query.verify(value),
            Self::Not(op) => !op.verify_value(value),
            Self::Or(operations) => {
                for op in operations {
                    if op.verify_value(value) {
                        return true
                    }
                }
                false
            }
            Self::And(operations) => {
                for op in operations {
                    if !op.verify_value(value) {
                        return false
                    }
                }
                true
            },
            Self::Type(expected) => value.kind() == *expected,
        }
    }

    pub fn is_for_element(&self) -> bool {
        match self {
            Self::Element(_) => true,
            _ => false
        }
    }
}

// This is used to do query in daemon (in future for Smart Contracts) and wallet
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")] 
pub enum QueryElement {
    // Check if DataElement::Fields has key and optional check on value
    HasKey { key: DataValue, value: Option<Box<Query>> },
    // check the array
    ArrayLen(QueryNumber),
    // Only array supported
    ContainsElement(DataElement)
}

impl QueryElement {
    pub fn verify(&self, data: &DataElement) -> bool {
        match self {
            Self::HasKey { key, value } => if let DataElement::Fields(fields) = data {
                fields.get(key).map(|v|
                    if let Some(query) = value {
                        query.verify_element(v)
                    } else {
                        false
                    }
                ).unwrap_or(false)
            } else {
                false
            },
            Self::ArrayLen(query) => if let DataElement::Array(array) = data {
                query.verify(&DataValue::U64(array.len() as u64))
            } else {
                false
            },
            Self::ContainsElement(query) => match data {
                DataElement::Array(array) => array.contains(query),
                _ => false
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct QueryResult {
    pub entries: IndexMap<DataValue, DataElement>,
    pub next: Option<usize>
}