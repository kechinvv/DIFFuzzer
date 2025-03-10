/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{cell::RefCell, rc::Rc};

use crate::{command::CommandInterface, path::RemotePath};

pub mod dash;
pub mod lcov;

pub type ObserverList = Vec<Rc<RefCell<dyn Observer>>>;

pub trait Observer {
    fn pre_exec(&mut self, cmdi: &dyn CommandInterface, output_dir: &RemotePath) -> anyhow::Result<()>;
    fn post_exec(&mut self, cmdi: &dyn CommandInterface, output_dir: &RemotePath) -> anyhow::Result<()>;
    fn skip_exec(&mut self);
}
