use std::collections::HashMap;
use std::io;
use std::iter::FromIterator;
use std::time::Instant;

use indicatif::ProgressBar;
use itertools::Itertools;
use roaring::bitmap::RoaringBitmap;

#[derive(Debug)]
struct Element {
    bitmap: RoaringBitmap,
    valid: bool,
}

type Level = HashMap<RoaringBitmap, Element>;
type Bitmaps = HashMap<RoaringBitmap, RoaringBitmap>;

fn collect_values(
    lineno: usize,
    all_values: &mut HashMap<String, usize>,
    constants: &mut HashMap<String, bool>,
    first_values: &mut HashMap<String, usize>,
    load_partitions: &mut HashMap<String, HashMap<usize, RoaringBitmap>>,
    path: &str,
    value: &json::JsonValue,
) {
    if value.is_object() {
        for (dict_key, dict_value) in value.entries() {
            let mut new_path: String = path.to_owned();

            if !new_path.is_empty() {
                new_path.push('.');
            }
            new_path.push_str(dict_key);

            collect_values(
                lineno,
                all_values,
                constants,
                first_values,
                load_partitions,
                &new_path,
                dict_value,
            );
        }
    } else if value.is_array() {
        // Loop through all array elements and add [] to the path
        for list_value in value.members() {
            let mut new_path: String = path.to_owned();
            new_path.push_str("[]");

            collect_values(
                lineno,
                all_values,
                constants,
                first_values,
                load_partitions,
                &new_path,
                list_value,
            );
        }
    } else if !value.is_null() && (!value.is_string() || !value.is_empty()) {
        // Find or add the new value
        let str_value = value.dump();
        let str_index: usize = match all_values.get(&str_value) {
            Some(index) => *index,
            None => {
                let new_index = all_values.len();
                all_values.insert(str_value.clone(), new_index);

                new_index
            }
        };

        if !first_values.contains_key(path) {
            // Track the first value observed for a path
            first_values.insert(path.to_owned(), str_index);
            constants.insert(path.to_owned(), true);
        } else if *first_values.get(path).unwrap() != str_index {
            // If we see a new value at a path, we no longer have a constant
            constants.insert(path.to_owned(), false);
        }

        // Add a hash map from paths to values if not created
        if !load_partitions.contains_key(path) {
            load_partitions.insert(path.to_owned(), HashMap::new());
        }

        // Add a hash map from values to document numbers of not created
        let path_map = load_partitions.get_mut(path).unwrap();
        if !path_map.contains_key(&str_index) {
            path_map.insert(str_index.to_owned(), RoaringBitmap::new());
        }

        // Store the presence of this value at this path in this document
        let lineno32: u32 = lineno as u32;
        path_map.get_mut(&str_index).unwrap().insert(lineno32);
    }
}

fn index(i: u32, j: u32) -> u32 {
    // Ensure the values are ordered smallest first
    // Since the relationship is bidirectional,
    // we want to represent both the same
    let (i2, j2) = if i > j { (j, i) } else { (i, j) };

    // Our index is simply a linearization of a lower
    // triangular adjacency matrix laid out as follows:
    // j\i|0 1 2 3 4
    // ------------
    // 0  |
    // 1  |0
    // 2  |1 2
    // 3  |3 4 5
    // 4  |6 7 8 9
    (j2 * (j2 - 1)) / 2 + i2
}

fn reverse_index(index: u32) -> (u32, u32) {
    // Ignore the addition of i in the previous equation
    // and solve: j * (j - 1) / 2 = index for j
    // taking the floor to handle the addition of i
    let j = ((((8 * index + 1) as f64).sqrt() + 1.0) / 2.0).floor() as u32;

    // Solve for i using the equation from index()
    let i = index - (j * (j - 1)) / 2;

    (i, j)
}

fn initialize_bitmaps(
    load_partitions: &HashMap<String, HashMap<usize, RoaringBitmap>>,
    paths: &HashMap<u32, String>,
    max_lineno: u32,
) -> HashMap<RoaringBitmap, RoaringBitmap> {
    let mut all_set = RoaringBitmap::new();
    all_set.insert_range(0..index(max_lineno - 1, max_lineno - 2) + 1);

    let mut bitmaps = HashMap::new();
    bitmaps.insert(RoaringBitmap::new(), all_set);

    for (path_index, path) in paths {
        let mut bitmap = RoaringBitmap::new();
        for values in load_partitions.get(path).unwrap().values() {
            for (i, j) in values
                .iter()
                .collect::<Vec<u32>>()
                .iter()
                .tuple_combinations::<(_, _)>()
            {
                bitmap.insert(index(*i, *j));
            }
        }

        bitmaps.insert(RoaringBitmap::from(*path_index), bitmap);
    }

    bitmaps
}

macro_rules! hashcomp {
    ($name:ident = $k:expr => $v:expr; for $i:ident in $itr:expr) => {
        let mut $name: Level = HashMap::new();
        for $i in $itr {
            $name.insert($k, $v);
        }
    };
}

fn main() {
    let mut all_values: HashMap<String, usize> = HashMap::new();
    let mut constants: HashMap<String, bool> = HashMap::new();
    let mut first_values: HashMap<String, usize> = HashMap::new();
    let mut load_partitions: HashMap<String, HashMap<usize, RoaringBitmap>> = HashMap::new();

    // Initialize spinner
    let mut spinner = ProgressBar::new_spinner().with_message("Reading input…");
    spinner.enable_steady_tick(100);

    // Process input and collect values
    let start = Instant::now();
    let stdin = io::stdin();
    let mut max_lineno = 0;
    for (lineno, line) in stdin.lines().enumerate() {
        let parsed = json::parse(&line.unwrap()).ok().take().unwrap();
        collect_values(
            lineno,
            &mut all_values,
            &mut constants,
            &mut first_values,
            &mut load_partitions,
            "",
            &parsed,
        );
        max_lineno = lineno;
    }
    max_lineno += 1;

    // Remove spinner
    let mut duration = start.elapsed();
    spinner.disable_steady_tick();
    spinner.finish_with_message(format!("Collected values in {:?}", duration));

    // Remove any constant values
    for (path, is_constant) in constants {
        if is_constant {
            load_partitions.remove(&path);
        }
    }

    // Reinitialize spinner
    spinner = ProgressBar::new_spinner().with_message("Building bitmaps…");
    spinner.enable_steady_tick(100);

    // Build a map from all paths to an integer index
    let paths = load_partitions
        .keys()
        .enumerate()
        .map(|(i, k)| (i as u32, k.clone()))
        .collect::<HashMap<u32, String>>();

    // Construct the set of bitmaps for each path based on the observed data
    let mut bitmaps = initialize_bitmaps(&load_partitions, &paths, max_lineno as u32);

    // Remove spinner
    duration = start.elapsed();
    spinner.disable_steady_tick();
    spinner.finish_with_message(format!("Collected values in {:?}", duration));

    // Construct a bitmap reprenting all paths
    let mut all = RoaringBitmap::new();
    all.insert_range(0..load_partitions.len() as u32);

    // Initialize the first two levels
    //
    // Note that we represent the lattice as a hash map with the keys
    // being a bitmap representing that lattice element and the values
    // as C+ according to the TANE paper. The value also tracks a valid
    // bit which we use so that we can continue storing a lattice element
    // to track the C+ value even after that element is pruned.
    let mut level0 = HashMap::from([(
        RoaringBitmap::new(),
        Element {
            bitmap: all,
            valid: true,
        },
    )]);
    hashcomp!(level1 = RoaringBitmap::from(*a) => Element {bitmap: RoaringBitmap::new(), valid: true}; for a in paths.keys());

    for i in 0..load_partitions.len() {
        eprintln!("Starting level {}...", i + 1);

        // Calculate dependencies at this level of the lattice
        compute_dependencies(&level0, &mut level1, &bitmaps, &paths, max_lineno as u32);
        prune(&mut level1, &bitmaps, &paths, max_lineno as u32);

        // Pruning may have left a level empty, so we can't continue
        if level1.is_empty() {
            break;
        }

        // Generate the next lattice level
        level0 = level1;
        level1 = generate_next_level(&level0, &mut bitmaps);

        // We may still not have valid levels to continue
        if level1.is_empty() {
            break;
        }
    }
}

fn process_block(
    level: &Level,
    new_level: &mut Level,
    bitmaps: &mut Bitmaps,
    k: Vec<RoaringBitmap>,
) {
    // Generate all combinations of elements in the prefix block
    for (y, z) in k.iter().tuple_combinations::<(_, _)>() {
        let x = y | z;

        // Check if all required subsets are contained in the lattice
        if check_included(&x, level) {
            let valid = if level.contains_key(&y) && level.contains_key(&z) {
                // Generate the new bitmap for this potential LHS
                let y_bitmap = bitmaps.get(&y).unwrap();
                let z_bitmap = bitmaps.get(&z).unwrap();
                bitmaps.insert(x.clone(), y_bitmap & z_bitmap);
                true
            } else {
                false
            };

            // Add a new lattice element
            new_level.insert(
                x,
                Element {
                    bitmap: RoaringBitmap::new(),
                    valid,
                },
            );
        }
    }
}

/// Implements the test from line 5 of GENERATE_NEXT_LEVEL in TANE
fn check_included(x: &RoaringBitmap, level: &Level) -> bool {
    let mut included = true;
    for a in x.iter() {
        // Generate X \ {A}
        let mut check_bitmap = RoaringBitmap::new();
        check_bitmap.insert(a);
        check_bitmap = x.clone() - check_bitmap;

        // Check if the level contains X \ {A}, if not, break
        if !level.contains_key(&check_bitmap) {
            included = false;
            break;
        }
    }

    included
}

fn prefix_blocks(level: &Level) -> Vec<Vec<RoaringBitmap>> {
    // Sort bitmaps representing lattice levels by converting
    // to vectors since RoaringBitmap does not implement Ord
    let mut sorted_levels = level.keys().map(|k| k.clone()).sorted_by(|a, b| {
        Ord::cmp(
            &a.iter().collect::<Vec<u32>>(),
            &b.iter().collect::<Vec<u32>>(),
        )
    });

    // Start with the first block as the first sorted element
    let mut blocks = vec![vec![sorted_levels.next().unwrap()]];

    for x in sorted_levels {
        let last_block = blocks.last().unwrap().last().unwrap();
        // If only one element differs and it's
        // the last then we have a common prefix
        if last_block.difference_len(&x) == 1 && last_block.max() != x.max() {
            // Add this element to the current block
            blocks.last_mut().unwrap().push(x);
        } else {
            // Start a new block since the prefix doesn't match
            blocks.push(vec![x]);
        }
    }

    blocks
}

fn generate_next_level(level: &Level, bitmaps: &mut Bitmaps) -> Level {
    let mut new_level = HashMap::new();
    let blocks = prefix_blocks(level);

    for k in blocks {
        process_block(level, &mut new_level, bitmaps, k);
    }

    new_level
}

fn check_bitmap(bitmap: &RoaringBitmap, max_lineno: u32) -> bool {
    let mut violations = RoaringBitmap::new();
    for index in bitmap {
        // Get the index of the original paths
        let (i, j) = reverse_index(index);

        // Count violations of the dependency
        if !violations.contains(i) && !violations.contains(j) {
            violations.insert(i);
            violations.insert(j);
        }
    }

    // Check if the violations are below a given threshold
    let threshold = 0.99;
    (violations.len() as f64) / (max_lineno as f64) < (1.0 - threshold)
}

fn print_dependency(lhs: &RoaringBitmap, rhs: u32, paths: &HashMap<u32, String>) {
    // Look up the path values by index and print the dependency
    println!(
        "{:?} -> {}",
        lhs.iter()
            .map(|b| paths.get(&b).unwrap())
            .sorted()
            .collect::<Vec<&String>>(),
        paths.get(&rhs).unwrap()
    );
}

/// Implements the PRUNE procuedure from TANE
fn prune(level: &mut Level, bitmaps: &Bitmaps, paths: &HashMap<u32, String>, max_lineno: u32) {
    let mut to_remove = Vec::new();
    let mut invalidate = Vec::new();

    for (x, l) in level.iter() {
        // If C+(X) is empty, we can remove from the lattice
        if l.bitmap.is_empty() {
            to_remove.push(x.clone());
            continue;
        }

        if l.valid && check_bitmap(bitmaps.get(&x).unwrap(), max_lineno) {
            for a in (l.bitmap.clone() - x).iter() {
                let mut first = true;
                let mut intersect = RoaringBitmap::new();
                for b in x {
                    let cidx = (x | RoaringBitmap::from(a)) - RoaringBitmap::from(b);
                    if level.contains_key(&cidx) {
                        let c = level.get(&cidx).unwrap().bitmap.clone();
                        if first {
                            first = false;
                            intersect = c;
                        } else {
                            intersect = intersect & c;
                        }
                    } else {
                        intersect = RoaringBitmap::new();
                    }
                }

                // print!("PRUNE CHECKING ");
                // print_dependency(&x, a, paths);
                if intersect.contains(a) {
                    print_dependency(x, a, paths);
                    let mut all = RoaringBitmap::new();
                    all.insert_range(0..paths.len() as u32);

                    let new_bitmap = l.bitmap.clone() - (RoaringBitmap::from(a) | (all - x));
                    invalidate.push((x.clone(), new_bitmap));
                }
            }
        }
    }

    for (x, new_bitmap) in invalidate.iter() {
        let mut element = level.get_mut(&x).unwrap();
        element.bitmap = new_bitmap.clone();
        element.valid = false;
    }

    // Remove uneeded lattice elements
    for x in to_remove {
        level.remove(&x);
    }
}

fn compute_dependencies(
    level0: &Level,
    level1: &mut Level,
    bitmaps: &Bitmaps,
    paths: &HashMap<u32, String>,
    max_lineno: u32,
) {
    initialize_cplus_for_level(level0, level1);

    // for each X in Ll
    for (x, l) in level1 {
        // Skip elements not valid (pruned)
        if !l.valid {
            continue;
        }

        // for each A in X ^ C+(X)
        for a in x & l.bitmap.clone() {
            let rhs = RoaringBitmap::from(a); // A
            let lhs = x - rhs.clone(); // X \ {A}

            // Check validity of X \ {A} -> A
            // print!("CHECKING ");
            // print_dependency(&lhs, a, paths);
            if check_bitmap(
                &(bitmaps.get(&lhs).unwrap() - bitmaps.get(&rhs).unwrap()),
                max_lineno,
            ) {
                print_dependency(&lhs, a, paths);

                // Update C+(X) by removing A and R \ X
                l.bitmap =
                    l.bitmap.clone() - rhs - (RoaringBitmap::from_iter(paths.keys().cloned()) - x);
            }
        }
    }
}

fn initialize_cplus_for_level(level0: &Level, level1: &mut Level) {
    // This represents lines 1-2 of the COMPUTE_DEPENDENCIES procedure of TANE
    for (x, l) in level1.iter_mut() {
        let mut first = true;
        for a in x {
            let level0_key = x - RoaringBitmap::from(a);
            let old_cplus = level0.get(&level0_key).unwrap();
            if first {
                // If this is the first time, don't intersect
                // because this will result in the empty set
                first = false;
                l.bitmap = old_cplus.bitmap.clone();
            } else {
                // Continue taking the intersection for future values
                l.bitmap = l.bitmap.clone() & old_cplus.bitmap.clone();
            }
        }
    }
}
