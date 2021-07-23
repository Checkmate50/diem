// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use itertools::Itertools;
use mutation::mutator;

fn main() {
    let args = std::env::args().collect_vec();
    if (args.len() > 1 && matches!(args[1].as_str(), "-h" | "--help")) {
        println!("mutation: optionally select a subset of mutations to perform");
        println!("Use `prover-lab <tool> -h` for tool specific information.");
        std::process::exit(1);
    } else {
        mutator::mutate(&args[1..]);
    }
}
