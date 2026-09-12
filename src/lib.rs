pub mod app;
#[cfg(target_os = "linux")]
pub mod collect;
pub mod model;
pub mod ui;

pub mod domain;
pub mod fixture;
pub mod flame;
pub mod symbols;

pub mod recording;

#[cfg(target_os = "linux")]
pub mod enrich;

#[cfg(target_os = "linux")]
pub mod bpf;
#[cfg(target_os = "linux")]
pub mod tracing;

pub mod actions;

#[cfg(target_os = "linux")]
pub mod probes;

#[cfg(target_os = "linux")]
pub mod logs;

pub mod diagnose;

#[cfg(target_os = "linux")]
pub mod inventory;

#[cfg(target_os = "linux")]
pub mod sources;

pub mod settings;

#[cfg(target_os = "linux")]
pub mod irq;

#[cfg(target_os = "linux")]
pub mod command;
#[cfg(target_os = "linux")]
pub mod enrichment;

#[cfg(target_os = "linux")]
pub mod bpf_metadata;

pub mod demo;
