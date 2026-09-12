//! Address to name resolution for captured stacks.
//!
//! Every answer is derived from something read off the host: `/proc/kallsyms`
//! for kernel text, the mapped file's own symbol table for user text. An
//! address outside every known symbol keeps its hexadecimal form; the nearest
//! preceding symbol is never borrowed across a gap, because a wrong name in a
//! profile is worse than an address.
use std::collections::BTreeMap;

/// A resolved function: where it starts, how long it is, what it is called.
#[derive(Clone, Debug, PartialEq)]
struct Symbol {
    value: u64,
    size: u64,
    name: String,
}

/// Longest run a symbol of unknown length is assumed to cover. Kernel symbols
/// carry no size in `/proc/kallsyms`, so the next symbol's address bounds them
/// and the final symbol is bounded by this instead of running to infinity.
const UNSIZED_LIMIT: u64 = 64 * 1024;

fn lookup(table: &[Symbol], address: u64) -> Option<&Symbol> {
    let index = match table.binary_search_by(|s| s.value.cmp(&address)) {
        Ok(exact) => exact,
        Err(0) => return None,
        Err(after) => after - 1,
    };
    let symbol = table.get(index)?;
    let span = if symbol.size > 0 {
        symbol.size
    } else {
        table
            .get(index + 1)
            .map(|next| next.value.saturating_sub(symbol.value))
            .filter(|span| *span > 0)
            .unwrap_or(UNSIZED_LIMIT)
            .min(UNSIZED_LIMIT)
    };
    (address < symbol.value.saturating_add(span)).then_some(symbol)
}

/// Parse `/proc/kallsyms` content. Text, data and read-only symbols are kept;
/// the addresses are zero when `kptr_restrict` hides them, which the caller
/// detects rather than rendering a profile of nothing.
fn parse_kallsyms(text: &str) -> Vec<Symbol> {
    let mut out: Vec<Symbol> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let value = u64::from_str_radix(parts.next()?, 16).ok()?;
            let kind = parts.next()?;
            if !matches!(kind, "t" | "T" | "w" | "W") {
                return None;
            }
            Some(Symbol {
                value,
                size: 0,
                name: parts.next()?.to_owned(),
            })
        })
        .collect();
    out.sort_by_key(|s| s.value);
    out.dedup_by_key(|s| s.value);
    out
}

/// One mapped region of a process address space.
#[derive(Clone, Debug, PartialEq)]
struct Mapping {
    start: u64,
    end: u64,
    offset: u64,
    path: String,
}

/// Parse `/proc/PID/maps`, keeping executable file-backed regions. Anonymous
/// and non-executable regions cannot contain a resolvable return address.
fn parse_maps(text: &str) -> Vec<Mapping> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let (start, end) = parts.next()?.split_once('-')?;
            let permissions = parts.next()?;
            if !permissions.contains('x') {
                return None;
            }
            let offset = u64::from_str_radix(parts.next()?, 16).ok()?;
            parts.next()?;
            parts.next()?;
            let path = parts.collect::<Vec<_>>().join(" ");
            if path.ends_with(" (deleted)") {
                return None;
            }
            if !path.starts_with('/') {
                return None;
            }
            Some(Mapping {
                start: u64::from_str_radix(start, 16).ok()?,
                end: u64::from_str_radix(end, 16).ok()?,
                offset,
                path: path.to_owned(),
            })
        })
        .collect()
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

/// A parsed ELF image: its function symbols, and the load segments needed to
/// turn a mapped file offset back into the virtual address the symbols use.
#[derive(Clone, Debug, Default, PartialEq)]
struct Image {
    fingerprint: u64,
    symbols: Vec<Symbol>,
    /// `(file offset, virtual address, length)` per PT_LOAD segment.
    loads: Vec<(u64, u64, u64)>,
}
impl Image {
    /// Where a mapped file offset lives in the image's own address space.
    fn vaddr(&self, file_offset: u64) -> Option<u64> {
        self.loads
            .iter()
            .find(|(offset, _, length)| {
                file_offset >= *offset && file_offset < offset.saturating_add(*length)
            })
            .map(|(offset, vaddr, _)| file_offset - offset + vaddr)
    }
}

/// Parse a 64-bit little-endian ELF file. Other classes are reported as
/// unparsed rather than guessed at; their frames keep their addresses.
fn parse_elf(bytes: &[u8]) -> Option<Image> {
    if bytes.get(..4)? != b"\x7fELF" || bytes.get(4)? != &2 || bytes.get(5)? != &1 {
        return None;
    }
    let mut image = Image {
        fingerprint: bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
        }),
        ..Default::default()
    };
    let phoff = u64_at(bytes, 0x20)? as usize;
    let phentsize = u16_at(bytes, 0x36)? as usize;
    let phnum = u16_at(bytes, 0x38)? as usize;
    for i in 0..phnum {
        let at = phoff + i * phentsize;
        if u32_at(bytes, at)? == 1 {
            image.loads.push((
                u64_at(bytes, at + 0x08)?,
                u64_at(bytes, at + 0x10)?,
                u64_at(bytes, at + 0x20)?,
            ));
        }
    }
    let shoff = u64_at(bytes, 0x28)? as usize;
    let shentsize = u16_at(bytes, 0x3a)? as usize;
    let shnum = u16_at(bytes, 0x3c)? as usize;
    let section = |i: usize| -> Option<(u32, u64, u64, u32, u64)> {
        let at = shoff + i * shentsize;
        Some((
            u32_at(bytes, at + 4)?,    // sh_type
            u64_at(bytes, at + 0x18)?, // sh_offset
            u64_at(bytes, at + 0x20)?, // sh_size
            u32_at(bytes, at + 0x28)?, // sh_link
            u64_at(bytes, at + 0x38)?, // sh_entsize
        ))
    };
    for i in 0..shnum {
        let Some((kind, offset, size, link, entsize)) = section(i) else {
            continue;
        };
        // SHT_SYMTAB and SHT_DYNSYM. A stripped binary has neither, and its
        // frames stay as addresses.
        if kind != 2 && kind != 11 || entsize == 0 {
            continue;
        }
        let Some((_, strings_at, strings_len, _, _)) = section(link as usize) else {
            continue;
        };
        let strings = bytes.get(strings_at as usize..(strings_at + strings_len) as usize)?;
        for entry in 0..(size / entsize) as usize {
            let at = offset as usize + entry * entsize as usize;
            let Some(info) = bytes.get(at + 4) else {
                continue;
            };
            // STT_FUNC only: data symbols never appear in a call stack.
            if info & 0xf != 2 {
                continue;
            }
            let (Some(name_at), Some(value), Some(size)) = (
                u32_at(bytes, at),
                u64_at(bytes, at + 8),
                u64_at(bytes, at + 16),
            ) else {
                continue;
            };
            if value == 0 {
                continue;
            }
            let name = strings
                .get(name_at as usize..)
                .and_then(|s| s.split(|b| *b == 0).next())
                .map(String::from_utf8_lossy)
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }
            image.symbols.push(Symbol { value, size, name });
        }
    }
    image.symbols.sort_by_key(|s| s.value);
    image.symbols.dedup_by_key(|s| s.value);
    Some(image)
}

type ImageVersion = (u64, u64, u64, i64, i64, i64, i64);

/// Resolves addresses for as long as the processes behind them exist.
#[derive(Default)]
pub struct Symbols {
    kernel: Vec<Symbol>,
    /// Set when `/proc/kallsyms` reported every address as zero.
    pub kernel_restricted: bool,
    maps: BTreeMap<u32, Vec<Mapping>>,
    images: BTreeMap<String, Option<Image>>,
    image_versions: BTreeMap<String, ImageVersion>,
    checked_images: std::collections::BTreeSet<String>,
}
impl Symbols {
    /// Read kernel symbols from the host. Absent or restricted symbols leave
    /// the table empty and set `kernel_restricted`.
    pub fn load_kernel(&mut self) {
        let text = std::fs::read_to_string("/proc/kallsyms").unwrap_or_default();
        self.kernel = parse_kallsyms(&text);
        self.kernel_restricted =
            !self.kernel.is_empty() && self.kernel.iter().all(|s| s.value == 0);
        if self.kernel_restricted {
            self.kernel.clear();
        }
    }
    pub fn kernel_loaded(&self) -> bool {
        !self.kernel.is_empty()
    }
    /// Name for a kernel text address, or None to keep the address.
    pub fn kernel(&self, address: u64) -> Option<&str> {
        lookup(&self.kernel, address).map(|s| s.name.as_str())
    }
    /// Name for an address in a process, or None to keep the address. A
    /// process that has exited resolves nothing; its stacks keep addresses.
    pub fn user(&mut self, pid: u32, address: u64) -> Option<String> {
        let mapping = self
            .maps
            .entry(pid)
            .or_insert_with(|| {
                parse_maps(
                    &std::fs::read_to_string(format!("/proc/{pid}/maps")).unwrap_or_default(),
                )
            })
            .iter()
            .find(|m| address >= m.start && address < m.end)?
            .clone();
        use std::os::unix::fs::MetadataExt;
        let file = format!("/proc/{pid}/root{}", mapping.path);
        if self.checked_images.insert(mapping.path.clone()) {
            let stat = std::fs::metadata(&file).ok()?;
            let version = (
                stat.dev(),
                stat.ino(),
                stat.len(),
                stat.mtime(),
                stat.mtime_nsec(),
                stat.ctime(),
                stat.ctime_nsec(),
            );
            if self.image_versions.get(&mapping.path) != Some(&version) {
                self.images.remove(&mapping.path);
                self.image_versions.insert(mapping.path.clone(), version);
            }
        }
        let image = self
            .images
            .entry(mapping.path.clone())
            .or_insert_with(|| std::fs::read(&file).ok().as_deref().and_then(parse_elf));
        let image = image.as_ref()?;
        let vaddr = image.vaddr(address - mapping.start + mapping.offset)?;
        lookup(&image.symbols, vaddr)
            .filter(|s| s.size > 0 || s.value == vaddr)
            .map(|s| s.name.clone())
    }
    /// Stable image-relative identity, separate from presentation and ASLR.
    pub fn identified(
        &mut self,
        pid: u32,
        address: u64,
        kernel: bool,
    ) -> (String, crate::flame::FrameInfo) {
        let raw = self.frame(pid, address, kernel);
        let (image, offset) = if kernel {
            ("kernel".to_owned(), address)
        } else {
            self.maps
                .get(&pid)
                .and_then(|maps| maps.iter().find(|m| address >= m.start && address < m.end))
                .map(|m| {
                    let offset = address - m.start + m.offset;
                    let image = self.images.get(&m.path).and_then(|i| i.as_ref());
                    let symbol = image
                        .and_then(|i| i.vaddr(offset).and_then(|a| lookup(&i.symbols, a)))
                        .map(|s| s.value);
                    (
                        format!("{}#{:016x}", m.path, image.map_or(0, |i| i.fingerprint)),
                        if raw.starts_with("0x") {
                            offset
                        } else {
                            symbol.unwrap_or(offset)
                        },
                    )
                })
                .unwrap_or_else(|| (format!("unmapped-pid-{pid}"), address))
        };
        // Resolved functions merge their return addresses; unresolved addresses
        // remain distinct. Image path is retained to disambiguate shared names.
        let identity = if raw.starts_with("0x") {
            format!("{image}@{offset:x}")
        } else {
            if kernel {
                format!("{image}!{raw}")
            } else {
                format!("{image}@{offset:x}!{raw}")
            }
        };
        let info = crate::flame::FrameInfo {
            display: demangle(&raw),
            raw,
            image,
            address: offset,
            kernel,
        };
        (identity, info)
    }
    /// Forget a process whose identity may have been reused.
    pub fn forget(&mut self, pid: u32) {
        self.maps.remove(&pid);
        self.checked_images.clear();
    }
    /// Render one frame: a resolved name, or the address it stays as.
    pub fn frame(&mut self, pid: u32, address: u64, kernel: bool) -> String {
        let resolved = if kernel {
            self.kernel(address).map(str::to_owned)
        } else {
            self.user(pid, address)
        };
        resolved.unwrap_or_else(|| format!("0x{address:x}"))
    }
}

/// Parse demangling strictly; preserve unknown or malformed names.
pub fn demangle(name: &str) -> String {
    if let Ok(symbol) = rustc_demangle::try_demangle(name) {
        return format!("{symbol:#}");
    }
    if let Ok(symbol) = cpp_demangle::Symbol::new(name) {
        if let Ok(name) = symbol.demangle(&cpp_demangle::DemangleOptions::default()) {
            return name;
        }
    }
    name.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn symbol(value: u64, size: u64, name: &str) -> Symbol {
        Symbol {
            value,
            size,
            name: name.into(),
        }
    }
    #[test]
    fn a_sized_symbol_covers_its_own_extent_and_nothing_after_it() {
        let table = [symbol(0x1000, 0x20, "start"), symbol(0x2000, 0x10, "later")];
        assert_eq!(lookup(&table, 0x1000).unwrap().name, "start");
        assert_eq!(lookup(&table, 0x101f).unwrap().name, "start");
        // Past the end of `start` but before `later` belongs to neither.
        assert!(lookup(&table, 0x1020).is_none());
        assert_eq!(lookup(&table, 0x2008).unwrap().name, "later");
        assert!(lookup(&table, 0x2010).is_none());
        // Below the first symbol there is nothing to borrow from.
        assert!(lookup(&table, 0x0fff).is_none());
        assert!(lookup(&[], 0x1000).is_none());
    }
    #[test]
    fn an_unsized_symbol_is_bounded_by_the_next_one_or_a_ceiling() {
        let table = [symbol(0x1000, 0, "first"), symbol(0x1100, 0, "second")];
        assert_eq!(lookup(&table, 0x10ff).unwrap().name, "first");
        assert_eq!(lookup(&table, 0x1100).unwrap().name, "second");
        // The last unsized symbol does not run to the end of memory.
        assert_eq!(
            lookup(&table, 0x1100 + UNSIZED_LIMIT - 1).unwrap().name,
            "second"
        );
        assert!(lookup(&table, 0x1100 + UNSIZED_LIMIT).is_none());
    }
    #[test]
    fn kallsyms_keeps_text_symbols_in_address_order() {
        let text = "ffffffff81000200 T second_fn\n\
                    ffffffff81000100 t first_fn\n\
                    ffffffff81000300 D a_data_symbol\n\
                    garbage line\n";
        let table = parse_kallsyms(text);
        assert_eq!(
            table.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["first_fn", "second_fn"],
            "data symbols never appear in a call stack"
        );
        assert_eq!(lookup(&table, 0xffffffff81000180).unwrap().name, "first_fn");
    }
    #[test]
    fn maps_keeps_only_executable_file_backed_regions() {
        let text = "\
55a1b2c00000-55a1b2c01000 r-xp 00001000 fd:00 131 /usr/bin/demo
55a1b2c01000-55a1b2c02000 rw-p 00002000 fd:00 131 /usr/bin/demo
7f0000000000-7f0000001000 r-xp 00000000 00:00 0 \n\
7ffd00000000-7ffd00021000 rwxp 00000000 00:00 0 [stack]
";
        let maps = parse_maps(text);
        assert_eq!(maps.len(), 1, "only the executable file mapping resolves");
        assert_eq!(maps[0].path, "/usr/bin/demo");
        assert_eq!(maps[0].offset, 0x1000);
    }
    #[test]
    fn a_mapped_offset_becomes_the_virtual_address_its_symbols_use() {
        // A PIE text segment mapped at a file offset that differs from its
        // virtual address: resolving without this step names the wrong symbol.
        let image = Image {
            fingerprint: 0,
            symbols: vec![symbol(0x3000, 0x40, "work")],
            loads: vec![(0x2000, 0x3000, 0x1000)],
        };
        assert_eq!(image.vaddr(0x2010), Some(0x3010));
        assert_eq!(lookup(&image.symbols, 0x3010).unwrap().name, "work");
        // Outside every load segment there is no address to resolve.
        assert_eq!(image.vaddr(0x9999), None);
    }
    #[test]
    fn a_file_that_is_not_an_elf_image_is_refused_rather_than_misread() {
        assert!(parse_elf(b"not an elf at all").is_none());
        assert!(parse_elf(b"").is_none());
        // 32-bit ELF: class byte 1.
        assert!(parse_elf(b"\x7fELF\x01\x01\x01\x00________________").is_none());
    }
    #[test]
    fn an_unresolvable_frame_keeps_its_address() {
        let mut symbols = Symbols::default();
        assert_eq!(symbols.frame(1, 0xdeadbeef, true), "0xdeadbeef");
        // A pid that cannot be read resolves nothing and invents nothing.
        assert_eq!(symbols.frame(u32::MAX, 0x1000, false), "0x1000");
    }
    #[test]
    fn this_host_resolves_its_own_text_when_symbols_are_readable() {
        // Uses the running process, so it exercises the real maps and ELF
        // paths rather than a fixture. Skips where symbols are stripped.
        let mut symbols = Symbols::default();
        let here: fn() = this_host_resolves_its_own_text_when_symbols_are_readable;
        let address = here as *const () as usize as u64;
        let pid = std::process::id();
        if let Some(name) = symbols.user(pid, address) {
            assert!(
                name.contains("this_host_resolves"),
                "resolved {name} for this test's own address"
            );
        }
    }
}
