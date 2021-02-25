
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrValidator {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    comment: String,
    #[serde(skip_serializing_if = "u64_is_zero", default)]
    bits_clr: u64,
    #[serde(skip_serializing_if = "u64_is_zero", default)]
    bits_set: u64,
    #[serde(skip_serializing_if = "int_is_zero", default)]
    default: Integer,
    #[serde(skip_serializing_if = "int_is_max", default = "Integer::max_value")]
    max: Integer,
    #[serde(skip_serializing_if = "int_is_min", default = "Integer::min_value")]
    min: Integer,
    #[serde(skip_serializing_if = "is_false", default)]
    ex_max: bool,
    #[serde(skip_serializing_if = "is_false", default)]
    ex_min: bool,
    #[serde(rename = "in", skip_serializing_if = "Vec::is_empty", default)]
    in_list: Vec<Integer>,
    #[serde(rename = "nin", skip_serializing_if = "Vec::is_empty", default)]
    nin_list: Vec<Integer>,
    #[serde(skip_serializing_if = "is_false", default)]
    query: bool,
    #[serde(skip_serializing_if = "is_false", default)]
    bit: bool,
    #[serde(skip_serializing_if = "is_false", default)]
    ord: bool,
}
