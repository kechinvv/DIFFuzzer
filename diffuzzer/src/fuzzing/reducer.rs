/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::fs::read_to_string;

use anyhow::{Context, Ok};
use dash::FileDiff;
use log::{info, warn};

use crate::{
    abstract_fs::{mutator::remove, workload::Workload},
    command::CommandInterface,
    config::Config,
    fuzzing::outcome::Outcome,
    mount::FileSystemMount,
    path::LocalPath,
    supervisor::Supervisor,
};

use super::runner::Runner;

pub struct Reducer {
    runner: Runner,
}

impl Reducer {
    pub fn create(
        config: Config,
        fst_mount: &'static dyn FileSystemMount,
        snd_mount: &'static dyn FileSystemMount,
        crashes_path: LocalPath,
        cmdi: Box<dyn CommandInterface>,
        supervisor: Box<dyn Supervisor>,
    ) -> anyhow::Result<Self> {
        let runner = Runner::create(
            fst_mount,
            snd_mount,
            crashes_path,
            config,
            false,
            cmdi,
            supervisor,
        )
        .with_context(|| "failed to create runner")?;
        Ok(Self { runner })
    }

    pub fn run(&mut self, test_path: &LocalPath, save_to_dir: &LocalPath) -> anyhow::Result<()> {
        info!("read testcase at '{}'", test_path);
        let input = read_to_string(test_path).with_context(|| "failed to read testcase")?;
        let input: Workload =
            serde_json::from_str(&input).with_context(|| "failed to parse json")?;

        let binary_path = self.runner.compile_test(&input)?;

        match self.runner.run_harness(&binary_path)? {
            (Outcome::Completed(fst_outcome), Outcome::Completed(snd_outcome)) => {
                let hash_diff_interesting = self
                    .runner
                    .dash_objective
                    .is_interesting(&fst_outcome.dash_state, &snd_outcome.dash_state)
                    .with_context(|| "failed to do hash objective")?;
                let _trace_is_interesting = self
                    .runner
                    .trace_objective
                    .is_interesting(&fst_outcome.trace, &snd_outcome.trace)
                    .with_context(|| "failed to do trace objective")?;

                if hash_diff_interesting {
                    let old_diff = self
                        .runner
                        .dash_objective
                        .get_diff(&fst_outcome.dash_state, &snd_outcome.dash_state);
                    self.reduce_by_hash(input, old_diff, save_to_dir)?;
                } else {
                    warn!("crash not detected");
                }
            }
            _ => todo!("handle all outcomes"),
        };

        Ok(())
    }

    fn reduce_by_hash(
        &mut self,
        input: Workload,
        old_diff: Vec<FileDiff>,
        output_dir: &LocalPath,
    ) -> anyhow::Result<()> {
        info!("reduce using hash difference");
        let mut index = input.ops.len() - 1;
        let mut workload = input;
        loop {
            if let Some(reduced) = remove(&workload, index) {
                let binary_path = self.runner.compile_test(&reduced)?;
                match self.runner.run_harness(&binary_path)? {
                    (Outcome::Completed(fst_outcome), Outcome::Completed(snd_outcome)) => {
                        let hash_diff_interesting = self
                            .runner
                            .dash_objective
                            .is_interesting(&fst_outcome.dash_state, &snd_outcome.dash_state)
                            .with_context(|| "failed to do hash objective")?;
                        if hash_diff_interesting {
                            let new_diff = self
                                .runner
                                .dash_objective
                                .get_diff(&fst_outcome.dash_state, &snd_outcome.dash_state);
                            if old_diff == new_diff {
                                workload = reduced;
                                info!("workload reduced (length = {})", workload.ops.len());
                                self.runner.report_diff(
                                    &workload,
                                    index.to_string(),
                                    &binary_path,
                                    output_dir.clone(),
                                    new_diff,
                                    &fst_outcome,
                                    &snd_outcome,
                                    "".to_owned(),
                                )?;
                            }
                        }
                    }
                    _ => {}
                };
            }
            if index == 0 {
                break;
            }
            index -= 1
        }
        Ok(())
    }
}
