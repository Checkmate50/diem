// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use itertools::Itertools;
use prover_mutation::mutator;

fn main() {
    let args = std::env::args().collect_vec();
    if args.len() > 1 && matches!(args[1].as_str(), "-h" | "--help") {
        println!("mutation: optionally select a subset of mutations to perform");
        std::process::exit(1);
    } else {
        mutator::mutate(&args[1..]);
    }
}
