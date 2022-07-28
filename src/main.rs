use std::collections::{BTreeSet, HashMap};
use std::io;

use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;

fn collect_values(
    values: &mut HashMap<String, BTreeSet<String>>,
    path: &str,
    value: &json::JsonValue,
) {
    if value.is_object() {
        // Traverse all keys in a dictionary adding a dot to the path
        for (dict_key, dict_value) in value.entries() {
            let mut new_path: String = path.to_owned();
            new_path.push('.');
            new_path.push_str(dict_key);
            collect_values(values, &new_path, dict_value);
        }
    } else if value.is_array() {
        // Loop through all array elements and add [] to the path
        for list_value in value.members() {
            let mut new_path: String = path.to_owned();
            new_path.push_str("[]");
            collect_values(values, &new_path, list_value);
        }
    } else if !value.is_null() && (!value.is_string() || !value.is_empty()) {
        if !values.contains_key(path) {
            // Create a new set to represent values with this path
            let mut set = BTreeSet::new();
            set.insert(value.dump());
            values.insert(path.to_owned(), set);
        } else {
            // Add this value to those observed at this path
            values.get_mut(path).unwrap().insert(value.dump());
        }
    }
}

fn main() {
    let mut values: HashMap<String, BTreeSet<String>> = HashMap::new();

    // Initialize spinner
    let mut spinner = ProgressBar::new_spinner().with_message("Reading input…");
    spinner.enable_steady_tick(100);

    // Process input and collect values
    let stdin = io::stdin();
    for line in stdin.lines() {
        let parsed = json::parse(&line.unwrap()).ok().take().unwrap();
        for (key, value) in parsed.entries() {
            collect_values(&mut values, key, value);
        }
    }

    // Remove spinner
    spinner.disable_steady_tick();
    spinner.finish_with_message("Collected values");

    // Start new progress for value counts
    spinner = ProgressBar::new_spinner().with_message("Counting unique values…");
    spinner.enable_steady_tick(100);

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

    // Remove spinner
    spinner.disable_steady_tick();
    spinner.finish_with_message("Counted values");

    // Start new progress for checking combinations
    spinner = ProgressBar::new(values.len() as u64).with_prefix("Finding dependencies");
    spinner.set_style(
        ProgressStyle::default_bar()
            .template("{prefix} [{elapsed_precise}] {bar} {pos:>7}/{len:7}"),
    );

    loop {
        let mut smallest: String = "".to_owned();
        let mut to_process = Vec::new();
        let mut to_delete = Vec::new();
        let mut remaining = false;

        // Find the smallest value and which paths need to be processed
        {
            for (path, vals) in values.iter() {
                match vals.iter().next() {
                    Some(val) => {
                        remaining = true;
                        if smallest == *val {
                            // Add this path to those which must be processed
                            to_process.push(path.clone());
                        } else if smallest.is_empty() || *val < smallest {
                            // We found a new smallest value, so start over
                            smallest = val.clone();
                            to_process = vec![path.clone()];
                        }
                    }
                    None => {
                        // Store this for deletion at the end
                        // since we didn't find anything matching
                        to_delete.push(path.clone());
                        continue;
                    }
                }
            }
        }

        // We found nothing remaining with the smallest value
        if !remaining {
            break;
        }

        for combo in to_process.iter().combinations(2) {
            *(refs.get_mut(combo[0]).unwrap().get_mut(combo[1]).unwrap()) += 1;
            *(refs.get_mut(combo[1]).unwrap().get_mut(combo[0]).unwrap()) += 1;
        }

        // Remove the smallest value from each candidate we processsed
        for k in to_process.iter() {
            values.get_mut(k).unwrap().remove(&smallest);
        }

        // Delete the keys which were pending from earlier
        spinner.inc(to_delete.len() as u64);
        for k in to_delete.iter() {
            values.remove(k);
        }
    }

    // Clear final spinner
    spinner.finish_and_clear();

    // Filter and sort dependencies
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
    inds.sort_unstable_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    for ind in inds.iter() {
        println!("{:?}", ind);
    }
}
