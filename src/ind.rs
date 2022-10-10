use crate::flatten::flatten_json;

use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};

use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use rayon::prelude::*;
use roaring::bitmap::RoaringBitmap;

fn collect_values(
    values: &mut HashMap<String, RoaringBitmap>,
    all_values: &mut HashMap<String, usize>,
    path: &str,
    value: &json::JsonValue,
) {
    if value.is_object() {
        // Traverse all keys in a dictionary adding a dot to the path
        for (dict_key, dict_value) in value.entries() {
            let mut new_path: String = path.to_owned();

            if !new_path.is_empty() {
                new_path.push('.');
            }
            new_path.push_str(dict_key);
            collect_values(values, all_values, &new_path, dict_value);
        }
    } else if value.is_array() {
        // Loop through all array elements and add [] to the path
        for list_value in value.members() {
            let mut new_path: String = path.to_owned();
            new_path.push_str("[*]");
            collect_values(values, all_values, &new_path, list_value);
        }
    } else if !value.is_null() && (!value.is_string() || !value.is_empty()) {
        let str_value = value.dump();
        let str_index: usize = match all_values.get(&str_value) {
            Some(index) => *index,
            None => {
                let new_index = all_values.len();
                all_values.insert(str_value.clone(), new_index);

                new_index
            }
        };

        if !values.contains_key(path) {
            // Create a new set to represent values with this path
            let mut set = RoaringBitmap::new();
            set.insert(str_index as u32);
            values.insert(path.to_owned(), set);
        } else {
            // Add this value to those observed at this path
            values.get_mut(path).unwrap().insert(str_index as u32);
        }
    }
}

#[derive(Args, Debug)]
/// Inclusion dependency discovery
pub struct INDArgs {
    #[clap(short, long, default_value_t = 0.9)]
    /// Threshold for approximate discovery
    threshold: f64,

    #[clap(short, long, action=clap::ArgAction::SetTrue, default_value_t = false)]
    /// Enable approximate discovery
    approximate: bool,

    #[clap(short='s', long="static", action=clap::ArgAction::SetFalse, default_value_t = true)]
    /// Use static discovery
    dynamic: bool,
}

pub fn discover(args: INDArgs) {
    let mut values: HashMap<String, RoaringBitmap> = HashMap::new();
    let mut all_values: HashMap<String, usize> = HashMap::new();

    // // Initialize spinner
    let mut spinner = ProgressBar::new_spinner().with_message("Reading input…");
    spinner.enable_steady_tick(Duration::from_millis(100));

    // Process input and collect values
    let start = Instant::now();
    let stdin = io::stdin();
    for line in stdin.lines() {
        let parsed =
            json::parse(&line.expect("Error reading input")).expect("Found invalid JSON line");

        if args.dynamic {
            collect_values(&mut values, &mut all_values, "", &parsed);
        } else {
            for obj in flatten_json(&parsed) {
                collect_values(&mut values, &mut all_values, "", &obj);
            }
        }
    }

    // Remove spinner
    let duration = start.elapsed();
    spinner.disable_steady_tick();
    spinner.finish_with_message(format!("Collected values in {:?}", duration));

    // Start new progress for checking combinations
    spinner = ProgressBar::new(values.len() as u64).with_prefix("Finding dependencies");
    spinner.set_style(
        ProgressStyle::with_template("{prefix} [{elapsed_precise}] {bar} {pos:>7}/{len:7}")
            .unwrap(),
    );

    // Discover dependencies
    let inds: Vec<_> = values
        .keys()
        .tuple_combinations::<(_, _)>()
        .par_bridge()
        .flat_map(|(key1, key2)| {
            let mut inds = Vec::new();
            let values1 = values.get(key1).unwrap();
            let values2 = values.get(key2).unwrap();
            let intersection = values1.intersection_len(values2);

            if args.approximate {
                if (intersection as f64) / (values1.len() as f64) >= args.threshold {
                    inds.push((key1, key2));
                }

                if (intersection as f64) / (values2.len() as f64) >= args.threshold {
                    inds.push((key2, key1));
                }
            } else {
                if values1.is_subset(values2) {
                    inds.push((key1, key2));
                }
                if values2.is_subset(values1) {
                    inds.push((key2, key1));
                }
            }

            inds
        })
        .collect();

    // Clear final spinner
    spinner.finish_and_clear();

    for ind in inds.iter() {
        println!("{:?}", ind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use json::{array, object};

    #[test]
    fn it_collects_object_values() {
        let obj = object! {a: 3};
        let mut values: HashMap<String, RoaringBitmap> = HashMap::new();
        let mut all_values: HashMap<String, usize> = HashMap::new();

        collect_values(&mut values, &mut all_values, "", &obj);

        assert!(values.get("a").unwrap().contains(0));
        assert!(all_values.contains_key("3"));
    }

    #[test]
    fn it_collects_nested_object_values() {
        let obj = object! {a: {b: 3}};
        let mut values: HashMap<String, RoaringBitmap> = HashMap::new();
        let mut all_values: HashMap<String, usize> = HashMap::new();

        collect_values(&mut values, &mut all_values, "", &obj);

        assert!(values.get("a.b").unwrap().contains(0));
        assert!(all_values.contains_key("3"));
    }

    #[test]
    fn it_collects_array_values() {
        let obj = array![3, 4];
        let mut values: HashMap<String, RoaringBitmap> = HashMap::new();
        let mut all_values: HashMap<String, usize> = HashMap::new();

        collect_values(&mut values, &mut all_values, "", &obj);

        assert!(values.get("[*]").unwrap().contains(0));
        assert!(values.get("[*]").unwrap().contains(1));
        assert!(all_values.contains_key("3"));
        assert!(all_values.contains_key("4"));
    }
}
