//! Functions for flattening nested JSON into simple unnested key-value objects

use itertools::Itertools;
use json::JsonValue;

/// Add everything in the `src` object into `dest`
fn merge_into<'a>(
    dest: &'a mut json::object::Object,
    src: &json::object::Object,
) -> &'a json::object::Object {
    for (key, value) in src.iter() {
        dest[key] = value.clone();
    }

    dest
}

/// Flatten a JSON document into an iterator of unnested values
pub fn flatten_json(json: &JsonValue) -> impl Iterator<Item = JsonValue> + '_ {
    flatten_json_with_path(json, "".to_string())
        .into_iter()
        .map(JsonValue::Object)
}

/// Flatten a JSON value with a particular prefix
fn flatten_json_with_path(
    json: &JsonValue,
    path: String,
) -> Box<dyn Iterator<Item = json::object::Object> + '_> {
    match json {
        JsonValue::Object(obj) => {
            if obj.is_empty() {
                let mut new_obj = json::object::Object::new();
                new_obj[path] = "".into();
                Box::new(vec![new_obj].into_iter())
            } else {
                // Add a prefix if necessary
                let prefix = if path.is_empty() {
                    path
                } else {
                    format!("{}.", path)
                };

                // Generate JSON objects for each dictionary element
                let dict_jsons = obj.iter().map(|(k, v)| {
                    flatten_json_with_path(v, prefix.clone() + k).collect::<Vec<_>>()
                });

                // Create the product of each of these elements
                // and then combine everything in the product
                let product = dict_jsons.multi_cartesian_product();
                Box::new(product.map(|dicts| {
                    dicts.iter().fold(json::object::Object::new(), |mut a, b| {
                        merge_into(&mut a, b).clone()
                    })
                }))
            }
        }
        JsonValue::Array(arr) => {
            if arr.is_empty() {
                let mut new_obj = json::object::Object::new();
                new_obj[path] = "".into();
                Box::new(vec![new_obj].into_iter())
            } else {
                Box::new(
                    arr.iter()
                        .flat_map(move |a| flatten_json_with_path(a, format!("{}[*]", path))),
                )
            }
        }
        _ => {
            let mut new_obj = json::object::Object::new();
            new_obj[path] = json.clone();
            Box::new(vec![new_obj].into_iter())
        }
    }
}
