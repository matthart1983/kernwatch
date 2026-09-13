//! CPU sampling attachment ownership and cumulative, non-destructive map reads.
use crate::{
    flame::{Kind, Profile},
    symbols::Symbols,
};
use aya::{
    maps::{HashMap as BpfHashMap, MapData, PerCpuArray, PerCpuHashMap, StackTraceMap},
    programs::{
        perf_event::{
            PerfEventConfig, PerfEventLinkId, PerfEventScope, SamplePolicy, SoftwareEvent,
        },
        PerfEvent,
    },
    Ebpf,
};
use std::{collections::BTreeMap, io};
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub start: u64,
    pub exec: u64,
    pub tgid: u32,
    pub tid: u32,
    pub user: i32,
    pub kernel: i32,
}
unsafe impl aya::Pod for Key {}
fn err(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
pub struct Sampler {
    counts: PerCpuHashMap<MapData, Key, u64>,
    execs: BpfHashMap<MapData, u32, u64>,
    stats: PerCpuArray<MapData, u64>,
    seen: BTreeMap<Key, u64>,
    frames: BTreeMap<(u32, u64, u64, i32, bool), Vec<String>>,
    links: BTreeMap<u32, PerfEventLinkId>,
    hz: u64,
    target: Option<(u32, u64)>,
    finished: bool,
}
pub fn start_ticks(pid: u32) -> Option<u64> {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()?
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}
impl Sampler {
    pub fn start(bpf: &mut Ebpf, hz: u64, tgid: u32) -> io::Result<Self> {
        let target = if tgid == 0 {
            None
        } else {
            Some((
                tgid,
                start_ticks(tgid).ok_or_else(|| err("CPU target no longer exists"))?,
            ))
        };
        let mut sampler = Self {
            execs: BpfHashMap::try_from(
                bpf.take_map("profile_execs")
                    .ok_or_else(|| err("missing exec generations"))?,
            )
            .map_err(err)?,
            counts: PerCpuHashMap::try_from(
                bpf.take_map("profile_counts")
                    .ok_or_else(|| err("missing profile counts"))?,
            )
            .map_err(err)?,
            stats: PerCpuArray::try_from(
                bpf.take_map("profile_stats")
                    .ok_or_else(|| err("missing profile stats"))?,
            )
            .map_err(err)?,
            seen: BTreeMap::new(),
            frames: BTreeMap::new(),
            links: BTreeMap::new(),
            hz,
            target,
            finished: false,
        };
        let program: &mut PerfEvent = bpf
            .program_mut("kw_profile")
            .ok_or_else(|| err("missing CPU program"))?
            .try_into()
            .map_err(err)?;
        program.load().map_err(err)?;
        sampler.attach_cpus(program)?;
        Ok(sampler)
    }
    fn attach_cpus(&mut self, program: &mut PerfEvent) -> io::Result<()> {
        let cpus = aya::util::online_cpus().map_err(|(_, e)| e)?;
        let removed: Vec<_> = self
            .links
            .keys()
            .filter(|cpu| !cpus.contains(cpu))
            .copied()
            .collect();
        for cpu in removed {
            if let Some(link) = self.links.remove(&cpu) {
                program.detach(link).map_err(err)?;
            }
        }
        for cpu in cpus {
            if let std::collections::btree_map::Entry::Vacant(entry) = self.links.entry(cpu) {
                let link = program
                    .attach(
                        PerfEventConfig::Software(SoftwareEvent::CpuClock),
                        PerfEventScope::AllProcessesOneCpu { cpu },
                        SamplePolicy::Frequency(self.hz),
                        false,
                    )
                    .map_err(|e| err(format!("CPU {cpu} sampling attachment failed: {e}")))?;
                entry.insert(link);
            }
        }
        Ok(())
    }
    pub fn stopped(&self) -> bool {
        self.finished
    }
    pub fn finish(&mut self, bpf: &mut Ebpf) -> io::Result<()> {
        let program: &mut PerfEvent = bpf
            .program_mut("kw_profile")
            .ok_or_else(|| err("missing CPU program"))?
            .try_into()
            .map_err(err)?;
        for (_, link) in std::mem::take(&mut self.links) {
            program.detach(link).map_err(err)?;
        }
        self.finished = true;
        Ok(())
    }
    pub fn poll(
        &mut self,
        bpf: &mut Ebpf,
        stacks: &StackTraceMap<MapData>,
        symbols: &mut Symbols,
        profile: &mut Profile,
    ) -> io::Result<()> {
        if !self.finished {
            if self
                .target
                .is_some_and(|(pid, start)| start_ticks(pid) != Some(start))
            {
                self.finish(bpf)?;
                profile
                    .metadata
                    .warnings
                    .push("Target exited or changed identity; sampling stopped".into());
            } else {
                let program: &mut PerfEvent = bpf
                    .program_mut("kw_profile")
                    .ok_or_else(|| err("missing CPU program"))?
                    .try_into()
                    .map_err(err)?;
                let old_cpus: Vec<_> = self.links.keys().copied().collect();
                self.attach_cpus(program)?;
                if old_cpus != self.links.keys().copied().collect::<Vec<_>>() {
                    profile
                        .metadata
                        .warnings
                        .push("Online CPU set changed; coverage changed between polls".into());
                }
                profile.metadata.cpus = self.links.keys().copied().collect();
            }
        }
        profile.metadata.kind = Kind::Cpu;
        profile.quality.attempted = self.stats.get(&0, 0).map_err(err)?.iter().sum();
        profile.quality.map_failures = self.stats.get(&1, 0).map_err(err)?.iter().sum();
        for entry in self.counts.iter() {
            let (key, counts) = entry.map_err(err)?;
            let total: u64 = counts.iter().sum();
            let delta = total.saturating_sub(*self.seen.get(&key).unwrap_or(&0));
            if delta == 0 {
                continue;
            }
            self.seen.insert(key, total);
            *profile
                .tasks
                .entry(format!(
                    "{}:{}:{}:{}",
                    key.tgid, key.tid, key.start, key.exec
                ))
                .or_default() += delta;
            let mut domains = [Vec::new(), Vec::new()];
            for (index, id) in [key.user, key.kernel].into_iter().enumerate() {
                let kernel = index == 1;
                let domain = if kernel { "kernel" } else { "user" };
                if id < 0 {
                    *profile
                        .quality
                        .errors
                        .entry(format!("{domain} walk errno {}", -(id as i64)))
                        .or_default() += delta;
                    continue;
                }
                let cache_key = (key.tgid, key.start, key.exec, id, kernel);
                if let Some(frames) = self.frames.get(&cache_key) {
                    domains[index] = frames.clone();
                    continue;
                }
                match stacks.get(&(id as u32), 0) {
                    Ok(stack) => {
                        symbols.forget(key.tgid);
                        let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
                        let ticks = key.start / 1_000_000_000 * hz
                            + key.start % 1_000_000_000 * hz / 1_000_000_000;
                        let valid_user = start_ticks(key.tgid) == Some(ticks)
                            && self.execs.get(&key.tgid, 0).ok() == Some(key.exec);
                        let frames = stack
                            .frames()
                            .iter()
                            .rev()
                            .map(|f| {
                                let (identity, info) = if kernel || valid_user {
                                    symbols.identified(key.tgid, f.ip, kernel)
                                } else {
                                    let raw = format!("0x{:x}", f.ip);
                                    (
                                        format!(
                                            "unavailable:{}:{}:{}:{:x}",
                                            key.tgid, key.start, key.exec, f.ip
                                        ),
                                        crate::flame::FrameInfo {
                                            raw: raw.clone(),
                                            display: raw,
                                            image:
                                                "process exited or exec changed before resolution"
                                                    .into(),
                                            address: f.ip,
                                            kernel: false,
                                        },
                                    )
                                };
                                profile.frames.insert(identity.clone(), info);
                                identity
                            })
                            .collect::<Vec<_>>();
                        domains[index] = frames.clone();
                        self.frames.insert(cache_key, frames);
                    }
                    Err(_) => {
                        *profile
                            .quality
                            .errors
                            .entry(format!("{domain} stack map read failed"))
                            .or_default() += delta;
                    }
                }
            }
            profile.observe(&domains[0], &domains[1], delta);
        }
        Ok(())
    }
}
