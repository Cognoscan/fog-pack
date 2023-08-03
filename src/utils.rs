use crate::{
    element::Element,
    types::{Hash, ValueRef},
};

/// Find all hashes within a data stream - assuming the data is valid.
pub(crate) fn find_hashes(data: &[u8]) -> Vec<Hash> {
    crate::element::Parser::new(data)
        .filter_map(|e| {
            if let Ok(Element::Hash(h)) = e {
                Some(h)
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn count_regexes(v: &ValueRef) -> usize {
    // First, unpack the validator enum
    if let ValueRef::Map(map) = v {
        // Enums should be a map with one key-value pair
        if map.len() > 1 {
            return 0;
        }
        match map.iter().next() {
            // String validator
            Some((&"Str", val)) => val["matches"].is_str() as usize,
            // Map validator
            Some((&"Map", val)) => {
                if !val.is_map() {
                    return 0;
                }
                let key_matches = if val["keys"]["matches"].is_str() {
                    1
                } else {
                    0
                };
                let req_matches = val["req"].as_map().map_or(0, |map| {
                    map.values().fold(0, |acc, val| acc + count_regexes(val))
                });
                let opt_matches = val["opt"].as_map().map_or(0, |map| {
                    map.values().fold(0, |acc, val| acc + count_regexes(val))
                });
                let values_matches = count_regexes(&val["values"]);
                key_matches + req_matches + opt_matches + values_matches
            }
            // Array validator
            Some((&"Array", val)) => {
                if !val.is_map() {
                    return 0;
                }
                let contains_matches = val["contains"].as_array().map_or(0, |array| {
                    array.iter().fold(0, |acc, val| acc + count_regexes(val))
                });
                let items_matches = count_regexes(&val["items"]);
                let prefix_matches = val["contains"].as_array().map_or(0, |array| {
                    array.iter().fold(0, |acc, val| acc + count_regexes(val))
                });
                contains_matches + items_matches + prefix_matches
            }
            // Hash validator
            Some((&"Hash", val)) => {
                if !val.is_map() {
                    return 0;
                }
                count_regexes(&val["link"])
            }
            // Enum validator
            Some((&"Enum", val)) => val.as_map().map_or(0, |map| {
                map.values().fold(0, |acc, val| acc + count_regexes(val))
            }),
            // Multi validator
            Some((&"Multi", val)) => val.as_array().map_or(0, |array| {
                array.iter().fold(0, |acc, val| acc + count_regexes(val))
            }),
            _ => 0,
        }
    } else {
        0
    }
}
