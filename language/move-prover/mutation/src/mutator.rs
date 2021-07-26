// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

// Functions for running benchmarks and storing the results as files, as well as reading
// benchmark data back into memory.

use anyhow::anyhow;
use bytecode::options::ProverOptions;
use clap::{App, Arg};
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use itertools::Itertools;
use log::LevelFilter;
use move_model::{
    model::{FunctionEnv, GlobalEnv, ModuleEnv, VerificationScope},
    run_model_builder,
};
use move_prover::{
    check_errors, cli::Options, create_and_process_bytecode, generate_boogie, verify_boogie,
};
use std::{
    fmt::Debug,
    fs::File,
    io::{LineWriter, Write},
    path::PathBuf,
    time::{Duration, Instant},
};

// ============================================================================================
// Command line interface for running a benchmark

struct Runner {
    options: Options,
    out: LineWriter<File>,
    error_writer: StandardStream,
    per_function: bool,
}

pub fn mutate(args: &[String]) {
    let cmd_line_parser = App::new("mutation")
        .version("0.1.0")
        .about("Mutation tool for the move prover")
        .author("The Diem Core Contributors")
        .arg(
            Arg::with_name("dependencies")
                .long("dependency")
                .short("d")
                .multiple(true)
                .number_of_values(1)
                .takes_value(true)
                .value_name("PATH_TO_DEPENDENCY")
                .help(
                    "path to a Move file, or a directory which will be searched for \
                    Move files, containing dependencies which will not be verified",
                ),
        )
        .arg(
            Arg::with_name("sources")
                .multiple(true)
                .value_name("PATH_TO_SOURCE_FILE")
                .min_values(1)
                .help("the source files to verify"),
        );
    let matches = cmd_line_parser.get_matches_from(args);
    let get_vec = |s: &str| -> Vec<String> {
        match matches.values_of(s) {
            Some(vs) => vs.map(|v| v.to_string()).collect(),
            _ => vec![],
        }
    };
    let sources = get_vec("sources");
    let deps = get_vec("dependencies");

    if let Err(s) = apply_mutation(&out, &sources, &deps) {
        println!("ERROR: execution failed: {}", s);
    } else {
        println!("results stored at `{}`", out);
    }
}

fn apply_mutation(
    out: &str,
    modules: &[String],
    dep_dirs: &[String],
) -> anyhow::Result<()> {
    println!("building model");
    let env = run_model_builder(modules, dep_dirs)?;
    let mut error_writer = StandardStream::stderr(ColorChoice::Auto);
    let mut options = Options::default();

    // Do not allow any mutation to run longer than 100 seconds to avoid absolute insanity
    options.backend.hard_timeout_secs = 100;

    options.verbosity_level = LevelFilter::Error;

    options.backend.derive_options();
    options.setup_logging();
    check_errors(&env, &options, &mut error_writer, "unexpected build errors")?;

    let config_descr = if let Some(config) = config_file_opt {
        config.clone()
    } else {
        "default".to_string()
    };

    let mut runner = Runner {
        options,
        out,
        error_writer,
        per_function,
    };
    println!(
        "Starting benchmarking with config `{}`.\n\
        Notice that execution is slow because we enforce single core execution.",
        config_descr
    );
    runner.mutate(&env)
}

impl Runner {
    fn mutate(&mut self, env: &GlobalEnv) -> anyhow::Result<()> {
        for module in env.get_modules() {
            if module.is_target() {
                self.mutate_module(module);
            }
        }
        Ok(())
    }

    fn mutate_module(&mut self, module: ModuleEnv<'_>) -> anyhow::Result<()> {
        print!("mutating module {} ..", module.get_full_name_str());
        std::io::stdout().flush()?;

        // Scope verification to the given module
        self.options.prover.verify_scope =
            VerificationScope::OnlyModule(module.get_full_name_str());
        ProverOptions::set(module.env, self.options.prover.clone());

        // Run benchmark
        let (duration, status) = self.mutate_module_duration(module.env)?;

        // Write data record of benchmark result
        writeln!(
            self.out,
            "{:<40} {:>12} {:>12}",
            module.get_full_name_str(),
            duration.as_millis(),
            status
        )?;

        println!("\x08\x08{:.3}s {}.", duration.as_secs_f64(), status);
        Ok(())
    }

    fn mutate_module_duration(&mut self, env: &GlobalEnv) -> anyhow::Result<(Duration, String)> {
        // Create and process bytecode.
        let targets = create_and_process_bytecode(&self.options, env);
        check_errors(
            env,
            &self.options,
            &mut self.error_writer,
            "unexpected transformation errors",
        )?;

        // Generate boogie code.
        let code_writer = generate_boogie(&env, &self.options, &targets)?;
        check_errors(
            env,
            &self.options,
            &mut self.error_writer,
            "unexpected boogie generation errors",
        )?;

        // Verify boogie, measuring duration.
        let now = Instant::now();
        verify_boogie(&env, &self.options, &targets, code_writer)?;

        // Determine result status.
        let status = if env.error_count() > 0 {
            if env.has_diag("timeout") {
                "timeout"
            } else {
                "errors"
            }
        } else {
            "ok"
        };
        env.clear_diag();
        Ok((now.elapsed(), status.to_string()))
    }
}
