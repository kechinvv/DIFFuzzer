/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::net::UnixStream,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle, sleep},
    time::Duration,
};

use anyhow::{Context, bail};
use log::{debug, error, info};
use serde::Deserialize;
use serde_json::{Deserializer, Value};
use crate::command::CommandWrapper;

use crate::config::QemuConfig;

const SNAPSHOT_TAG: &str = "fresh";

/// Controls environment (system) in which tests are executed.
pub trait Supervisor {
    fn load_snapshot(&self) -> anyhow::Result<()>;
    fn save_snapshot(&self) -> anyhow::Result<()>;
    fn reset_events(&mut self) -> anyhow::Result<()>;
    fn had_panic_event(&mut self) -> anyhow::Result<bool>;
}

/// Stub implementation that does nothing
pub struct NativeSupervisor {}

impl NativeSupervisor {
    pub fn new() -> Self {
        Self {}
    }
}

impl Supervisor for NativeSupervisor {
    fn load_snapshot(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn save_snapshot(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn reset_events(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn had_panic_event(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }
}

pub struct QemuSupervisor {
    config: QemuConfig,
    _qemu_thread: JoinHandle<()>,
    event_handler: EventHandler,
    id_receiver: Receiver<u32>,
}

impl QemuSupervisor {
    pub fn launch(config: &QemuConfig) -> anyhow::Result<Self> {
        let console_log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_path)
            .with_context(|| format!("failed to open QEMU log file at '{}'", &config.log_path))?;
        let console_stdio = Stdio::from(console_log);

        let mut launch = Command::new(&config.launch_script);
        launch
            .env("OS_IMAGE", config.os_image.clone())
            .env("MONITOR_PORT", config.monitor_port.to_string())
            .env("SSH_PORT", config.ssh_port.to_string())
            .env("QMP_SOCKET_PATH", config.qmp_socket_path.clone())
            .env("MONITOR_SOCKET_PATH", config.monitor_socket_path.clone());
        launch
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(console_stdio);

        let (tx, rx) = mpsc::channel();

        let script = config.launch_script.clone();
        let log_path = config.log_path.clone();
        let _qemu_thread = thread::spawn(move || {
            match launch
                .spawn()
                .with_context(|| format!("failed to run qemu vm from script '{}'", script))
            {
                Ok(mut child) => {
                    tx.send(child.id()).unwrap();
                    match child.wait() {
                        Ok(status) => {
                            error!(
                                "qemu finished unexpectedly ({}), check log at '{}'",
                                status, log_path
                            );
                        }
                        Err(err) => {
                            error!(
                                "qemu finished with error, check log at '{}':\n{}",
                                log_path, err
                            )
                        }
                    };
                }
                Err(err) => error!("{:?}", err),
            };
        });

        info!("wait for VM to init ({}s)", config.boot_wait_time);
        sleep(Duration::from_secs(config.boot_wait_time.into()));

        let event_handler = EventHandler::launch(&config.qmp_socket_path)
            .with_context(|| "failed to launch event handler")?;

        Ok(Self {
            config: config.clone(),
            _qemu_thread,
            event_handler,
            id_receiver: rx,
        })
    }

    /// Connect to QEMU monitor using QMP protocol
    fn monitor_stream(&self) -> anyhow::Result<UnixStream> {
        UnixStream::connect(&self.config.monitor_socket_path).with_context(|| {
            format!(
                "failed to connect to monitor at '{}'",
                &self.config.monitor_socket_path
            )
        })
    }
}

impl Supervisor for QemuSupervisor {
    fn load_snapshot(&self) -> anyhow::Result<()> {
        info!("load vm snapshot");
        let mut stream = self.monitor_stream()?;
        writeln!(stream, "loadvm {}", SNAPSHOT_TAG)?;
        Ok(())
    }

    fn save_snapshot(&self) -> anyhow::Result<()> {
        info!("save vm snapshot");
        let mut stream = self.monitor_stream()?;
        writeln!(stream, "savevm {}", SNAPSHOT_TAG)?;
        Ok(())
    }
    fn reset_events(&mut self) -> anyhow::Result<()> {
        self.event_handler.reset()
    }
    fn had_panic_event(&mut self) -> anyhow::Result<bool> {
        self.event_handler.had_panic_event()
    }
}

impl Drop for QemuSupervisor {
    fn drop(&mut self) {
        let id = self.id_receiver.recv().unwrap();
        if id != 0 {
            let mut cmd = CommandWrapper::new("kill");
            cmd.arg(id.to_string());
            cmd.exec_local(None).expect("QEMU not killed");
        }
    }
}

/// Handles events from VM, such as resets, shutdowns and panics.
struct EventHandler {
    rx: Receiver<()>,
}

#[derive(Debug, Deserialize)]
struct ReturnMessage {
    #[serde(rename = "return")]
    _ret: Value,
}

impl EventHandler {
    fn launch(socket_path: &str) -> anyhow::Result<Self> {
        debug!("create event handler");
        let mut stream = UnixStream::connect(socket_path)
            .with_context(|| format!("failed to connect to unix socket at '{}'", &socket_path))?;
        let mut de = Deserializer::from_reader(stream.try_clone()?);
        debug!("read greeting message:");
        let value =
            Value::deserialize(&mut de).with_context(|| "failed to deserialize response")?;
        debug!("{}", value);
        stream.write_all(b"{ \"execute\": \"qmp_capabilities\" }")?;
        debug!("read response (deserialized):");
        let return_msg = ReturnMessage::deserialize(&mut de)
            .with_context(|| "failed to deserialize return message")?;
        debug!("{:?}", return_msg);

        let (tx, rx): (Sender<()>, Receiver<()>) = mpsc::channel();

        thread::spawn(move || {
            loop {
                let value = Value::deserialize(&mut de)
                    .with_context(|| "failed to deserialize response")
                    .unwrap();
                debug!("received QMP message:\n{}", value);
                if let Value::Object(map) = value {
                    if map.contains_key("event") {
                        tx.send(()).unwrap();
                    }
                }
            }
        });

        Ok(Self { rx })
    }

    fn had_panic_event(&mut self) -> anyhow::Result<bool> {
        let mut panicked = false;
        loop {
            match self.rx.try_recv() {
                Ok(()) => panicked = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => bail!("event channel disconnected"),
            }
        }
        Ok(panicked)
    }

    fn reset(&mut self) -> anyhow::Result<()> {
        loop {
            match self.rx.try_recv() {
                Ok(()) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => bail!("event channel disconnected"),
            }
        }
        Ok(())
    }
}
