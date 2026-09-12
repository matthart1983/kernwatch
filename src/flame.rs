//! Stack aggregation and icicle layout.
//!
//! A profile counts identical stacks; it never weights a stack it did not
//! observe. Frames that could not be resolved keep their address rather than
//! borrowing a neighbouring symbol's name, and stacks that end too shallow to
//! be a real call path are counted so the panel can say how many there were.
use serde::{Deserialize, Serialize};

/// Shallow stacks can indicate an incomplete frame-pointer walk. This is a
/// heuristic, not proof: legitimately short call paths also occur.
pub const SHALLOW: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub name: String,
    pub samples: u64,
    pub children: Vec<Node>,
}
impl Node {
    fn child(&mut self, name: &str) -> &mut Node {
        if let Some(index) = self.children.iter().position(|c| c.name == name) {
            return &mut self.children[index];
        }
        self.children.push(Node {
            name: name.to_owned(),
            ..Default::default()
        });
        self.children.last_mut().expect("just pushed")
    }
    /// Samples in this frame itself: what the children do not account for.
    pub fn own(&self) -> u64 {
        self.samples
            .saturating_sub(self.children.iter().map(|c| c.samples).sum())
    }
    fn sort(&mut self) {
        self.children
            .sort_by(|a, b| b.samples.cmp(&a.samples).then_with(|| a.name.cmp(&b.name)));
        for child in &mut self.children {
            child.sort();
        }
    }
    fn fold(&self, prefix: &str, out: &mut Vec<String>, profile: &Profile) {
        let name = profile
            .label(&self.name)
            .replace(';', ":")
            .replace(['\n', '\r'], " ");
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix};{name}")
        };
        let own = self.own();
        if own > 0 {
            out.push(format!("{path} {own}"));
        }
        for child in &self.children {
            child.fold(&path, out, profile);
        }
    }
    /// Levels below this frame, counting itself as one.
    pub fn depth(&self) -> u16 {
        1 + self.children.iter().map(Node::depth).max().unwrap_or(0)
    }
    /// The subtree at `path`, for zooming into one frame.
    pub fn at(&self, path: &[String]) -> Option<&Node> {
        match path.split_first() {
            None => Some(self),
            Some((head, rest)) => self.children.iter().find(|c| c.name == *head)?.at(rest),
        }
    }
}

/// Capture semantics are explicit; old recordings remain Unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Unknown,
    Syscalls,
    Cpu,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub kind: Kind,
    pub scope: String,
    pub unit: String,
    pub duration_seconds: u64,
    pub elapsed_seconds: f64,
    pub frequency_hz: u64,
    pub cpus: Vec<u32>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Quality {
    pub attempted: u64,
    pub user_stacks: u64,
    pub kernel_stacks: u64,
    pub partial: u64,
    pub failed: u64,
    pub depth_limit: u64,
    pub map_failures: u64,
    pub errors: std::collections::BTreeMap<String, u64>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameInfo {
    pub raw: String,
    pub display: String,
    pub image: String,
    pub address: u64,
    pub kernel: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Shared<T>(std::sync::Arc<T>);
impl<T> Shared<T> {
    pub fn same_version(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Self(std::sync::Arc::new(value))
    }
}
impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: Clone> std::ops::DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileData {
    #[serde(default)]
    pub tasks: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub quality: Quality,
    #[serde(default)]
    pub frames: Shared<std::collections::BTreeMap<String, FrameInfo>>,
    pub root: Shared<Node>,
    /// What produced these stacks, named the way the capture named it.
    pub source: String,
    /// Samples whose stack ended at or below `SHALLOW` frames.
    pub shallow: u64,
    /// Samples whose stack still carries at least one unresolved address.
    pub unresolved: u64,
}
/// Copy-on-write capture. Retained frames and render caches share an immutable
/// version; direct field mutation also detaches, so cached identities stay valid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Profile(std::sync::Arc<ProfileData>);
impl std::ops::Deref for Profile {
    type Target = ProfileData;
    fn deref(&self) -> &ProfileData {
        &self.0
    }
}
impl std::ops::DerefMut for Profile {
    fn deref_mut(&mut self) -> &mut ProfileData {
        std::sync::Arc::make_mut(&mut self.0)
    }
}
impl Profile {
    pub fn same_version(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Profile {
    pub fn new(source: &str) -> Self {
        Self(std::sync::Arc::new(ProfileData {
            root: Node {
                name: "all".into(),
                ..Default::default()
            }
            .into(),
            source: source.to_owned(),
            ..Default::default()
        }))
    }
    /// Fold one observed stack, innermost frame last.
    pub fn add(&mut self, frames: &[String], weight: u64) {
        if frames.is_empty() || weight == 0 {
            return;
        }
        // Counters are weighted like the tree, so every share the panel
        // reports is a share of observations rather than of insertions.
        if frames.len() <= SHALLOW {
            self.shallow += weight;
        }
        if frames.iter().any(|f| f.starts_with("0x")) {
            self.unresolved += weight;
        }
        self.root.samples += weight;
        let mut node: &mut Node = &mut self.root;
        for frame in frames {
            node = node.child(frame);
            node.samples += weight;
        }
    }
    pub fn label<'a>(&'a self, identity: &'a str) -> &'a str {
        self.frames
            .get(identity)
            .map_or(identity, |f| f.display.as_str())
    }
    /// User quality is measured before adding domain/process frames.
    pub fn observe(&mut self, user: &[String], kernel: &[String], weight: u64) {
        if !user.is_empty() {
            self.quality.user_stacks += weight;
        }
        if !kernel.is_empty() {
            self.quality.kernel_stacks += weight;
        }
        if user.len() == 127 || kernel.len() == 127 {
            self.quality.depth_limit += weight;
        }
        if user.is_empty() && kernel.is_empty() {
            self.quality.failed += weight;
            return;
        }
        if self.metadata.kind == Kind::Cpu && (user.is_empty() || kernel.is_empty()) {
            self.quality.partial += weight;
        }
        let mut frames = user.to_vec();
        if !kernel.is_empty() {
            frames.push("[kernel]".into());
            frames.extend_from_slice(kernel);
        }
        let before = self.shallow;
        let unresolved = self.unresolved;
        self.add(&frames, weight);
        self.shallow = before
            + if !user.is_empty() && user.len() <= SHALLOW {
                weight
            } else {
                0
            };
        self.unresolved = unresolved
            + if frames.iter().any(|id| {
                self.frames
                    .get(id)
                    .map_or_else(|| id.starts_with("0x"), |f| f.raw.starts_with("0x"))
            }) {
                weight
            } else {
                0
            };
    }
    pub fn sort(&mut self) {
        self.root.sort();
    }
    pub fn is_empty(&self) -> bool {
        self.root.samples == 0
    }
    /// Folded stacks, the interchange format flamegraph.pl and speedscope read.
    pub fn folded(&self) -> Vec<String> {
        let mut out = Vec::new();
        for child in &self.root.children {
            child.fold("", &mut out, self);
        }
        out
    }
    /// Share of stacks too shallow to be a believable call path, if any were
    /// collected at all. Reported rather than corrected: without frame
    /// pointers a truncated stack is indistinguishable from a short one.
    pub fn shallow_share(&self) -> Option<f64> {
        let denominator = if self.metadata.kind == Kind::Cpu {
            self.quality.user_stacks
        } else {
            self.root.samples
        };
        (denominator > 0).then(|| self.shallow as f64 / denominator as f64 * 100.)
    }
}

/// One frame's place in the icicle.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub delta: Option<f64>,
    pub depth: u16,
    pub x: u16,
    pub width: u16,
    pub name: String,
    pub samples: u64,
    /// Set on the cell standing in for siblings too narrow to draw.
    pub folded: u64,
    /// Child indices from the laid-out root, so a cursor over the tree can be
    /// matched to the cell that draws it.
    pub path: Vec<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub cells: Vec<Placed>,
    /// Frames that did not fit anywhere and are drawn nowhere.
    pub hidden: u64,
}

/// Lay a tree out as an icicle: the root spans `width`, each child takes a
/// share of its parent proportional to its samples. Children narrower than one
/// cell are collected into a `…` cell where there is room for one, and counted
/// in `hidden` where there is not. A frame is never widened to make it visible.
pub fn layout(root: &Node, width: u16, depth_limit: u16, cursor: &[usize]) -> Layout {
    let mut out = Layout::default();
    if width == 0 || depth_limit == 0 || root.samples == 0 {
        return out;
    }
    let frame = Frame {
        limit: depth_limit,
        cursor,
    };
    place(root, 0, width, 0, &frame, &mut Vec::new(), &mut out);
    out
}

/// What stays the same all the way down a layout.
struct Frame<'a> {
    limit: u16,
    cursor: &'a [usize],
}

fn place(
    node: &Node,
    x: u16,
    width: u16,
    depth: u16,
    frame: &Frame<'_>,
    path: &mut Vec<usize>,
    out: &mut Layout,
) {
    out.cells.push(Placed {
        delta: None,
        depth,
        x,
        width,
        name: node.name.clone(),
        samples: node.samples,
        folded: 0,
        path: path.clone(),
    });
    if depth + 1 >= frame.limit || width == 0 || node.children.is_empty() {
        return;
    }
    let total = node.samples.max(1) as u128;
    let mut consumed = 0u16;
    let mut narrow = 0u64;
    let mut narrow_frames = 0u64;
    for (index, child) in node.children.iter().enumerate() {
        // Each width is measured against the parent on its own, never against
        // the running total. Accumulating would let rounding hand a cell to
        // whichever equal sibling happened to be last, and hide the rest.
        let child_width = ((child.samples as u128 * width as u128) / total) as u16;
        let mut child_width = child_width.min(width.saturating_sub(consumed));
        // A frame the reader has moved onto is always drawn, however thin its
        // share: losing sight of the cursor is worse than one column of
        // over-statement, and the panel reports the real share anyway.
        path.push(index);
        if child_width == 0 && frame.cursor.starts_with(path) && consumed < width {
            child_width = 1;
        }
        if child_width == 0 {
            path.pop();
            narrow += child.samples;
            narrow_frames += 1;
            continue;
        }
        place(
            child,
            x + consumed,
            child_width,
            depth + 1,
            frame,
            path,
            out,
        );
        path.pop();
        consumed += child_width;
    }
    let spare = width.saturating_sub(consumed);
    if narrow_frames > 0 {
        if spare > 0 {
            // The stand-in covers what the folded frames are worth, not the
            // whole gap: the rest of it is this frame's own time, and painting
            // that as callees would overstate them.
            let earned = ((narrow as u128 * width as u128) / total) as u16;
            let stand_in = earned.max(1).min(spare);
            out.cells.push(Placed {
                delta: None,
                depth: depth + 1,
                x: x + consumed,
                width: stand_in,
                // Say how many are in there, so a hidden frame is visibly
                // hidden rather than silently absent.
                name: format!("…{narrow_frames}"),
                samples: narrow,
                folded: narrow_frames,
                path: path.clone(),
            });
        } else {
            out.hidden += narrow_frames;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    #[default]
    Counts,
    Share,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub path: Vec<String>,
    pub before: u64,
    pub after: u64,
    pub delta: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comparison {
    pub mode: ComparisonMode,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
    pub baseline: Metadata,
    pub current: Metadata,
}
/// Prepared once per immutable profile pair and mode. Rendering performs
/// indexed delta lookup and formats only the visible list rows.
pub struct PreparedComparison {
    pub current: Profile,
    pub baseline: Profile,
    pub comparison: Comparison,
    pub union: Profile,
    pub order: Vec<usize>,
}
impl PreparedComparison {
    pub fn new(
        current: &Profile,
        baseline: &Profile,
        mode: ComparisonMode,
    ) -> Result<Self, String> {
        let comparison = current.compare(baseline, mode)?;
        let mut order: Vec<_> = (0..comparison.changes.len()).collect();
        order.sort_unstable_by(|&x, &y| {
            let x = &comparison.changes[x];
            let y = &comparison.changes[y];
            y.delta
                .abs()
                .total_cmp(&x.delta.abs())
                .then_with(|| x.path.cmp(&y.path))
        });
        Ok(Self {
            current: current.clone(),
            baseline: baseline.clone(),
            comparison,
            union: current.comparison_union(baseline),
            order,
        })
    }
}
impl Profile {
    /// Layout mass is the maximum baseline/current count at each terminal
    /// path. This preserves removed paths without claiming widths are deltas.
    pub fn comparison_union(&self, baseline: &Profile) -> Profile {
        fn union(before: Option<&Node>, after: Option<&Node>, name: String) -> Node {
            if let Some((b, a)) = before.zip(after).filter(|(b, a)| b == a) {
                let _ = b;
                let mut node = a.clone();
                node.name = name;
                node.sort();
                return node;
            }
            let b = before.map_or(&[][..], |n| n.children.as_slice());
            let a = after.map_or(&[][..], |n| n.children.as_slice());
            let mut children = if b.len() <= 1
                && a.len() <= 1
                && (b.is_empty() || a.is_empty() || b[0].name == a[0].name)
            {
                if let Some(child) = a.first().or(b.first()) {
                    vec![union(b.first(), a.first(), child.name.clone())]
                } else {
                    Vec::new()
                }
            } else {
                let mut pairs =
                    std::collections::BTreeMap::<&str, (Option<&Node>, Option<&Node>)>::new();
                if let Some(n) = before {
                    for c in &n.children {
                        pairs.entry(&c.name).or_default().0 = Some(c);
                    }
                }
                if let Some(n) = after {
                    for c in &n.children {
                        pairs.entry(&c.name).or_default().1 = Some(c);
                    }
                }
                let children: Vec<Node> = pairs
                    .into_iter()
                    .map(|(name, (before, after))| union(before, after, name.to_owned()))
                    .collect();
                children
            };
            children.sort_by(|x, y| y.samples.cmp(&x.samples).then_with(|| x.name.cmp(&y.name)));
            let own = before.map_or(0, Node::own).max(after.map_or(0, Node::own));
            Node {
                name,
                samples: own + children.iter().map(|c| c.samples).sum::<u64>(),
                children,
            }
        }
        let mut frames = self.frames.clone();
        if !frames.same_version(&baseline.frames) {
            for (key, value) in baseline.frames.iter() {
                if !frames.contains_key(key) {
                    frames.insert(key.clone(), value.clone());
                }
            }
        }
        Profile(std::sync::Arc::new(ProfileData {
            root: union(Some(&baseline.root), Some(&self.root), "comparison".into()).into(),
            frames,
            metadata: self.metadata.clone(),
            quality: self.quality.clone(),
            source: self.source.clone(),
            ..Default::default()
        }))
    }
    pub fn validate(&self) -> Result<(), String> {
        fn node(n: &Node) -> Result<(), String> {
            let mut total = 0u64;
            let mut names = std::collections::BTreeSet::new();
            for child in &n.children {
                if !names.insert(&child.name) {
                    return Err("Profile contains duplicate sibling identities".into());
                }
                total = total
                    .checked_add(child.samples)
                    .ok_or("Profile counts overflow")?;
                node(child)?;
            }
            if total > n.samples {
                return Err("Profile children exceed their parent's count".into());
            }
            Ok(())
        }
        node(&self.root)
    }
    pub fn compare(&self, baseline: &Profile, mode: ComparisonMode) -> Result<Comparison, String> {
        if self.metadata.kind == Kind::Unknown || baseline.metadata.kind == Kind::Unknown {
            return Err(
                "Profile provenance is unknown; select a capture with explicit kind and units"
                    .into(),
            );
        }
        if self.metadata.kind != baseline.metadata.kind
            || self.metadata.unit != baseline.metadata.unit
        {
            return Err("Cannot compare different capture kinds or weight units".into());
        }
        if self.is_empty() || baseline.is_empty() {
            return Err("Both profiles need usable samples".into());
        }
        fn visit(
            before: Option<&Node>,
            after: Option<&Node>,
            path: &mut Vec<String>,
            out: &mut Vec<Change>,
            totals: (u64, u64),
            mode: ComparisonMode,
        ) {
            let name = after.or(before).unwrap().name.clone();
            path.push(name);
            let b = before.map_or(0, |n| n.samples);
            let a = after.map_or(0, |n| n.samples);
            out.push(Change {
                path: path.clone(),
                before: b,
                after: a,
                delta: match mode {
                    ComparisonMode::Counts => (a as i128 - b as i128) as f64,
                    ComparisonMode::Share => {
                        100. * (a as f64 / totals.1 as f64 - b as f64 / totals.0 as f64)
                    }
                },
            });
            walk(before, after, path, out, totals, mode);
            path.pop();
        }
        fn walk(
            before: Option<&Node>,
            after: Option<&Node>,
            path: &mut Vec<String>,
            out: &mut Vec<Change>,
            totals: (u64, u64),
            mode: ComparisonMode,
        ) {
            let b = before.map_or(&[][..], |n| n.children.as_slice());
            let a = after.map_or(&[][..], |n| n.children.as_slice());
            // Most frames are chains. Avoid an allocated join table for them.
            if b.len() <= 1 && a.len() <= 1 {
                match (b.first(), a.first()) {
                    (None, None) => return,
                    (Some(b), Some(a)) if b.name == a.name => {
                        visit(Some(b), Some(a), path, out, totals, mode);
                        return;
                    }
                    (b, None) => {
                        visit(b, None, path, out, totals, mode);
                        return;
                    }
                    (None, a) => {
                        visit(None, a, path, out, totals, mode);
                        return;
                    }
                    _ => {}
                }
            }
            let mut pairs =
                std::collections::BTreeMap::<&str, (Option<&Node>, Option<&Node>)>::new();
            for c in b {
                pairs.entry(&c.name).or_default().0 = Some(c);
            }
            for c in a {
                pairs.entry(&c.name).or_default().1 = Some(c);
            }
            for (b, a) in pairs.into_values() {
                visit(b, a, path, out, totals, mode);
            }
        }
        let mut changes = Vec::new();
        walk(
            Some(&baseline.root),
            Some(&self.root),
            &mut Vec::new(),
            &mut changes,
            (baseline.root.samples, self.root.samples),
            mode,
        );
        let mut warnings = Vec::new();
        if self.metadata.scope != baseline.metadata.scope {
            warnings.push("Scopes differ".into());
        }
        if self.metadata.elapsed_seconds != baseline.metadata.elapsed_seconds {
            warnings.push("Capture durations differ".into());
        }
        if self.metadata.frequency_hz != baseline.metadata.frequency_hz {
            warnings.push("Requested frequencies differ".into());
        }
        Ok(Comparison {
            mode,
            changes,
            warnings,
            baseline: baseline.metadata.clone(),
            current: self.metadata.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frames(path: &str) -> Vec<String> {
        path.split(';').map(str::to_owned).collect()
    }
    fn profile(entries: &[(&str, u64)]) -> Profile {
        let mut p = Profile::new("test");
        for (path, weight) in entries {
            p.add(&frames(path), *weight);
        }
        p.sort();
        p
    }
    #[test]
    fn identical_stacks_accumulate_and_siblings_stay_separate() {
        let p = profile(&[("main;work", 3), ("main;work", 2), ("main;idle", 1)]);
        assert_eq!(p.root.samples, 6);
        let main = p.root.at(&["main".into()]).unwrap();
        assert_eq!(main.samples, 6);
        assert_eq!(main.children.len(), 2);
        // Sorted by samples, so the heavier sibling leads.
        assert_eq!(main.children[0].name, "work");
        assert_eq!(main.children[0].samples, 5);
        assert_eq!(main.children[1].samples, 1);
    }
    #[test]
    fn own_samples_exclude_children_and_folded_output_reports_them() {
        let p = profile(&[("main", 4), ("main;work", 6)]);
        let main = p.root.at(&["main".into()]).unwrap();
        assert_eq!(main.samples, 10);
        assert_eq!(main.own(), 4);
        assert_eq!(
            p.folded(),
            vec!["main 4".to_string(), "main;work 6".to_string()]
        );
    }
    #[test]
    fn an_empty_or_zero_weight_stack_is_not_an_observation() {
        let mut p = Profile::new("test");
        p.add(&[], 5);
        p.add(&frames("main"), 0);
        assert!(p.is_empty());
        assert_eq!(p.folded(), Vec::<String>::new());
    }
    #[test]
    fn shallow_stacks_and_unresolved_frames_are_counted_not_hidden() {
        let mut p = Profile::new("test");
        p.add(&frames("0x7ffff7a1"), 3);
        p.add(&frames("main;work;read"), 1);
        // Counted in samples, not in calls to add: three observations landed
        // on the truncated stack and one on the full one.
        assert_eq!(p.shallow, 3);
        assert_eq!(p.unresolved, 3);
        assert_eq!(p.shallow_share(), Some(75.));
        assert_eq!(Profile::new("test").shallow_share(), None);
    }
    #[test]
    fn children_divide_their_parent_and_never_overflow_it() {
        let p = profile(&[("a;b", 3), ("a;c", 1)]);
        let l = layout(&p.root, 40, 8, &[]);
        let root = &l.cells[0];
        assert_eq!((root.x, root.width), (0, 40));
        for cell in &l.cells {
            assert!(
                cell.x as u32 + cell.width as u32 <= 40,
                "{cell:?} leaves the area"
            );
        }
        let b = l.cells.iter().find(|c| c.name == "b").unwrap();
        let c = l.cells.iter().find(|c| c.name == "c").unwrap();
        assert_eq!(b.width, 30);
        assert_eq!(c.width, 10);
        assert_eq!(c.x, 30, "siblings abut without a gap");
    }
    #[test]
    fn a_frame_narrower_than_one_cell_is_folded_rather_than_widened() {
        // 200 samples across 10 cells: each cell is 20 samples, so the tail
        // children cannot be drawn honestly.
        let mut entries = vec![("a;wide", 190u64)];
        for path in ["a;t1", "a;t2", "a;t3"] {
            entries.push((path, 1));
        }
        let p = profile(&entries);
        let l = layout(&p.root, 10, 8, &[]);
        assert!(
            !l.cells.iter().any(|c| c.name.starts_with('t')),
            "a sub-cell frame must not be rounded up into visibility"
        );
        let stand_in = l.cells.iter().find(|c| c.name.starts_with('…'));
        match stand_in {
            Some(cell) => {
                assert_eq!(cell.folded, 3);
                assert_eq!(cell.samples, 3);
                // The count is on the cell, so a hidden frame is visibly
                // hidden rather than silently absent.
                assert_eq!(cell.name, "…3");
            }
            None => assert_eq!(l.hidden, 3),
        }
    }
    #[test]
    fn the_frame_under_the_cursor_is_drawn_however_thin_its_share() {
        let mut entries = vec![("a;wide", 190u64)];
        for path in ["a;t1", "a;t2", "a;t3"] {
            entries.push((path, 1));
        }
        let p = profile(&entries);
        // Without a cursor the tail folds away.
        let plain = layout(&p.root, 10, 8, &[]);
        assert!(!plain.cells.iter().any(|c| c.name == "t3"));
        // The reader has moved onto the last of them: root -> a -> t3.
        let a = p.root.at(&["a".into()]).unwrap();
        let index = a.children.iter().position(|c| c.name == "t3").unwrap();
        let l = layout(&p.root, 10, 8, &[0, index]);
        let drawn = l
            .cells
            .iter()
            .find(|c| c.name == "t3")
            .expect("cursor drawn");
        assert_eq!(drawn.width, 1, "one column, not a share it did not earn");
    }
    #[test]
    fn depth_limit_and_degenerate_areas_produce_nothing_unplaceable() {
        let p = profile(&[("a;b;c;d", 4)]);
        let l = layout(&p.root, 20, 2, &[]);
        assert_eq!(l.cells.iter().map(|c| c.depth).max(), Some(1));
        assert!(layout(&p.root, 0, 8, &[]).cells.is_empty());
        assert!(layout(&p.root, 20, 0, &[]).cells.is_empty());
        assert!(layout(&Node::default(), 20, 8, &[]).cells.is_empty());
    }
    #[test]
    fn zooming_selects_a_subtree_and_an_unknown_path_selects_nothing() {
        let p = profile(&[("a;b;c", 2)]);
        assert_eq!(p.root.at(&["a".into(), "b".into()]).unwrap().samples, 2);
        assert!(p.root.at(&["a".into(), "zz".into()]).is_none());
    }
}
