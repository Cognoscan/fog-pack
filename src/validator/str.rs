use super::*;
use crate::element::*;
use crate::error::{Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[inline]
fn is_false(v: &bool) -> bool {
    !v
}

#[inline]
fn u32_is_zero(v: &u32) -> bool {
    *v == 0
}

#[inline]
fn u32_is_max(v: &u32) -> bool {
    *v == u32::MAX
}

#[inline]
fn normalize_is_none(v: &Normalize) -> bool {
    matches!(v, Normalize::None)
}

/// Validator for UTF-8 strings.
///
/// This validator type will only pass string values. Validation passes if:
///
/// - The value's length in bytes is less than or equal to the value in `max_len`.
/// - The value's length in bytes is greater than or equal to the value in `min_len`.
/// - The value's number of unicode characters is less than or equal to the value in `max_char`.
/// - The value's number of unicode characters is greater than or equal to the value in `min_char`.
/// - The value does not begin with any of the prefixes in the `ban_prefix` list.
/// - The value does not end with any of the suffixes in the `ban_suffix` list.
/// - The value does not contain any of the characters in the `ban_char` string.
/// - If a regular expression is present in `matches`, the possibly-normalized value must match
///     against the expression.
/// - If the `in` list is not empty, the possibly-normalized value must be among the values in the list.
/// - The possibly-normalized value must not be among the values in the `nin` list.
///
/// The `normalize` field may be set to `None`, `NFC`, or `NFKC`, corresponding to Unicode
/// normalization forms. When checked for `in`, `nin`, `ban_prefix`, `ban_suffix`, `ban_char`, and 
/// `matches`, the value is first put into the selected normalization form, and any `in`, `nin`, 
/// `ban_prefix`, and `ban_suffix` list strings are normalized as well.
///
/// # Defaults
///
/// Fields that aren't specified for the validator use their defaults instead. The defaults for
/// each field are:
///
/// - comment: ""
/// - in_list: empty
/// - nin_list: empty
/// - matches: None
/// - max_len: u32::MAX
/// - min_len: 0
/// - max_char: u32::MAX
/// - min_char: 0
/// - normalize: Normalize::None
/// - ban_prefix: empty
/// - ban_suffix: empty
/// - ban_char: ""
/// - query: false
/// - regex: false
/// - size: false
///
/// # Regular Expressions
///
/// Regular expressions can be set for StrValidator using the `matches` field, but should be used
/// sparingly, and should generally be avoided if possible. If they must be used, be aware of their
/// limitations due to their memory, computation, and general consistency issues.
///
/// Before you use regular expressions or try to work around the look-around limitations, consider 
/// whether or not your validation requirement can be fulfilled by using some combination of the 
/// `ban_prefix`, `ban_suffix`, `ban_char`, `in`, and `nin` fields.
///
/// Regular expression can rapidly use up a lot of memory when compiled. This is one of the reasons
/// why it is inadvisable to accept and use unknown schemas without first checking for regexes. For 
/// queries, a schema will have some upper limit on the number of allowed regular expressions, in 
/// order to mitigate possible memory exhaustion.
///
/// Beyond their memory cost, regular expressions have a second problem: there's not really a
/// universal standard for regular expressions; at least, not one that is rigidly followed in
/// implementations. The Rust fog-pack library uses the [`regex`](https://crates.io/crates/regex)
/// crate for regular expressions, supporting Perl-style expression syntax, unicode character
/// classes, and flags for unicode support and case insensitivity. Look around and backreferences
/// are *not* supported. It is hoped that other implementations will support the same syntax, with
/// the same limitations on look around and backreferences.
///
/// Finally, because unicode support is enabled, it is possible to have a string that fails on one
/// library version and succeeds on another due to Unicode versions changing their character class
/// definitions. This is a corner case, but any schema writer should be aware of it as a
/// possibility.
///
/// # Unicode NFC and NFKC
///
/// Unicode normalization can be tricky to get right. Strings are never required to be in a
/// particular normalization form, as it may be that the creator or user of a string specifically
/// wants no normalization, but a query or schema may desire it. To this end, normalization of the
/// string being validated, as well as the `in` and `nin` lists' strings can all be done
/// before running validation. This is settable through the `normalization` field, which can be
/// `None`, `NFC`, or `NFKC`.
///
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct StrValidator {
    /// An optional comment explaining the validator.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub comment: String,
    /// A vector of specific allowed values, stored under the `in` field. If empty, this vector is not checked against.
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty")]
    pub in_list: Vec<String>,
    /// A vector of specific unallowed values, stored under the `nin` field.
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty")]
    pub nin_list: Vec<String>,
    /// A regular expression that the value must match against.
    #[serde(skip_serializing_if = "Option::is_none", with = "serde_regex")]
    pub matches: Option<Box<Regex>>,
    /// The maximum allowed number of bytes in the string value.
    #[serde(skip_serializing_if = "u32_is_max")]
    pub max_len: u32,
    /// The minimum allowed number of bytes in the string value.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub min_len: u32,
    /// The maximum allowed number of unicode characters in the string value.
    #[serde(skip_serializing_if = "u32_is_max")]
    pub max_char: u32,
    /// The minimum allowed number of unicode characters in the string value.
    #[serde(skip_serializing_if = "u32_is_zero")]
    pub min_char: u32,
    /// The Unicode normalization setting.
    #[serde(skip_serializing_if = "normalize_is_none")]
    pub normalize: Normalize,
    /// Banned string prefixes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ban_prefix: Vec<String>,
    /// Banned string suffixes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ban_suffix: Vec<String>,
    /// Banned characters.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub ban_char: String,
    /// If true, queries against matching spots may have values in the `in` or `nin` lists.
    #[serde(skip_serializing_if = "is_false")]
    pub query: bool,
    /// If true, queries against matching spots may use the `matches` value.
    #[serde(skip_serializing_if = "is_false")]
    pub regex: bool,
    /// If true, queries against matching spots may set the `ban_prefix`, `ban_suffix`, and 
    /// `ban_char` values to non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub ban: bool,
    /// If true, queries against matching spots may set the `max_len`, `min_len`, `max_char`, and
    /// `min_char` values to non-defaults.
    #[serde(skip_serializing_if = "is_false")]
    pub size: bool,
}

impl PartialEq for StrValidator {
    fn eq(&self, rhs: &Self) -> bool {
        (self.comment == rhs.comment)
            && (self.in_list == rhs.in_list)
            && (self.nin_list == rhs.nin_list)
            && (self.max_len == rhs.max_len)
            && (self.min_len == rhs.min_len)
            && (self.max_char == rhs.max_char)
            && (self.min_char == rhs.min_char)
            && (self.normalize == rhs.normalize)
            && (self.ban_prefix == rhs.ban_prefix)
            && (self.ban_suffix == rhs.ban_suffix)
            && (self.ban_char == rhs.ban_char)
            && (self.query == rhs.query)
            && (self.regex == rhs.regex)
            && (self.size == rhs.size)
            && (self.ban == rhs.ban)
            && match (&self.matches, &rhs.matches) {
                (None, None) => true,
                (Some(_), None) => false,
                (None, Some(_)) => false,
                (Some(lhs), Some(rhs)) => lhs.as_str() == rhs.as_str(),
            }
    }
}

impl std::default::Default for StrValidator {
    fn default() -> Self {
        Self {
            comment: String::new(),
            in_list: Vec::new(),
            nin_list: Vec::new(),
            matches: None,
            max_len: u32::MAX,
            min_len: u32::MIN,
            max_char: u32::MAX,
            min_char: u32::MIN,
            normalize: Normalize::None,
            ban_prefix: Vec::new(),
            ban_suffix: Vec::new(),
            ban_char: String::new(),
            query: false,
            regex: false,
            ban: false,
            size: false,
        }
    }
}

impl StrValidator {
    /// Make a new validator with the default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a comment for the validator.
    pub fn comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = comment.into();
        self
    }

    /// Set the maximum number of allowed bytes.
    pub fn max_len(mut self, max_len: u32) -> Self {
        self.max_len = max_len;
        self
    }

    /// Set the minimum number of allowed bytes.
    pub fn min_len(mut self, min_len: u32) -> Self {
        self.min_len = min_len;
        self
    }

    /// Set the maximum number of allowed characters.
    pub fn max_char(mut self, max_char: u32) -> Self {
        self.max_char = max_char;
        self
    }

    /// Set the minimum number of allowed characters.
    pub fn min_char(mut self, min_char: u32) -> Self {
        self.min_char = min_char;
        self
    }

    /// Set the unicode normalization form to use for `in`, `nin`, and `matches` checks.
    pub fn normalize(mut self, normalize: Normalize) -> Self {
        self.normalize = normalize;
        self
    }

    /// Set the regular expression to check against.
    pub fn matches(mut self, matches: Regex) -> Self {
        self.matches = Some(Box::new(matches));
        self
    }

    /// Add a value to the `in` list.
    pub fn in_add(mut self, add: impl Into<String>) -> Self {
        self.in_list.push(add.into());
        self
    }

    /// Add a value to the `nin` list.
    pub fn nin_add(mut self, add: impl Into<String>) -> Self {
        self.nin_list.push(add.into());
        self
    }

    /// Add a value to the `ban_prefix` list.
    pub fn ban_prefix_add(mut self, add: impl Into<String>) -> Self {
        self.ban_prefix.push(add.into());
        self
    }

    /// Add a value to the `ban_suffix` list.
    pub fn ban_suffix_add(mut self, add: impl Into<String>) -> Self {
        self.ban_suffix.push(add.into());
        self
    }

    /// Set the `ban_char` string.
    pub fn ban_char(mut self, ban_char: impl Into<String>) -> Self {
        self.ban_char = ban_char.into();
        self
    }

    /// Set whether or not queries can use the `in` and `nin` lists.
    pub fn query(mut self, query: bool) -> Self {
        self.query = query;
        self
    }

    /// Set whether or not queries can use the `matches` value.
    pub fn regex(mut self, regex: bool) -> Self {
        self.regex = regex;
        self
    }

    /// Set whether or not queries can use the `ban_prefix`, `ban_suffix`, and `ban_char` values.
    pub fn ban(mut self, ban: bool) -> Self {
        self.ban = ban;
        self
    }

    /// Set whether or not queries can use the `max_len`, `min_len`, `max_char`, and `min_char`
    /// values.
    pub fn size(mut self, ord: bool) -> Self {
        self.size = ord;
        self
    }

    /// Build this into a [`Validator`] enum.
    pub fn build(self) -> Validator {
        Validator::Str(self)
    }

    pub(crate) fn validate(&self, parser: &mut Parser) -> Result<()> {
        // Get element
        let elem = parser
            .next()
            .ok_or_else(|| Error::FailValidate("expected a string".to_string()))??;
        let val = if let Element::Str(v) = elem {
            v
        } else {
            return Err(Error::FailValidate(format!(
                "expected Str, got {}",
                elem.name()
            )));
        };

        // Length Checks
        if (val.len() as u32) > self.max_len {
            return Err(Error::FailValidate(
                "String is longer than max_len".to_string(),
            ));
        }
        if (val.len() as u32) < self.min_len {
            return Err(Error::FailValidate(
                "String is shorter than min_len".to_string(),
            ));
        }
        if self.max_char < u32::MAX || self.min_char > 0 {
            let len_char = bytecount::num_chars(val.as_bytes()) as u32;
            if len_char > self.max_char {
                return Err(Error::FailValidate(
                    "String is longer than max_len".to_string(),
                ));
            }
            if len_char < self.min_char {
                return Err(Error::FailValidate(
                    "String is shorter than min_len".to_string(),
                ));
            }
        }

        // Content checks
        use unicode_normalization::{
            is_nfc_quick, is_nfkc_quick, IsNormalized, UnicodeNormalization,
        };
        match self.normalize {
            Normalize::None => {
                if !self.in_list.is_empty() && !self.in_list.iter().any(|v| *v == val) {
                    return Err(Error::FailValidate(
                        "String is not on `in` list".to_string(),
                    ));
                }
                if self.nin_list.iter().any(|v| *v == val) {
                    return Err(Error::FailValidate("String is on `nin` list".to_string()));
                }
                if let Some(pre) = self.ban_prefix.iter().find(|v| val.starts_with(*v)) {
                    return Err(Error::FailValidate(format!("String begins with banned prefix {:?}", pre)));
                }
                if let Some(suf) = self.ban_suffix.iter().find(|v| val.ends_with(*v)) {
                    return Err(Error::FailValidate(format!("String ends with banned suffix {:?}", suf)));
                }
                if !self.ban_char.is_empty() {
                    if let Some(c) = val.chars().find(|c| self.ban_char.contains(*c)) {
                        return Err(Error::FailValidate(format!("String contains banned character {:?}", c)));
                    }
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
            Normalize::NFC => {
                let temp_string: String;
                let val = match is_nfc_quick(val.chars()) {
                    IsNormalized::Yes => val,
                    _ => {
                        temp_string = val.nfc().collect::<String>();
                        temp_string.as_str()
                    }
                };

                if !self.in_list.is_empty() && !self.in_list.iter().any(|v| v.nfc().eq(val.chars()))
                {
                    return Err(Error::FailValidate(
                        "NFC String is not on `in` list".to_string(),
                    ));
                }
                if self.nin_list.iter().any(|v| v.nfc().eq(val.chars())) {
                    return Err(Error::FailValidate("NFC String is on `nin` list".to_string()));
                }
                if let Some(pre) = self.ban_prefix.iter()
                    .find(|v| v.nfc().zip(val.chars()).all(|(vc, valc)| vc == valc))
                {
                    return Err(Error::FailValidate(format!("NFC String begins with banned prefix {:?}", pre)));
                }
                if !self.ban_suffix.is_empty() {
                    let mut temp = String::new();
                    if self.ban_suffix.iter()
                        .any(|v| {
                            temp.clear();
                            temp.extend(v.nfc());
                            val.ends_with(&temp)
                        })
                    {
                        return Err(Error::FailValidate(format!("NFC String ends with banned suffix {:?}", temp)));
                    }
                }
                if !self.ban_char.is_empty() {
                    if let Some(c) = val.chars().find(|c| self.ban_char.contains(*c)) {
                        return Err(Error::FailValidate(format!("NFC String contains banned character {:?}", c)));
                    }
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
            Normalize::NFKC => {
                let temp_string: String;
                let val = match is_nfkc_quick(val.chars()) {
                    IsNormalized::Yes => val,
                    _ => {
                        temp_string = val.nfkc().collect::<String>();
                        temp_string.as_str()
                    }
                };

                if !self.in_list.is_empty()
                    && !self.in_list.iter().any(|v| v.nfkc().eq(val.chars()))
                {
                    return Err(Error::FailValidate(
                        "NFKC String is not on `in` list".to_string(),
                    ));
                }
                if self.nin_list.iter().any(|v| v.nfkc().eq(val.chars())) {
                    return Err(Error::FailValidate("NFKC String is on `nin` list".to_string()));
                }
                if let Some(pre) = self.ban_prefix.iter()
                    .find(|v| v.nfkc().zip(val.chars()).all(|(vc, valc)| vc == valc))
                {
                    return Err(Error::FailValidate(format!("NFKC String begins with banned prefix {:?}", pre)));
                }
                if !self.ban_suffix.is_empty() {
                    let mut temp = String::new();
                    if self.ban_suffix.iter()
                        .any(|v| {
                            temp.clear();
                            temp.extend(v.nfkc());
                            val.ends_with(&temp)
                        })
                    {
                        return Err(Error::FailValidate(format!("NFKC String ends with banned suffix {:?}", temp)));
                    }
                }
                if !self.ban_char.is_empty() {
                    if let Some(c) = val.chars().find(|c| self.ban_char.contains(*c)) {
                        return Err(Error::FailValidate(format!("NFKC String contains banned character {:?}", c)));
                    }
                }
                if let Some(ref regex) = self.matches {
                    if !regex.is_match(val) {
                        return Err(Error::FailValidate(
                            "NFKC String doesn't match regular expression".to_string(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn query_check_str(&self, other: &Self) -> bool {
        (self.query || (other.in_list.is_empty() && other.nin_list.is_empty()))
            && (self.regex || other.matches.is_none())
            && (self.ban
                || (other.ban_prefix.is_empty()
                    && other.ban_suffix.is_empty()
                    && other.ban_char.is_empty()))
            && (self.size
                || (u32_is_max(&other.max_len)
                    && u32_is_zero(&other.min_len)
                    && u32_is_max(&other.max_char)
                    && u32_is_zero(&other.min_char)))
    }

    pub(crate) fn query_check(&self, other: &Validator) -> bool {
        match other {
            Validator::Str(other) => self.query_check_str(other),
            Validator::Multi(list) => list.iter().all(|other| match other {
                Validator::Str(other) => self.query_check_str(other),
                _ => false,
            }),
            Validator::Any => true,
            _ => false,
        }
    }
}
