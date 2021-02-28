use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::default::Default;

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}
#[inline]
fn f64_is_zero(v: &f64) -> bool {
    *v == 0.0
}
#[inline]
fn is_nan(v: &f64) -> bool {
    v.is_nan()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BinValidator {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    #[serde(skip_serializing_if = "f64_is_zero")]
    pub default: f64,
    #[serde(skip_serializing_if = "is_nan")]
    pub max: f64,
    #[serde(skip_serializing_if = "is_nan")]
    pub min: f64,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_max: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ex_min: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<f64>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<f64>,
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub ord: bool,
}

impl Default for BinValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            default: f64::default(),
            max: f64::NAN,
            min: f64::NAN,
            ex_max: false,
            ex_min: false,
            in_list: Vec::new(),
            nin_list: Vec::new(),
            query: false,
            ord: false,
        }
    }
}

impl BinValidator {
    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        todo!()
    }

    fn query_check_self(&self, other: &Self) -> bool {
        todo!()
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Bin(other) => self.query_check_self(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Bin(other) => self.query_check_self(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}

