/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{
    ffi::OsStr,
    fs,
    net::TcpListener,
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::Context;
use thiserror::Error;

use crate::{
    config::{Config, QemuConfig},
    path::{LocalPath, RemotePath},
};

const EXECUTOR_SOURCE_DIR: &str = "./executor";
const MAKEFILE_NAME: &str = "makefile";
const EXECUTOR_H_NAME: &str = "executor.h";
const EXECUTOR_CPP_NAME: &str = "executor.cpp";
const TEST_NAME: &str = "test.c";

#[derive(Error, Debug)]
pub enum ExecError {
    #[error("execution error: {0}")]
    IoError(String),
    #[error("timed out: {0}")]
    TimedOut(String),
}

pub struct RemoteCommandInterfaceOptions {
    pub ssh_port: u16,
    pub tmp_dir: LocalPath,
}

pub enum CommandInterfaceOptions {
    Local,
    Remote(RemoteCommandInterfaceOptions),
}

/// Send commands and transfer files to guest (remote) machine where tests are executed.
pub trait CommandInterface {
    fn create_dir_all(&self, path: &RemotePath) -> anyhow::Result<()>;
    fn remove_dir_all(&self, path: &RemotePath) -> anyhow::Result<()>;
    fn copy_to_remote(
        &self,
        local_path: &LocalPath,
        remote_path: &RemotePath,
    ) -> anyhow::Result<()>;
    fn copy_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()>;
    fn copy_dir_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()>;
    fn write(&self, path: &RemotePath, contents: &[u8]) -> anyhow::Result<()>;
    fn read_to_string(&self, path: &RemotePath) -> anyhow::Result<String>;

    fn exec(&self, cmd: CommandWrapper, timeout: Option<u8>) -> Result<Output, ExecError>;

    /// Execute command with current working directory changed.
    fn exec_in_dir(
        &self,
        cmd: CommandWrapper,
        dir: &RemotePath,
        timeout: Option<u8>,
    ) -> Result<Output, ExecError>;

    /// Execute command in background with stdout and stderr disabled.
    fn exec_background(&self, cmd: CommandWrapper) -> Result<(), ExecError>;

    /// Setup directory on remote where tests are compiled and executed.
    fn setup_remote_dir(&self) -> anyhow::Result<RemotePath> {
        let remote_dir = RemotePath::new_tmp("remote");

        self.remove_dir_all(&remote_dir).unwrap_or(());
        self.create_dir_all(&remote_dir).with_context(|| {
            format!(
                "failed to create remote directory at '{}'",
                remote_dir.base.display()
            )
        })?;

        let executor_dir = LocalPath::new(Path::new(EXECUTOR_SOURCE_DIR));
        self.copy_to_remote(
            &executor_dir.join(MAKEFILE_NAME),
            &remote_dir.join(MAKEFILE_NAME),
        )?;
        self.copy_to_remote(
            &executor_dir.join(EXECUTOR_H_NAME),
            &remote_dir.join(EXECUTOR_H_NAME),
        )?;
        self.copy_to_remote(
            &executor_dir.join(EXECUTOR_CPP_NAME),
            &remote_dir.join(EXECUTOR_CPP_NAME),
        )?;
        self.copy_to_remote(
            &executor_dir.join(EXECUTOR_CPP_NAME),
            &remote_dir.join(EXECUTOR_CPP_NAME),
        )?;
        self.copy_to_remote(&executor_dir.join(TEST_NAME), &remote_dir.join(TEST_NAME))?;

        let mut make = CommandWrapper::new("make");
        make.arg("-C").arg(remote_dir.base.as_ref());
        self.exec(make, None)
            .with_context(|| "failed to make test binary")?;

        Ok(remote_dir)
    }
}

pub struct CommandWrapper {
    internal: Command,
}

impl CommandWrapper {
    pub fn new<S: AsRef<OsStr>>(cmd: S) -> Self {
        Self {
            internal: Command::new(cmd),
        }
    }
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.internal.arg(arg);
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.internal.args(args);
        self
    }

    /// Execute command on host (local) machine.
    pub fn exec_local(mut self, timeout: Option<u8>) -> Result<Output, ExecError> {
        let output = match timeout {
            Some(secs) => {
                let mut timeout = Command::new("timeout");
                timeout.arg(secs.to_string());
                timeout.arg(self.internal.get_program());
                timeout.args(self.internal.get_args());
                if let Some(dir) = self.internal.get_current_dir() {
                    timeout.current_dir(dir);
                }
                timeout.output()
            }
            None => self.internal.output(),
        };
        let output = output.map_err(|v| {
            ExecError::IoError(format!(
                "failed to run local command: {:?}\n{}",
                self.internal, v
            ))
        })?;
        match output.status.code() {
            Some(0) => Ok(output),
            Some(124) if timeout.is_some() => Err(ExecError::TimedOut(format!(
                "local command {:?} timed out",
                self.internal
            ))),
            Some(_) => Err(ExecError::IoError(format!(
                "local command {:?} execution ended with error:\n{}",
                self.internal,
                String::from_utf8(output.stderr).unwrap_or("<invalid UTF-8 string>".into())
            ))),
            None => Err(ExecError::IoError(format!(
                "local command {:?} execution terminated by signal",
                self.internal
            ))),
        }
    }
}

/// Used when running with QEMU disabled.
pub struct LocalCommandInterface {}

impl LocalCommandInterface {
    pub fn new() -> Self {
        LocalCommandInterface {}
    }
}

impl CommandInterface for LocalCommandInterface {
    fn create_dir_all(&self, path: &RemotePath) -> anyhow::Result<()> {
        fs::create_dir_all(path.base.as_ref())
            .with_context(|| format!("failed to create local dir at '{}'", path))
    }
    fn remove_dir_all(&self, path: &RemotePath) -> anyhow::Result<()> {
        fs::remove_dir_all(path.base.as_ref())
            .with_context(|| format!("failed to remove local dir at '{}'", path))
    }
    fn copy_to_remote(
        &self,
        local_path: &LocalPath,
        remote_path: &RemotePath,
    ) -> anyhow::Result<()> {
        fs::copy(local_path, remote_path.base.as_ref()).with_context(|| {
            format!(
                "failed to copy local file from '{}' to '{}'",
                local_path, remote_path,
            )
        })?;
        Ok(())
    }
    fn copy_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()> {
        fs::copy(remote_path.base.as_ref(), local_path).with_context(|| {
            format!(
                "failed to copy local file from '{}' to '{}'",
                remote_path, local_path,
            )
        })?;
        Ok(())
    }
    fn copy_dir_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()> {
        // to match remote (scp) implementation
        fs::remove_dir_all(local_path).unwrap_or(());
        fs::create_dir_all(local_path)?;
        for entry in fs::read_dir(remote_path.base.as_ref())? {
            let entry = entry?;
            fs::copy(entry.path(), local_path.join(entry.file_name()))?;
        }
        Ok(())
    }
    fn write(&self, path: &RemotePath, contents: &[u8]) -> anyhow::Result<()> {
        fs::write(path.base.as_ref(), contents)
            .with_context(|| format!("failed to write local file '{}'", path))
    }
    fn read_to_string(&self, path: &RemotePath) -> anyhow::Result<String> {
        fs::read_to_string(path.base.as_ref())
            .with_context(|| format!("failed to read local file '{}'", path))
    }

    fn exec(&self, cmd: CommandWrapper, timeout: Option<u8>) -> Result<Output, ExecError> {
        cmd.exec_local(timeout)
    }
    fn exec_in_dir(
        &self,
        cmd: CommandWrapper,
        dir: &RemotePath,
        timeout: Option<u8>,
    ) -> Result<Output, ExecError> {
        let mut cmd = cmd;
        cmd.internal.current_dir(dir.base.as_ref());
        cmd.exec_local(timeout)
    }
    fn exec_background(&self, cmd: CommandWrapper) -> Result<(), ExecError> {
        let mut cmd = cmd.internal;
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        match cmd.spawn() {
            Err(err) => Err(ExecError::IoError(format!(
                "failed to run local command in background: {:?}\n{}",
                cmd, err
            ))),
            Ok(_) => Ok(()),
        }
    }
}

/// Uses SSH to execute command on guest (remote) machine
/// and SCP to copy files between host and guest
pub struct RemoteCommandInterface {
    config: QemuConfig,
    options: RemoteCommandInterfaceOptions,
    tmp_file: LocalPath,
}

impl RemoteCommandInterface {
    pub fn new(config: &QemuConfig, options: RemoteCommandInterfaceOptions) -> Self {
        RemoteCommandInterface {
            config: config.clone(),
            tmp_file: options.tmp_dir.join("ssh-tmp"),
            options,
        }
    }
}

impl CommandInterface for RemoteCommandInterface {
    fn create_dir_all(&self, path: &RemotePath) -> anyhow::Result<()> {
        let mut mkdir = CommandWrapper::new("mkdir");
        mkdir.arg("-p");
        mkdir.arg(path.base.as_ref());
        self.exec(mkdir, None)
            .with_context(|| format!("failed to create remote dir at '{}'", path))?;
        Ok(())
    }
    fn remove_dir_all(&self, path: &RemotePath) -> anyhow::Result<()> {
        let mut rm = CommandWrapper::new("rm");
        rm.arg("-rf");
        rm.arg(path.base.as_ref());
        self.exec(rm, None)
            .with_context(|| format!("failed to remove remote dir at '{}'", path))?;
        Ok(())
    }
    fn copy_to_remote(
        &self,
        local_path: &LocalPath,
        remote_path: &RemotePath,
    ) -> anyhow::Result<()> {
        let mut scp = self.copy_common();
        scp.arg(local_path.as_ref());
        scp.arg(format!("root@localhost:{}", remote_path));
        scp.exec_local(None).with_context(|| {
            format!(
                "failed to copy file from '{}' (local) to '{}' (remote)",
                local_path, remote_path,
            )
        })?;
        Ok(())
    }
    fn copy_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()> {
        let mut scp = self.copy_common();
        scp.arg(format!("root@localhost:{}", remote_path));
        scp.arg(local_path.as_ref());
        scp.exec_local(None).with_context(|| {
            format!(
                "failed to copy file to '{}' (local) from '{}' (remote)",
                local_path, remote_path,
            )
        })?;
        Ok(())
    }
    fn copy_dir_from_remote(
        &self,
        remote_path: &RemotePath,
        local_path: &LocalPath,
    ) -> anyhow::Result<()> {
        // because if local directory exists scp will copy remote directory inside local directory, for some reason...
        fs::remove_dir_all(local_path).unwrap_or(());
        let mut scp = self.copy_common();
        scp.arg("-r");
        scp.arg(format!("root@localhost:{}", remote_path));
        scp.arg(local_path.as_ref());
        scp.exec_local(None).with_context(|| {
            format!(
                "failed to copy file from '{}' (local) to '{}' (remote)",
                local_path, remote_path,
            )
        })?;
        Ok(())
    }
    fn write(&self, path: &RemotePath, contents: &[u8]) -> anyhow::Result<()> {
        fs::write(self.tmp_file.as_ref(), contents)
            .with_context(|| format!("failed to write to temporary file at '{}'", self.tmp_file))?;
        self.copy_to_remote(&self.tmp_file, path)?;
        fs::remove_file(self.tmp_file.as_ref())
            .with_context(|| format!("failed to remove temporary file at '{}'", self.tmp_file))
    }
    fn read_to_string(&self, path: &RemotePath) -> anyhow::Result<String> {
        self.copy_from_remote(path, &self.tmp_file)?;
        let s = fs::read_to_string(&self.tmp_file)
            .with_context(|| format!("failed to read from temprary file at '{}'", self.tmp_file))?;
        fs::remove_file(self.tmp_file.as_ref())
            .with_context(|| format!("failed to remove temporary file at '{}'", self.tmp_file))?;
        Ok(s)
    }

    fn exec(&self, cmd: CommandWrapper, timeout: Option<u8>) -> Result<Output, ExecError> {
        let mut ssh = self.exec_common();
        ssh.arg("-t").arg(format!("{:?}", cmd.internal));
        ssh.exec_local(timeout).map_err(|v| match v {
            ExecError::IoError(v) => {
                ExecError::IoError(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
            ExecError::TimedOut(v) => {
                ExecError::TimedOut(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
        })
    }
    fn exec_in_dir(
        &self,
        cmd: CommandWrapper,
        dir: &RemotePath,
        timeout: Option<u8>,
    ) -> Result<Output, ExecError> {
        let mut ssh = self.exec_common();
        ssh.arg("-t")
            .arg("cd")
            .arg(dir.base.as_ref())
            .arg("&&")
            .arg(format!("{:?}", cmd.internal));
        ssh.exec_local(timeout).map_err(|v| match v {
            ExecError::IoError(v) => {
                ExecError::IoError(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
            ExecError::TimedOut(v) => {
                ExecError::TimedOut(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
        })
    }
    fn exec_background(&self, cmd: CommandWrapper) -> Result<(), ExecError> {
        let mut ssh = self.exec_common();
        ssh.arg("-t")
            .arg(format!("{:?}", cmd.internal))
            .arg(">/dev/null")
            .arg("2>&1")
            .arg("&");
        ssh.exec_local(None).map_err(|v| match v {
            ExecError::IoError(v) => {
                ExecError::IoError(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
            ExecError::TimedOut(v) => {
                ExecError::TimedOut(format!("remote command error: {:?}\n{}", cmd.internal, v))
            }
        })?;
        Ok(())
    }
}

impl RemoteCommandInterface {
    fn copy_common(&self) -> CommandWrapper {
        let mut scp = CommandWrapper::new("scp");
        scp.arg("-q");
        scp.arg("-i").arg(self.config.ssh_private_key_path.clone());
        scp.arg("-o").arg("StrictHostKeyChecking no");
        // reuse connection
        scp.arg("-o").arg("ControlMaster auto");
        scp.arg("-o").arg("ControlPath /tmp/diffuzzer-ssh-%r@%h:%p");
        scp.arg("-o").arg("ControlPersist 1m");
        // not a typo
        scp.arg("-P").arg(self.options.ssh_port.to_string());
        scp
    }
    fn exec_common(&self) -> CommandWrapper {
        let mut ssh = CommandWrapper::new("ssh");
        ssh.arg("-q");
        ssh.arg("-i").arg(self.config.ssh_private_key_path.clone());
        ssh.arg("-o").arg("StrictHostKeyChecking no");
        // reuse connection
        ssh.arg("-o").arg("ControlMaster auto");
        ssh.arg("-o").arg("ControlPath /tmp/diffuzzer-ssh-%r@%h:%p");
        ssh.arg("-o").arg("ControlPersist 1m");
        ssh.arg("-p").arg(self.options.ssh_port.to_string());
        ssh.arg("root@localhost");
        ssh
    }
}

pub fn launch_cmdi(config: &Config, options: CommandInterfaceOptions) -> Box<dyn CommandInterface> {
    if let CommandInterfaceOptions::Remote(options) = options {
        Box::new(RemoteCommandInterface::new(&config.qemu, options))
    } else {
        Box::new(LocalCommandInterface::new())
    }
}

pub fn fresh_tcp_port() -> anyhow::Result<u16> {
    // Binding with a port number of 0 will request that the OS assigns a port to this listener.
    let listener = TcpListener::bind("127.0.0.1:0").with_context(|| "failed to bind TCP port")?;
    match listener
        .local_addr()
        .with_context(|| "failed to get listener local address")?
    {
        std::net::SocketAddr::V4(socket_addr_v4) => Ok(socket_addr_v4.port()),
        std::net::SocketAddr::V6(socket_addr_v6) => Ok(socket_addr_v6.port()),
    }
}
