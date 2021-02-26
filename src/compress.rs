use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::fmt;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Compress {
    None,
    General {
        #[serde(deserialize_with = "uint_5bit")]
        algorithm: u8,
        level: u8,
    },
    Dict {
        #[serde(deserialize_with = "uint_5bit")]
        algorithm: u8,
        dict: ByteBuf,
    },
}

fn uint_5bit<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    struct MyVisitor;
    impl<'de> serde::de::Visitor<'de> for MyVisitor {
        type Value = u8;
        fn expecting(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
            write!(fmt, "an integer between 0 and 31")
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
            if v >= 0 && v <= 31 {
                Ok(v as u8)
            } else {
                Err(E::invalid_value(
                    serde::de::Unexpected::Signed(v),
                    &"integer between 0 and 31",
                ))
            }
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
            if v <= 31 {
                Ok(v as u8)
            } else {
                Err(E::invalid_value(
                    serde::de::Unexpected::Unsigned(v),
                    &"integer between 0 and 31",
                ))
            }
        }
    }

    deserializer.deserialize_u8(MyVisitor)
}
