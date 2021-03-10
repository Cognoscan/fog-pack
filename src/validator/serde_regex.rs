use super::*;
use regex::Regex;
use serde::{Deserializer, Serializer};

pub(super) fn serialize<S: Serializer>(
    value: &Option<Box<Regex>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        None => {
            serializer.serialize_none() // This should never actually happen, it should be skipped
        }
        Some(regex) => serializer.serialize_str(regex.as_str()),
    }
}

pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<Box<Regex>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    // Note that this will not accept a null value - it *must* be a string, even though this is
    // ends up as an Option. This is because we chose to have validators where the field is
    // either defined, or it is absent.
    let regex: String = String::deserialize(deserializer)?;
    let regex = Regex::new(&regex).map_err(|e| D::Error::custom(e.to_string()))?;
    Ok(Some(Box::new(regex)))
}
