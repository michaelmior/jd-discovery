use std::collections::{HashMap, BTreeSet};
use std::io;
use std::io::prelude::*;

use itertools::Itertools;

fn collect_values(values: &mut HashMap<String, BTreeSet<String>>, key: &str, value: &json::JsonValue) {
    if value.is_object() {
        for (dict_key, dict_value) in value.entries() {
            let mut new_key: String = key.to_owned();
            new_key.push_str(".");
            new_key.push_str(dict_key);
            collect_values(values, &new_key, dict_value);
        }
    } else if value.is_array() {
        for list_value in value.members() {
            let mut new_key: String = key.to_owned();
            new_key.push_str("[]");
            collect_values(values, &new_key, list_value);
        }
    } else if !value.is_null() && (!value.is_string() || !value.is_empty()) {
        if !values.contains_key(key) {
            let mut set = BTreeSet::new();
            set.insert(value.dump());
            values.insert(key.to_owned(), set);
        } else {
            values.get_mut(key).unwrap().insert(value.dump());
        }
    }
}

fn main() {
    let mut values: HashMap<String, BTreeSet<String>> = HashMap::new();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let parsed = json::parse(&line.unwrap()).ok().take().unwrap();
        for (key, value) in parsed.entries() {
            collect_values(&mut values, key, value);
        }
    }

    let mut refs: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut value_counts: HashMap<String, usize> = HashMap::new();
    for key in values.keys() {
        let mut map = HashMap::new();
        for key2 in values.keys() {
            if key2 != key {
                map.insert(key2.to_owned(), 0);
            }
        }
        refs.insert(key.to_owned(), map);
        value_counts.insert(key.to_owned(), values[key].len());
    }

    loop {
        let mut smallest: String = "".to_owned();
        let mut to_process = Vec::new();
        let mut to_delete = Vec::new();
        let mut remaining = false;


        {
            for (key, vals) in values.iter() {
                match vals.iter().next() {
                    Some(val) => {
                        remaining = true;
                        if smallest == *val {
                            to_process.push(key.clone());
                        } else if smallest == "" || *val < smallest.to_owned() {
                            smallest = val.clone();
                            to_process = vec![key.clone()];
                        }
                    }
                    None => {
                        to_delete.push(key.clone());
                        continue;
                    }
                }
            }
        }

        if !remaining {
            break;
        }

        for combo in to_process.iter().combinations(2) {
            *(refs.get_mut(combo[0]).unwrap().get_mut(combo[1]).unwrap()) += 1;
            *(refs.get_mut(combo[1]).unwrap().get_mut(combo[0]).unwrap()) += 1;
        }

        for k in to_process.iter() {
            values.get_mut(k).unwrap().remove(&smallest);
        }

        for k in to_delete.iter() {
            values.remove(k);
        }
    }

    let mut inds = Vec::new();
    for (k, v) in refs.iter() {
        for (d, i) in v.iter() {
            let frac = if value_counts[k] > 0 {
                (*i as f64) * 1.0 / (value_counts[k] as f64)
            } else {
                0.0
            };

            if frac > 0.8 {
                inds.push((k, d, frac));
            }
        }
    }
    inds.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    
    for ind in inds.iter() {
        println!("{:?}", ind);
    }
}
