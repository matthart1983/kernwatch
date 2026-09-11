use crate::model::Snapshot;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};
const MAGIC: &[u8; 8] = b"KWATCH\0\x01";
const MAX_FRAME: usize = 32 * 1024 * 1024;
pub struct Recorder {
    writer: BufWriter<File>,
    pub path: PathBuf,
    pub count: u64,
}
impl Recorder {
    pub fn create(path: PathBuf) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut writer = BufWriter::new(options.open(&path)?);
        writer.write_all(MAGIC)?;
        Ok(Self {
            writer,
            path,
            count: 0,
        })
    }
    pub fn push(&mut self, s: &Snapshot) -> io::Result<()> {
        let data = serde_json::to_vec(s)?;
        if data.len() > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot exceeds 32 MiB",
            ));
        }
        self.writer.write_all(&(data.len() as u32).to_le_bytes())?;
        self.writer.write_all(&data)?;
        self.writer.flush()?;
        self.count += 1;
        Ok(())
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}
pub fn read(path: &Path) -> io::Result<Vec<Snapshot>> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut magic = [0; 8];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported kernwatch recording version",
        ));
    }
    let mut frames = Vec::new();
    let mut total = 0usize;
    loop {
        let mut size = [0; 4];
        match reader.read_exact(&mut size) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let size = u32::from_le_bytes(size) as usize;
        if size > MAX_FRAME || total + size > 512 * 1024 * 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recording exceeds replay memory limit",
            ));
        }
        let mut data = vec![0; size];
        match reader.read_exact(&mut data) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let s: Snapshot = serde_json::from_slice(&data)?;
        if s.views.len() != 13 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recording has invalid view schema",
            ));
        }
        total += size;
        frames.push(s);
    }
    Ok(frames)
}
pub fn report(s: &Snapshot) -> String {
    let mut out = format!(
        "# kernwatch incident report\n\nMode: {}\n\nHost: {} · kernel {}\n\n## Observations\n\n",
        if s.demo {
            "DEMO — synthetic fixture"
        } else {
            "LIVE"
        },
        s.telemetry.hostname,
        s.telemetry.kernel
    );
    for f in &s.findings {
        out.push_str(&format!("- {f}\n"));
    }
    for issue in &s.telemetry.issues {
        out.push_str(&format!(
            "\n## {}\n\nSubject: {} · state: {}\n\n### Evidence\n\n",
            issue.title, issue.subject, issue.state
        ));
        for e in &issue.evidence {
            out.push_str(&format!(
                "- {} — {} at {} ms ({})\n",
                e.label,
                e.source,
                e.at_ms,
                if e.observed { "observed" } else { "hypothesis" }
            ));
        }
        out.push_str("\n### Remediation\n\n");
        for step in &issue.steps {
            out.push_str(&format!("- {step}\n"));
        }
        out.push_str(&format!("\n### Verify\n\n{}\n", issue.verification));
    }
    out.push_str("\n## Evidence and limitations\n\n");
    for (name, q) in &s.telemetry.capabilities {
        out.push_str(&format!("- {name}: {q:?}\n"));
    }
    out
}
pub fn export(s: &Snapshot) -> io::Result<PathBuf> {
    export_with_actions(s, &[])
}
pub fn export_with_actions(s: &Snapshot, actions: &[crate::actions::Plan]) -> io::Result<PathBuf> {
    let path = PathBuf::from(format!("kernwatch-report-{}", stamp()));
    let files = [
        (
            "environment.json",
            serde_json::to_vec_pretty(
                &serde_json::json!({"hostname":s.telemetry.hostname,"kernel":s.telemetry.kernel,"boot_id":s.telemetry.boot_id,"captured_wall_seconds":s.captured,"monotonic_ms":s.telemetry.at_ms,"demo":s.demo,"capabilities":s.telemetry.capabilities,"trace_drops":s.telemetry.trace_drops}),
            )?,
        ),
        ("actions.json", serde_json::to_vec_pretty(actions)?),
        ("snapshot.json", serde_json::to_vec_pretty(s)?),
        (
            "report.json",
            serde_json::to_vec_pretty(&s.telemetry.issues)?,
        ),
        (
            "events.json",
            serde_json::to_vec_pretty(&s.telemetry.events)?,
        ),
        (
            "series.json",
            serde_json::to_vec_pretty(&s.telemetry.series)?,
        ),
        ("report.md", report(s).into_bytes()),
    ];
    let staging = PathBuf::from(format!(".kernwatch-report-{}.partial", stamp()));
    std::fs::create_dir(&staging)?;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(staging.clone());
    let write = |name: &str, data: &[u8]| -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(staging.join(name))?;
        file.write_all(data)?;
        file.sync_all()
    };
    let mut manifest = Vec::new();
    for (name, data) in files {
        write(name, &data)?;
        manifest.push(serde_json::json!({"file":name,"bytes":data.len()}));
    }
    write(
        "manifest.json",
        &serde_json::to_vec_pretty(
            &serde_json::json!({"version":1,"demo":s.demo,"captured":s.captured,"files":manifest}),
        )?,
    )?;
    publish_directory(&staging, &path)?;
    #[cfg(unix)]
    std::fs::File::open(".")?.sync_all()?;
    Ok(path)
}
pub fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(target_os = "linux")]
fn publish_directory(staging: &Path, path: &Path) -> io::Result<()> {
    std::fs::File::open(staging)?.sync_all()?;
    let from =
        std::ffi::CString::new(staging.as_os_str().as_encoded_bytes()).map_err(io::Error::other)?;
    let to =
        std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).map_err(io::Error::other)?;
    // Use the kernel syscall directly: musl does not expose a renameat2 wrapper.
    // SAFETY: valid NUL-terminated paths; Linux no-replace rename publishes the complete directory.
    if unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}
#[cfg(target_os = "macos")]
fn publish_directory(staging: &Path, path: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let from = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
    let to = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    File::open(staging)?.sync_all()?;
    // SAFETY: both paths are valid NUL-terminated strings; RENAME_EXCL forbids replacement.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(windows)]
fn publish_directory(staging: &Path, path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<_> = staging.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: terminated UTF-16 paths; WRITE_THROUGH is set and REPLACE_EXISTING is absent.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 8) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
