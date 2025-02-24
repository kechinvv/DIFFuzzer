/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use anyhow::{Context, Ok};
use log::info;
use std::fs::read_to_string;

use crate::abstract_fs::workload::Workload;
use crate::command::LocalCommandInterface;
use crate::config::Config;

use crate::fuzzing::fuzzer::Fuzzer;
use crate::fuzzing::runner::{parse_trace, Runner};
use crate::mount::FileSystemMount;
use crate::path::LocalPath;

pub struct DuoSingleFuzzer {
    runner: Runner,
    test_path: LocalPath,
}

impl DuoSingleFuzzer {
    pub fn create(
        config: Config,
        fst_mount: &'static dyn FileSystemMount,
        snd_mount: &'static dyn FileSystemMount,
        crashes_path: LocalPath,
        test_path: LocalPath,
        keep_fs: bool,
    ) -> anyhow::Result<Self> {
        let runner = Runner::create(
            fst_mount,
            snd_mount,
            crashes_path,
            config,
            keep_fs,
            Box::new(LocalCommandInterface::new()),
        )
        .with_context(|| "failed to create runner")?;
        Ok(Self { runner, test_path })
    }
}

impl Fuzzer for DuoSingleFuzzer {
    fn fuzz_one(&mut self) -> anyhow::Result<()> {
        info!("read testcase at '{}'", self.test_path);
        let input = read_to_string(&self.test_path).with_context(|| "failed to read testcase")?;
        let input: Workload =
            serde_json::from_str(&input).with_context(|| "failed to parse json")?;

        let binary_path = self.runner().compile_test(&input)?;

        let (fst_outcome, snd_outcome) = self.runner().run_harness(&binary_path)?;

        let fst_trace = parse_trace(&fst_outcome).with_context(|| "failed to parse first trace")?;
        let snd_trace =
            parse_trace(&snd_outcome).with_context(|| "failed to parse second trace")?;

        if self.detect_errors(
            &input,
            &binary_path,
            &fst_trace,
            &snd_trace,
            &fst_outcome,
            &snd_outcome,
        )? {
            return Ok(());
        }

        self.do_objective(
            &input,
            &binary_path,
            &fst_trace,
            &snd_trace,
            &fst_outcome,
            &snd_outcome,
        )?;

        Ok(())
    }

    fn show_stats(&mut self) {}

    fn runner(&mut self) -> &mut Runner {
        &mut self.runner
    }
}
