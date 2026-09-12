#[cfg(not(target_os = "linux"))]
compile_error!("kernwatch monitors the Linux kernel and builds on Linux only");

pub mod app;
pub mod collect;
pub mod model;
pub mod ui;

pub mod domain;
pub mod fixture;
pub mod flame;
pub mod symbols;

pub mod recording;

pub mod enrich;

pub mod bpf;
pub mod tracing;

pub mod actions;

pub mod probes;

pub mod logs;

pub mod diagnose;

pub mod inventory;

pub mod sources;

pub mod settings;

pub mod irq;

pub mod command;
pub mod enrichment;

pub mod bpf_metadata;

pub mod demo;

pub mod cpu_profile;
