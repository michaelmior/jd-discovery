#![feature(map_first_last)]
mod fd;
mod flatten;
mod ind;

use std::io;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Fd(fd::FDArgs),
    Ind(ind::INDArgs),
    Flatten,
}

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Fd(fd_args) => fd::discover(fd_args),
        Commands::Ind(ind_args) => ind::discover(ind_args),
        Commands::Flatten => {
            let stdin = io::stdin();
            for line in stdin.lines() {
                let parsed = json::parse(&line.expect("Error reading input"))
                    .expect("Found invalid JSON line");
                for obj in flatten::flatten_json(&parsed) {
                    println!("{}", obj.dump());
                }
            }
        }
    }
}
