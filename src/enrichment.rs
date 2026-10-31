//! Slow metadata has separate workers, per-object caches, explicit age and bounded reads.
use crate::{command, domain::*};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::Path,
    time::Duration,
};
type Fields = Vec<(String, String)>;
#[derive(Default)]
pub struct Extra {
    cache: BTreeMap<String, (u64, Fields)>,
    cursor: usize,
    swept_at: u64,
}
pub fn bounded_file(path: impl AsRef<Path>, limit: usize) -> io::Result<String> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(io::Error::other("file exceeds read limit"));
    }
    Ok(String::from_utf8_lossy(&data).into())
}
pub fn pairs(text: &str, delimiter: char) -> Fields {
    text.split(delimiter)
        .filter_map(|s| {
            let (k, v) = s.split_once(':')?;
            Some((k.trim().into(), v.trim().into()))
        })
        .collect()
}
fn unavailable(e: impl std::fmt::Display) -> Fields {
    vec![("acquisition".into(), e.to_string())]
}
pub fn named(fields: &Fields, key: &str) -> Option<String> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
}
impl Extra {
    fn cached(&mut self, key: String, at: u64, ttl: u64, read: impl FnOnce() -> Fields) -> Fields {
        if !self
            .cache
            .get(&key)
            .is_some_and(|(then, _)| at.saturating_sub(*then) < ttl)
        {
            self.cache.insert(key.clone(), (at, read()));
        }
        let (then, fields) = self.cache.get(&key).unwrap();
        let mut fields = fields.clone();
        fields.push(("metadata sampled boot ms".into(), then.to_string()));
        if at.saturating_sub(self.swept_at) >= 60000 {
            self.cache
                .retain(|_, (then, _)| at.saturating_sub(*then) < 600000);
            self.swept_at = at;
        }
        fields
    }
    pub fn modules(&mut self, t: &mut Telemetry) {
        let total = t.modules.len();
        if total == 0 {
            return;
        }

        for i in 0..total.min(8) {
            let module = &t.modules[(self.cursor + i) % total];
            let name = module.name.clone();
            let version = named(&module.fields, "source version");
            let identity = named(&module.fields, "sysfs identity").unwrap_or_default();
            let fields=self.cached(format!("module:{name}:{identity}"),t.at_ms,60000,||match command::text("modinfo",&["-0","--",&name]){Ok(raw)=>{let mut fields=pairs(&raw,'\0');fields.retain(|(k,_)|["filename","version","srcversion","vermagic","signer","sig_key","sig_hashalgo","intree","depends","license","description","parm"].contains(&k.as_str()));
    if let Some(loaded)=version.filter(|v|!v.starts_with("unavailable")&&!v.is_empty()){if let Some(disk)=named(&fields,"srcversion"){fields.push(("loaded/file match".into(),if loaded==disk{"source versions match; signature metadata is not independent verification"}else{"MISMATCH: on-disk file differs from loaded module"}.into()));}}
    fields.push(("metadata provenance".into(),"modinfo on-disk file; loaded sysfs state is separate".into()));fields},Err(e)=>unavailable(e)});
            t.details.insert(format!("module:{name}"), fields);
        }
        self.cursor = (self.cursor + 8) % total;
        // Reuse previously visited modules while the bounded scan advances.
        for module in &t.modules {
            let identity = named(&module.fields, "sysfs identity").unwrap_or_default();
            if let Some((at, fields)) = self
                .cache
                .get(&format!("module:{}:{identity}", module.name))
            {
                t.details
                    .entry(format!("module:{}", module.name))
                    .or_insert_with(|| {
                        let mut fields = fields.clone();
                        fields.push(("metadata sampled boot ms".into(), at.to_string()));
                        fields
                    });
            }
        }
        set_quality(t, "module_metadata", "module:");
    }
    pub fn systemd(&mut self, t: &mut Telemetry) {
        let units = t
            .cgroups
            .iter()
            .filter_map(|g| {
                let unit = Path::new(&g.path).file_name()?.to_str()?;
                if !unit.ends_with(".service")
                    && !unit.ends_with(".scope")
                    && !unit.ends_with(".slice")
                {
                    return None;
                }
                Some((g.path.clone(), g.inode, unit.to_string()))
            })
            .collect::<Vec<_>>();
        if units.is_empty() {
            return;
        }
        for i in 0..units.len().min(4) {
            let (path, inode, unit) = &units[(self.cursor + i) % units.len()];
            let fields = self.cached(format!("unit:{path}:{inode}"), t.at_ms, 30000, || {
                let mut fields=unit_metadata(unit);
                if let Some(actual)=named(&fields,"manager ControlGroup").filter(|s|!s.is_empty()) {
                    if actual!=*path {fields.push(("acquisition".into(),format!("Manager unit belongs to {actual}, not requested cgroup {path}; manager data is not target configuration")));}
                }
                fields
            });
            t.details.insert(format!("cgroup:{path}"), fields);
        }
        self.cursor = (self.cursor + 4) % units.len();
        for (path, inode, _) in &units {
            if let Some((at, fields)) = self.cache.get(&format!("unit:{path}:{inode}")) {
                t.details
                    .entry(format!("cgroup:{path}"))
                    .or_insert_with(|| {
                        let mut fields = fields.clone();
                        fields.push(("metadata sampled boot ms".into(), at.to_string()));
                        fields
                    });
            }
        }
        set_quality(t, "systemd", "cgroup:");
    }
    pub fn storage(&mut self, t: &mut Telemetry) {
        let devices = t
            .devices
            .iter()
            .filter(|d| !d.partition)
            .collect::<Vec<_>>();
        let total = devices.len();
        for device in devices.iter().cycle().skip(self.cursor).take(total.min(8)) {
            let name = device.name.clone();
            if name.contains('/') || name.starts_with('-') {
                continue;
            }
            let fields = self.cached(
                format!("storage:{}:{}", device.major_minor, name),
                t.at_ms,
                60000,
                || {
                    let path = format!("/dev/{name}");
                    match command::run(
                        "smartctl",
                        &["-a", "-j", "-n", "standby", &path],
                        Duration::from_millis(900),
                        1024 * 1024,
                    ) {
                        Ok(out) => match serde_json::from_str::<serde_json::Value>(&out.stdout) {
                            Ok(v) => storage_fields(&v, out.status),
                            Err(e) => unavailable(format!("smartctl JSON: {e}; {}", out.stderr)),
                        },
                        Err(e) => unavailable(e),
                    }
                },
            );
            t.details.insert(format!("device:{name}"), fields);
        }
        self.cursor = if total == 0 {
            0
        } else {
            (self.cursor + 8) % total
        };
        for device in &t.devices {
            let key = format!("storage:{}:{}", device.major_minor, device.name);
            if let Some((at, fields)) = self.cache.get(&key) {
                t.details
                    .entry(format!("device:{}", device.name))
                    .or_insert_with(|| {
                        let mut fields = fields.clone();
                        fields.push(("metadata sampled boot ms".into(), at.to_string()));
                        fields
                    });
            }
        }
        set_quality(t, "storage_health", "device:");
    }
    pub fn tasks(&mut self, t: &mut Telemetry) {
        let total = t.tasks.len();
        for task in t.tasks.iter().cycle().skip(self.cursor).take(total.min(32)) {
            let pid = task.pid;
            let start = task.start_ticks;
            let fields = self.cached(format!("task:{pid}:{start}"), t.at_ms, 5000, || {
                if let Err(e) = crate::actions::validate_task(pid as i32, start) {
                    return unavailable(e);
                }
                let mut fields = match bounded_file(format!("/proc/{pid}/sched"), 65536) {
                    Ok(raw) => pairs(&raw, '\n')
                        .into_iter()
                        .filter(|(k, _)| {
                            k.contains("load.weight")
                                || k.contains("vruntime")
                                || k.contains("nr_switches")
                                || k.contains("nr_migrations")
                                || k.contains("wait_sum")
                                || k.contains("wait_count")
                                || k.contains("sum_exec_runtime")
                                || k == "policy"
                                || k == "prio"
                        })
                        .collect(),
                    Err(e) => unavailable(format!("/proc/{pid}/sched: {e}")),
                };
                if let Ok(raw) = bounded_file(format!("/proc/{pid}/schedstat"), 4096) {
                    let values = raw.split_whitespace().collect::<Vec<_>>();
                    for (k, v) in ["runtime ns", "runqueue wait ns", "timeslices"]
                        .into_iter()
                        .zip(values)
                    {
                        fields.push((k.into(), v.into()));
                    }
                }
                if let Err(e) = crate::actions::validate_task(pid as i32, start) {
                    return unavailable(e);
                }
                fields
            });
            t.details.insert(format!("task:{pid}"), fields);
        }
        self.cursor = if total == 0 {
            0
        } else {
            (self.cursor + 32) % total
        };
        for task in &t.tasks {
            if let Some((at, fields)) = self
                .cache
                .get(&format!("task:{}:{}", task.pid, task.start_ticks))
            {
                t.details
                    .entry(format!("task:{}", task.pid))
                    .or_insert_with(|| {
                        let mut fields = fields.clone();
                        fields.push(("metadata sampled boot ms".into(), at.to_string()));
                        fields
                    });
            }
        }
        set_quality(t, "task_metadata", "task:");
    }
}
pub fn unit_metadata(unit: &str) -> Fields {
    let properties="Id,LoadState,ActiveState,FragmentPath,DropInPaths,Transient,ControlGroup,CPUWeight,CPUQuotaPerSecUSec,CPUQuotaPeriodUSec,AllowedCPUs,EffectiveCPUs,MemoryCurrent,MemoryMax,TasksCurrent,TasksMax";
    let mut fields = match command::text(
        "systemctl",
        &["show", "--no-pager", "--property", properties, "--", unit],
    ) {
        Ok(raw) => raw
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (format!("manager {k}"), v.to_string()))
            .collect(),
        Err(e) => unavailable(format!("system manager: {e}")),
    };
    if named(&fields, "manager LoadState").as_deref() == Some("not-found") {
        fields.push((
            "acquisition".into(),
            "Unit not loaded in accessible system manager; user-manager units require that manager"
                .into(),
        ));
    }
    match command::text("systemctl", &["cat", "--no-pager", "--", unit]) {
        Ok(raw) => fields.push(("unit and drop-ins (manager order)".into(), raw)),
        Err(e) => {
            fields.push(("unit files".into(), e.to_string()));
            fields.extend(unit_files(Path::new("/"), unit));
        }
    }
    if named(&fields, "acquisition").is_none() {
        fields.push(("effective configuration".into(),"Manager properties incorporate drop-ins and transient settings; cgroup files show enforced runtime values".into()));
    }
    fields
}
pub fn unit_files(root: &Path, unit: &str) -> Fields {
    if unit.contains('/') || unit == ".." {
        return unavailable("invalid unit name");
    }
    let mut fields = Vec::new();
    let mut dropins = BTreeMap::new();
    for dir in [
        "usr/lib/systemd/system",
        "run/systemd/system",
        "etc/systemd/system",
    ] {
        let path = root.join(dir);
        if let Ok(raw) = bounded_file(path.join(unit), 65536) {
            fields.retain(|(k, _)| k != "unit fragment");
            fields.push((
                "unit fragment".into(),
                format!("# {}\n{raw}", path.join(unit).display()),
            ));
        }
        if let Ok(entries) = fs::read_dir(path.join(format!("{unit}.d"))) {
            for file in entries.flatten().take(128) {
                if file.path().extension().is_some_and(|e| e == "conf") {
                    dropins.insert(file.file_name(), file.path());
                }
            }
        }
    }
    for (_, path) in dropins {
        match bounded_file(&path, 65536) {
            Ok(raw) => fields.push((format!("drop-in {}", path.display()), raw)),
            Err(e) => fields.push((format!("drop-in {}", path.display()), e.to_string())),
        }
    }
    fields.push(("file fallback scope".into(),"Exact unit-name directories; manager unavailable, so template/type/dash-prefix overrides and transient properties require manager access".into()));
    fields
}
pub fn storage_fields(v: &serde_json::Value, status: i32) -> Fields {
    let mut fields = Vec::new();
    for (key, pointer) in [
        ("health", "/smart_status/passed"),
        ("temperature C", "/temperature/current"),
        ("power-on hours", "/power_on_time/hours"),
        ("power cycles", "/power_cycle_count"),
        ("model", "/model_name"),
        ("firmware", "/firmware_version"),
        ("capacity bytes", "/user_capacity/bytes"),
        (
            "NVMe critical warning",
            "/nvme_smart_health_information_log/critical_warning",
        ),
        (
            "NVMe spare %",
            "/nvme_smart_health_information_log/available_spare",
        ),
        (
            "NVMe life used %",
            "/nvme_smart_health_information_log/percentage_used",
        ),
        (
            "NVMe media errors",
            "/nvme_smart_health_information_log/media_errors",
        ),
        (
            "NVMe unsafe shutdowns",
            "/nvme_smart_health_information_log/unsafe_shutdowns",
        ),
    ] {
        if let Some(value) = v.pointer(pointer) {
            fields.push((
                format!("SMART {key}"),
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string()),
            ));
        }
    }
    if let Some(rows) = v
        .pointer("/ata_smart_attributes/table")
        .and_then(|v| v.as_array())
    {
        for row in rows.iter().take(32) {
            fields.push((
                format!("SMART attribute {}", row["name"].as_str().unwrap_or("?")),
                row["raw"]["value"].to_string(),
            ));
        }
    }
    if let Some(messages) = v.pointer("/smartctl/messages").and_then(|v| v.as_array()) {
        for message in messages {
            fields.push((
                "SMART message".into(),
                message["string"].as_str().unwrap_or("").into(),
            ));
        }
    }
    if status & 7 != 0
        || !fields
            .iter()
            .any(|(k, _)| k == "SMART health" || k == "SMART NVMe critical warning")
    {
        fields.push((
            "acquisition".into(),
            format!(
                "SMART health unavailable; exit {status}; {}",
                fields
                    .iter()
                    .filter(|(k, _)| k == "SMART message")
                    .map(|(_, v)| v.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        ));
    }
    fields.push((
        "SMART exit bitmask".into(),
        format!("{status}; health flags may accompany valid measurements"),
    ));
    fields
}

fn set_quality(t: &mut Telemetry, source: &str, prefix: &str) {
    let fields = t
        .details
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(_, v)| v)
        .collect::<Vec<_>>();
    let errors = fields
        .iter()
        .filter_map(|f| named(f, "acquisition"))
        .collect::<Vec<_>>();
    let quality = if fields.is_empty() {
        Quality::Warming
    } else if errors.is_empty() {
        Quality::Available
    } else {
        Quality::Error(format!(
            "{} of {} objects failed: {}",
            errors.len(),
            fields.len(),
            errors.join("; ")
        ))
    };
    t.capabilities.insert(source.into(), quality);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn health_errors_are_not_success() {
        let denied = serde_json::json!({"smartctl":{"messages":[{"string":"Permission denied"}]}});
        assert!(named(&storage_fields(&denied, 2), "acquisition")
            .unwrap()
            .contains("Permission denied"));
        let failing =
            serde_json::json!({"smart_status":{"passed":false},"temperature":{"current":51}});
        let fields = storage_fields(&failing, 8);
        assert!(named(&fields, "acquisition").is_none());
        assert_eq!(named(&fields, "SMART health").as_deref(), Some("false"));
    }
    #[test]
    fn mixed_acquisition_is_visible() {
        let mut t = Telemetry::default();
        t.details
            .insert("task:1".into(), vec![("runtime".into(), "12".into())]);
        t.details
            .insert("task:2".into(), unavailable("process exited"));
        set_quality(&mut t, "task_metadata", "task:");
        assert!(
            matches!(&t.capabilities["task_metadata"],Quality::Error(s) if s.contains("1 of 2"))
        );
    }
    #[test]
    fn module_records_preserve_colons_and_parameters() {
        assert_eq!(
            pairs("filename: /lib/a.ko\0parm: mode:description\0", '\0')[1].1,
            "mode:description"
        );
    }
    #[test]
    fn unit_override_precedence() {
        let root = std::env::temp_dir().join(format!("kernwatch-unit-test-{}", std::process::id()));
        for dir in ["usr/lib", "run", "etc"] {
            let path = root.join(format!("{dir}/systemd/system/example.service.d"));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("10-cpu.conf"), dir).unwrap();
        }
        let fields = unit_files(&root, "example.service");
        let dropins = fields
            .iter()
            .filter(|(k, _)| k.starts_with("drop-in"))
            .collect::<Vec<_>>();
        assert_eq!(dropins.len(), 1);
        assert_eq!(dropins[0].1, "etc");
        fs::remove_dir_all(root).unwrap();
    }
}
