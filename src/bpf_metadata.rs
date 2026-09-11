//! Read-only Linux BPF UAPI. Buffers and enumeration are bounded; FDs are RAII-owned.
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
#[repr(C)]
struct InfoAttr {
    fd: u32,
    len: u32,
    ptr: u64,
}
fn info<T>(fd: i32, value: &mut T) -> io::Result<()> {
    let attr = InfoAttr {
        fd: fd as u32,
        len: std::mem::size_of::<T>() as u32,
        ptr: value as *mut T as u64,
    };
    // SAFETY: initialized repr(C) attr points to a live writable T of the supplied length.
    let rc = unsafe { libc::syscall(libc::SYS_bpf, 15, &attr, std::mem::size_of::<InfoAttr>()) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[repr(C)]
#[derive(Default)]
struct LinkInfo {
    kind: u32,
    id: u32,
    program: u32,
    pad: u32,
    data: [u64; 8],
}
pub fn links() -> io::Result<BTreeMap<u32, Vec<String>>> {
    let mut result = BTreeMap::<u32, Vec<String>>::new();
    let mut id = 0;
    for _ in 0..4096 {
        let mut attr = [id, 0u32, 0];
        // SAFETY: initialized 12-byte get-next-ID attr; kernel writes next ID.
        if unsafe { libc::syscall(libc::SYS_bpf, 31, attr.as_mut_ptr(), 12usize) } < 0 {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ENOENT) {
                return Ok(result);
            }
            return Err(e);
        }
        id = attr[1];
        let attr = [id, 0u32, 0];
        // SAFETY: initialized get-FD-by-ID attr; returned descriptor is immediately owned.
        let fd = unsafe { libc::syscall(libc::SYS_bpf, 30, attr.as_ptr(), 12usize) };
        if fd < 0 {
            continue;
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
        let mut data = LinkInfo::default();
        info(fd.as_raw_fd(), &mut data)?;
        let target = match data.kind {
            1 => {
                let mut name = [0u8; 256];
                data.data[0] = name.as_mut_ptr() as u64;
                data.data[1] = 256;
                match info(fd.as_raw_fd(), &mut data) {
                    Ok(()) => format!(
                        "raw tracepoint {}",
                        String::from_utf8_lossy(
                            &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())]
                        )
                    ),
                    Err(e) => format!("raw tracepoint name: {e}"),
                }
            }
            2 => format!(
                "tracing attach={} target object={} BTF={}",
                data.data[0] as u32,
                (data.data[0] >> 32) as u32,
                data.data[1] as u32
            ),
            3 => format!("cgroup {} attach={}", data.data[0], data.data[1] as u32),
            6 => format!("XDP ifindex={}", data.data[0] as u32),
            kind => format!("kernel link type {kind}"),
        };
        result
            .entry(data.program)
            .or_default()
            .push(format!("link {}: {target}", data.id));
    }
    Err(io::Error::other("BPF link scan reached 4096-object cap"))
}
#[repr(C)]
#[derive(Default)]
struct ProgramInfo {
    kind: u32,
    id: u32,
    tag: [u8; 8],
    jited_len: u32,
    xlated_len: u32,
    jited_ptr: u64,
    xlated_ptr: u64,
}
pub fn helpers(fd: i32, bytes: u32) -> io::Result<String> {
    if bytes > 1024 * 1024 {
        return Err(io::Error::other(
            "translated program exceeds 1 MiB inspection cap",
        ));
    }
    let mut insns = vec![0u8; bytes as usize];
    let mut data = ProgramInfo {
        xlated_len: bytes,
        xlated_ptr: insns.as_mut_ptr() as u64,
        ..Default::default()
    };
    info(fd, &mut data)?;
    if bytes > 0 && data.xlated_len == 0 {
        return Err(io::Error::other("kernel withheld translated instructions"));
    }
    if data.xlated_len > bytes {
        return Err(io::Error::other("translated length changed"));
    }
    let offsets = helper_offsets(&insns[..data.xlated_len as usize]);
    let resolved = SYMBOLS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed().as_secs() >= 60)
        {
            let symbols = crate::enrichment::bounded_file("/proc/kallsyms", 64 * 1024 * 1024)
                .map(|raw| parse_symbols(&raw))
                .map_err(|e| e.to_string());
            *cache = Some((std::time::Instant::now(), symbols));
        }
        match &cache.as_ref().unwrap().1 {
            Ok((base, symbols)) if *base != 0 => offsets
                .iter()
                .map(|offset| {
                    symbols
                        .get(&base.wrapping_add_signed(*offset as i64))
                        .cloned()
                        .unwrap_or_else(|| format!("unresolved call offset {offset:+}"))
                })
                .collect::<Vec<_>>()
                .join(", "),
            Ok(_) => format!("symbol addresses restricted; translated call offsets {offsets:?}"),
            Err(e) => format!("symbol acquisition: {e}; translated call offsets {offsets:?}"),
        }
    });
    Ok(format!("{resolved}; static helper call targets, not per-helper time; kfunc and BPF-to-BPF calls excluded"))
}
type Symbols = (u64, BTreeMap<u64, String>);
type SymbolCache = Option<(std::time::Instant, Result<Symbols, String>)>;
thread_local! {static SYMBOLS: std::cell::RefCell<SymbolCache> = const {std::cell::RefCell::new(None)};}
fn parse_symbols(raw: &str) -> Symbols {
    let mut base = 0;
    let mut symbols = BTreeMap::new();
    for line in raw.lines() {
        let mut words = line.split_whitespace();
        if let (Some(address), Some(_), Some(name)) = (words.next(), words.next(), words.next()) {
            if let Ok(address) = u64::from_str_radix(address, 16) {
                if name == "__bpf_call_base" {
                    base = address;
                }
                if address != 0 {
                    symbols.insert(address, name.into());
                }
            }
        }
    }
    (base, symbols)
}

pub fn helper_offsets(bytes: &[u8]) -> BTreeSet<i32> {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .filter(|i| i[0] == 0x85 && i[1] >> 4 == 0)
        .map(|i| i32::from_ne_bytes(i[4..8].try_into().unwrap()))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers_exclude_pseudo_calls() {
        let (base,symbols)=parse_symbols("0000000000001000 T __bpf_call_base\n0000000000001010 T bpf_ktime_get_ns\n0000000000000000 T hidden");
        assert_eq!(
            symbols
                .get(&base.wrapping_add_signed(16))
                .map(String::as_str),
            Some("bpf_ktime_get_ns")
        );
        assert!(!symbols.contains_key(&0));
        let mut bytes = vec![];
        for (src, id) in [(0, 5i32), (1, 8), (2, 9), (0, 5), (0, 16)] {
            bytes.extend([0x85, src << 4, 0, 0]);
            bytes.extend(id.to_ne_bytes());
        }
        assert_eq!(helper_offsets(&bytes), BTreeSet::from([5, 16]));
        assert_eq!(std::mem::size_of::<ProgramInfo>(), 40);
        assert_eq!(std::mem::size_of::<InfoAttr>(), 16);
        assert_eq!(std::mem::offset_of!(LinkInfo, data), 16);
    }
}
