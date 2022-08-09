use itertools::Itertools;
use json::JsonValue;

fn merge_into<'a>(
    dest: &'a mut json::object::Object,
    src: &json::object::Object,
) -> &'a json::object::Object {
    for (key, value) in src.iter() {
        dest[key] = value.clone();
    }

    dest
}

pub fn flatten_json(json: &JsonValue) -> Vec<JsonValue> {
    let path = "";
    flatten_json_with_path(json, path)
        .iter()
        .map(|o| JsonValue::Object(o.to_owned()))
        .collect()
}

pub fn flatten_json_with_path(json: &JsonValue, path: &str) -> Vec<json::object::Object> {
    match json {
        JsonValue::Object(obj) => {
            if obj.is_empty() {
                let mut new_obj = json::object::Object::new();
                new_obj[path] = "".into();
                vec![new_obj]
            } else {
                // Add a prefix if necessary
                let prefix = if path.is_empty() {
                    path.to_owned()
                } else {
                    format!("{}.", path)
                };

                // Generate JSON objects for each dictionary element
                let dict_jsons = obj
                    .iter()
                    .map(|(k, v)| flatten_json_with_path(v, (prefix.clone() + k).as_ref()));

                // Create the product of each of these elements
                // and then combine everything in the product
                let product = dict_jsons.multi_cartesian_product();
                product
                    .map(|dicts| {
                        dicts.iter().fold(json::object::Object::new(), |mut a, b| {
                            merge_into(&mut a, b).clone()
                        })
                    })
                    .collect()
            }
        }
        JsonValue::Array(arr) => {
            if arr.is_empty() {
                let mut new_obj = json::object::Object::new();
                new_obj[path] = "".into();
                vec![new_obj]
            } else {
                arr.iter()
                    .flat_map(|a| flatten_json_with_path(a, &format!("{}[*]", path)))
                    .collect()
            }
        }
        _ => {
            let mut new_obj = json::object::Object::new();
            new_obj[path] = json.clone();
            vec![new_obj]
        }
    }
}
