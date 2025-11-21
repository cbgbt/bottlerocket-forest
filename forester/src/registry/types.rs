//! Type definitions for the local OCI registry.
//!
//! This module provides domain-specific types for managing a local Docker registry
//! container used to store and serve Bottlerocket kits + sdks during development.

use bon::Builder;
use nutype::nutype;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Port number for the registry HTTP server.
///
/// Enforces a minimum value of 1024 to avoid privileged ports.
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

/// Name of the Docker container running the registry.
///
/// Must be non-empty and unique within the Docker daemon.
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

/// Name of the Docker volume for persistent registry storage.
///
/// Must be non-empty and unique within the Docker daemon.
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

/// OCI image reference for the registry container.
///
/// Typically points to the official Docker registry image.
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

/// HTTP URL for accessing the registry.
///
/// Combines host and port into a complete registry endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into))]
pub struct RegistryUrl {
    host: String,
    port: RegistryPort,
}

impl RegistryUrl {
    /// Returns the port number as a primitive u16.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port.into_inner()
    }
}

impl fmt::Display for RegistryUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "http://{}:{}", self.host, self.port.into_inner())
    }
}

/// Current lifecycle state of the registry container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryState {
    NotCreated,
    Stopped,
    Running { url: RegistryUrl },
}

/// Complete status information for the registry.
#[derive(Debug, Clone)]
pub struct RegistryStatus {
    pub state: RegistryState,
    pub volume_exists: bool,
}

/// Serializable registry configuration.
///
/// Contains user-configurable settings that can be loaded from environment
/// variables or configuration files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Builder)]
#[serde(rename_all = "kebab-case", default)]
#[builder(on(_, into))]
#[non_exhaustive]
pub struct RegistryConfig {
    pub port: RegistryPort,
    pub image: ImageRef,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            port: RegistryPort::default(),
            image: ImageRef::default(),
        }
    }
}

impl RegistryConfig {
    /// Converts to runtime configuration with derived names.
    pub fn into_runtime(self) -> RegistryRuntimeConfig {
        RegistryRuntimeConfig::builder()
            .port(self.port)
            .image(self.image)
            .build()
    }
}

/// Runtime registry configuration with derived container and volume names.
///
/// Container and volume names are automatically derived from the port number
/// to allow multiple registry instances on different ports.
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[builder(on(_, into), finish_fn(vis = "", name = build_internal))]
#[non_exhaustive]
pub struct RegistryRuntimeConfig {
    #[builder(default)]
    pub port: RegistryPort,
    #[builder(default)]
    pub image: ImageRef,
    #[builder(skip)]
    pub container_name: ContainerName,
    #[builder(skip)]
    pub volume_name: VolumeName,
}

impl<S: registry_runtime_config_builder::IsComplete> RegistryRuntimeConfigBuilder<S> {
    /// Builds the runtime configuration with derived container and volume names.
    ///
    /// Container and volume names are automatically generated from the port number.
    pub fn build(self) -> RegistryRuntimeConfig {
        let config = self.build_internal();
        let container_name =
            ContainerName::try_new(format!("forester-registry-{}", config.port.into_inner()))
                .unwrap();
        let volume_name = VolumeName::try_new(format!(
            "forester-registry-data-{}",
            config.port.into_inner()
        ))
        .unwrap();
        RegistryRuntimeConfig {
            port: config.port,
            image: config.image,
            container_name,
            volume_name,
        }
    }
}

impl Default for RegistryRuntimeConfig {
    fn default() -> Self {
        RegistryRuntimeConfig::builder().build()
    }
}
