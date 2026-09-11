# Gap audit and implementation

11 September 2026. This audit separates missing code, inaccessible sources and information that the selected kernel interfaces do not retain. An “unavailable” label alone does not satisfy a feature requirement.

## Implemented in this pass

| Gap | Implementation | Bounds and evidence |
|---|---|---|
| Module file, signer and vermagic metadata | `modinfo` records, parameters, source-version comparison and provenance in the module inspector | Eight modules per worker pass; 60-second cache keyed by loaded sysfs identity. On-disk signer metadata is not independent signature verification. Parser and host acquisition checks. |
| Systemd overrides and transient properties | Manager properties and manager-ordered unit/drop-in text; filesystem fallback with same-filename precedence | Four units per pass; 30-second cache keyed by cgroup inode. Exact-name filesystem fallback explicitly excludes manager-only/template/type/prefix resolution. Override precedence test. |
| Storage health | JSON SMART/NVMe health, temperature, wear, errors, ATA attributes and diagnostic messages | Eight whole devices per pass; 60-second cache. Standby-aware request. Nonzero health flags preserve actual failing measurements; acquisition errors stay errors. No device writes. |
| Scheduler internals | Per-task scheduler weight, vruntime, switches, migrations and schedstat counters where exported | Thirty-two tasks per pass with rotation; five-second cache and PID/start identity checks before and after reads. Inspector includes sample age. |
| Kernel/audit journal ingestion | Current-boot JSON journal, bounded cursor polling, visible-access warnings and retained records | Five-second polling; 300 records; 900 ms command deadline. Does not configure audit rules or daemons. Malformed/foreign-boot records tested. |
| Events discarded during refresh | Merge kernel-log events with topology/journal events by source identity | IRQ and module change events survive kernel-log refresh. Sources remain separately labelled. |
| External BPF metadata | Creator UID, verifier processed instructions, BPF-link targets and translated static helper targets | Direct Linux UAPI; 4,096-link cap and 1 MiB program cap. Symbol resolution uses visible `__bpf_call_base` and exact kallsyms matches. Static call targets are not helper execution cost. Privileged VM checks exercise real links and programs. |
| Slow optional tools on the sampling path | Dedicated metadata and BPF workers; bounded channels, subprocess output and deadlines, per-object caches | No shell execution. Timeout/output-cap tests; subprocesses die when their owner exits. Metadata merges reject reused task/module/cgroup identities. |
| Missing acquisition overview | `: capabilities` displays source quality, failure reason and sampling metadata | Inactive probes show stopped, rather than incorrectly claiming unsupported kernel functionality. Workflow test covers inspector and return navigation. |
| Incomplete report publication | Write and sync a staging directory, then publish with no-replace rename | Eight files; failed writes clean up staging. A process crash can leave a hidden `.partial` directory, never a completed-looking report prefix. Existing export/manifest tests exercise final contents. |
| Tracefs instance cleanup | Close the reader before removing the owned instance | Cleanup fix only; this does not turn the tracefs prototype into a supported interactive backend. |

Optional command execution is read-only, uses literal arguments and does not invoke privilege escalation. Its 900 ms deadline and 1 MiB per-stream limit can produce explicit timeout/size errors on unusually slow or large sources. Cached fields display their own sample time and age separately from worker freshness. A mixed batch with failures reports the failed-object count and reasons.

## Environment restrictions observed here

The read-only [host audit](VALIDATION.md) successfully reads module metadata and 300 visible journal records. The sandbox denies the system-manager bus connection. The exposed NVMe node cannot be opened as a device, and zram does not support SMART health. These are acquisition failures, not healthy zero values. Filesystem unit fallback still supplies inspectable configuration when present.

A host with the relevant bus/device permissions can use those existing collectors. Nothing in this pass changes system permissions, enables audit, changes global BPF statistics settings, or writes host device/control files. Privileged probe checks run in disposable VMs.

## Remaining implementation work

These are still code gaps, not reclassified as environment restrictions:

- User-stack ELF/debug symbolization, including address-space and executable identity over capture time.
- Allocation call-site attribution and module load/unload actor capture. Current module changes are polling intervals; historical loader identities cannot be reconstructed from that baseline.
- A supported interactive tracefs fallback. The existing session/parser is not wired to the command path with equivalent typed hydration and coverage checks.
- Bounded occupancy sampling for compatible BPF map types, and process FD-holder attribution. Creator UID and link targets are now implemented; neither identifies a historical loader PID.
- Cross-suspend clock alignment for historical kernel/journal records against the boot-time dashboard. Current-boot filtering prevents cross-boot mixing, but does not resolve historical suspend offsets.

The completion ledger marks affected screens partial. This build is not asserted to satisfy every original specification row.

## Information requiring new evidence

Past affinity-change actors, pre-monitoring module load timestamps, unrecorded latency distributions, external helper execution time and arbitrary driver queue-to-IRQ causality cannot be reconstructed from aggregate counters. Capturing a new event can establish new evidence; it cannot manufacture the missing history. Kernel restrictions on translated instructions or symbol addresses remain explicit.

Interface references: [Linux BPF UAPI](https://github.com/torvalds/linux/blob/master/include/uapi/linux/bpf.h), [bpftool translated-call symbol interpretation](https://github.com/libbpf/bpftool/blob/main/src/xlated_dumper.c), [trace event interfaces](https://docs.kernel.org/trace/events.html). Commands also follow the locally installed `modinfo`, `smartctl`, `systemctl` and `journalctl` interfaces.
