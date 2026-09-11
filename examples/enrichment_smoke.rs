#[cfg(target_os = "linux")]
mod linux {
    //! Read-only host integration audit; optional permission failures are recorded explicitly.
    pub fn main() {
        let mut inventory = kernwatch::inventory::Inventory::default();
        let mut t = kernwatch::domain::Telemetry {
            at_ms: kernwatch::enrich::monotonic_ms(),
            ..Default::default()
        };
        inventory.modules(&mut t);
        inventory.disks(&mut t);
        inventory.groups(&mut t);
        t.details.clear();
        inventory.module_metadata(&mut t);
        inventory.storage_health(&mut t);
        inventory.systemd(&mut t);
        inventory.journal(&mut t);
        println!("Capabilities: {:#?}", t.capabilities);
        for prefix in ["module:", "device:", "cgroup:"] {
            for (key, fields) in t
                .details
                .iter()
                .filter(|(key, _)| key.starts_with(prefix))
                .take(2)
            {
                println!("{key}");
                for (k, v) in fields {
                    println!("  {k}: {}", v.chars().take(500).collect::<String>());
                }
            }
        }
        println!(
            "Journal records: {}",
            t.events
                .iter()
                .filter(|e| e.source.starts_with("journal/"))
                .count()
        );
    }
}
#[cfg(target_os = "linux")]
fn main() {
    linux::main()
}
#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("This probe example requires Linux");
}
