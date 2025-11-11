use nutype::nutype;
use serde::{Deserialize, Serialize};
use std::fmt;

#[nutype(
    validate(greater_or_equal = 1024),
    derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)
)]
pub struct RegistryPort(u16);

impl Default for RegistryPort {
    fn default() -> Self {
        Self::try_new(5000).unwrap()
    }
}

#[nutype(
    validate(not_empty),
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref)
)]
pub struct ContainerName(String);

impl Default for ContainerName {
    fn default() -> Self {
        Self::try_new("forester-registry").unwrap()
    }
}

#[nutype(
    validate(not_empty),
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref)
)]
pub struct VolumeName(String);

impl Default for VolumeName {
    fn default() -> Self {
        Self::try_new("forester-registry-data").unwrap()
    }
}

#[nutype(
    validate(not_empty),
    derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsRef, Deref)
)]
pub struct ImageRef(String);

impl Default for ImageRef {
    fn default() -> Self {
        Self::try_new("registry:2").unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[builder(on(_, into))]
pub struct RegistryUrl {
    host: String,
    port: RegistryPort,
}

impl RegistryUrl {
    pub fn port(&self) -> u16 {
        self.port.into_inner()
    }
}

impl fmt::Display for RegistryUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.host, self.port.into_inner())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryState {
    NotCreated,
    Stopped,
    Running { url: RegistryUrl },
}

#[derive(Debug, Clone)]
pub struct RegistryStatus {
    pub state: RegistryState,
    pub volume_exists: bool,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, bon::Builder)]
#[serde(default)]
#[builder(on(_, into))]
pub struct RegistryConfig {
    pub port: RegistryPort,
    pub container_name: ContainerName,
    pub volume_name: VolumeName,
    pub image: ImageRef,
}
