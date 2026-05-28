// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

pub fn serialize<S: Serializer>(
    value: &Option<HashMap<String, Value>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        None => Option::<HashMap<String, String>>::None.serialize(serializer),
        Some(map) => {
            let string_map: HashMap<String, String> = map
                .iter()
                .map(|(k, v)| {
                    serde_json::to_string(v)
                        .map(|s| (k.clone(), s))
                        .map_err(serde::ser::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Some(string_map).serialize(serializer)
        }
    }
}

pub fn deserialize<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<HashMap<String, Value>>, D::Error> {
    let string_map: Option<HashMap<String, String>> = Option::deserialize(deserializer)?;
    match string_map {
        None => Ok(None),
        Some(map) => {
            let value_map: HashMap<String, Value> = map
                .into_iter()
                .map(|(k, v)| {
                    serde_json::from_str(&v)
                        .map(|val| (k, val))
                        .map_err(serde::de::Error::custom)
                })
                .collect::<Result<_, _>>()?;
            Ok(Some(value_map))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_postcard() {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("string_val".to_string(), serde_json::json!("hello"));
        map.insert("number_val".to_string(), serde_json::json!(42));
        map.insert("float_val".to_string(), serde_json::json!(2.5));
        map.insert("bool_val".to_string(), serde_json::json!(true));
        map.insert("null_val".to_string(), serde_json::json!(null));
        map.insert(
            "object_val".to_string(),
            serde_json::json!({"nested": "data"}),
        );
        map.insert("array_val".to_string(), serde_json::json!([1, "two", true]));

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Wrapper {
            #[serde(with = "super")]
            data: Option<HashMap<String, Value>>,
        }

        let original = Wrapper { data: Some(map) };
        let bytes = postcard::to_allocvec(&original).expect("serialize");
        let restored: Wrapper = postcard::from_bytes(&bytes).expect("deserialize");

        assert_eq!(
            restored.data.as_ref().unwrap()["string_val"],
            serde_json::json!("hello")
        );
        assert_eq!(
            restored.data.as_ref().unwrap()["number_val"],
            serde_json::json!(42)
        );
    }
}
