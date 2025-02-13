/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::fs::read_to_string;

use anyhow::{Context, Ok};
use hasher::FileDiff;
use log::{info, warn};

use crate::{
    abstract_fs::{mutator::remove, workload::Workload},
    config::Config,
    fuzzing::runner::parse_trace,
    mount::mount::FileSystemMount,
    path::LocalPath,
};

use super::runner::Runner;

pub struct Reducer {
    runner: Runner,
}

impl Reducer {
    pub fn new(
        config: Config,
        fst_mount: &'static dyn FileSystemMount,
        snd_mount: &'static dyn FileSystemMount,
        crashes_path: LocalPath,
    ) -> Self {
        Self {
            runner: Runner::new(fst_mount, snd_mount, crashes_path, config),
        }
    }

    pub fn run(&mut self, test_path: &LocalPath, save_to_dir: &LocalPath) -> anyhow::Result<()> {
        info!("running reducer");
        info!("reading testcase at '{}'", test_path);
        let input = read_to_string(test_path)
            .with_context(|| format!("failed to read testcase"))
            .unwrap();
        let input: Workload = serde_json::from_str(&input)
            .with_context(|| format!("failed to parse json"))
            .unwrap();

        let binary_path = self.runner.compile_test(&input)?;

        let (fst_outcome, snd_outcome) = self.runner.run_harness(&binary_path, false)?;

        let fst_trace =
            parse_trace(&fst_outcome).with_context(|| format!("failed to parse first trace"))?;
        let snd_trace =
            parse_trace(&snd_outcome).with_context(|| format!("failed to parse second trace"))?;

        let hash_diff_interesting = self
            .runner
            .hash_objective
            .is_interesting()
            .with_context(|| format!("failed to do hash objective"))?;
        let trace_is_interesting = self
            .runner
            .trace_objective
            .is_interesting(&fst_trace, &snd_trace)
            .with_context(|| format!("failed to do trace objective"))?;

        if hash_diff_interesting {
            let old_diff = self.runner.hash_objective.get_diff();
            self.reduce_by_hash(input, old_diff, save_to_dir)?;
        } else {
            warn!("crash not detected");
        }

        Ok(())
    }

    fn reduce_by_hash(
        &mut self,
        input: Workload,
        old_diff: Vec<FileDiff>,
        output_dir: &LocalPath,
    ) -> anyhow::Result<()> {
        info!("reducing using hash difference");
        let mut index = input.ops.len() - 1;
        let mut workload = input;
        loop {
            if let Some(reduced) = remove(&workload, index) {
                let binary_path = self.runner.compile_test(&workload)?;
                let (fst_outcome, snd_outcome) = self.runner.run_harness(&binary_path, false)?;
                let hash_diff_interesting = self
                    .runner
                    .hash_objective
                    .is_interesting()
                    .with_context(|| format!("failed to do hash objective"))?;
                if hash_diff_interesting {
                    let new_diff = self.runner.hash_objective.get_diff();
                    if old_diff == new_diff {
                        workload = reduced;
                        info!("reduced workload (length = {})", workload.ops.len());
                        self.runner.report_crash(
                            &workload,
                            &binary_path,
                            output_dir.clone(),
                            new_diff,
                            &fst_outcome,
                            &snd_outcome,
                        )?;
                    }
                }
            }
            if index == 0 {
                break;
            }
            index -= 1
        }
        Ok(())
    }
}
