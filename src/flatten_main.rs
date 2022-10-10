#[forbid(clippy::missing_docs_in_private_items)]
mod flatten;

use std::io;

fn main() {
    let stdin = io::stdin();
    for line in stdin.lines() {
        let parsed =
            json::parse(&line.expect("Error reading input")).expect("Found invalid JSON line");
        for obj in flatten::flatten_json(&parsed) {
            println!("{}", obj.dump());
        }
    }
}
