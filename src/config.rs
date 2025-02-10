use serde::{Deserialize, Serialize};

use crate::abstract_fs::{mutator::MutationWeights, operation::OperationWeights};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub greybox: GreyboxConfig,
    pub operation_weights: OperationWeights,
    pub mutation_weights: MutationWeights,
    pub max_workload_length: u16,
    pub fs_name: String,
    pub hashing_enabled: bool,
    pub heartbeat_interval: u16,
    pub timeout: u8,
    pub qemu: QemuConfig,
}

#[derive(Serialize, Deserialize)]
pub struct GreyboxConfig {
    pub max_mutations: u16,
    pub save_corpus: bool,
}

/// [QEMU documentation](https://www.qemu.org/docs/master/system/invocation.html)
#[derive(Serialize, Deserialize)]
pub struct QemuConfig {
    /// Path to VM launch script
    pub launch_script: String,
    /// Private key used to connect to VM instance using SSH
    pub ssh_private_key_path: String,
    /// Port for monitor connection
    pub monitor_port: u16,
    /// Port for SSH connection
    pub ssh_port: u16,
    /// Path to OS image
    pub os_image: String,
}
