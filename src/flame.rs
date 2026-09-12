//! Stack aggregation and icicle layout.
//!
//! A profile counts identical stacks; it never weights a stack it did not
//! observe. Frames that could not be resolved keep their address rather than
//! borrowing a neighbouring symbol's name, and stacks that end too shallow to
//! be a real call path are counted so the panel can say how many there were.
use serde::{Deserialize, Serialize};

/// A stack shallower than this almost always means the frame pointer chain
/// ended early, not that the program really is one call deep.
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
    fn fold(&self, prefix: &str, out: &mut Vec<String>) {
        let path = if prefix.is_empty() {
            self.name.clone()
        } else {
            format!("{prefix};{}", self.name)
        };
        let own = self.own();
        if own > 0 {
            out.push(format!("{path} {own}"));
        }
        for child in &self.children {
            child.fold(&path, out);
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub root: Node,
    /// What produced these stacks, named the way the capture named it.
    pub source: String,
    /// Samples whose stack ended at or below `SHALLOW` frames.
    pub shallow: u64,
    /// Samples whose stack still carries at least one unresolved address.
    pub unresolved: u64,
}
impl Profile {
    pub fn new(source: &str) -> Self {
        Self {
            root: Node {
                name: "all".into(),
                ..Default::default()
            },
            source: source.to_owned(),
            ..Default::default()
        }
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
        let mut node = &mut self.root;
        for frame in frames {
            node = node.child(frame);
            node.samples += weight;
        }
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
            child.fold("", &mut out);
        }
        out
    }
    /// Share of stacks too shallow to be a believable call path, if any were
    /// collected at all. Reported rather than corrected: without frame
    /// pointers a truncated stack is indistinguishable from a short one.
    pub fn shallow_share(&self) -> Option<f64> {
        (self.root.samples > 0).then(|| self.shallow as f64 / self.root.samples as f64 * 100.)
    }
}

/// One frame's place in the icicle.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub depth: u16,
    pub x: u16,
    pub width: u16,
    pub name: String,
    pub samples: u64,
    /// Set on the cell standing in for siblings too narrow to draw.
    pub folded: u64,
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
pub fn layout(root: &Node, width: u16, depth_limit: u16) -> Layout {
    let mut out = Layout::default();
    if width == 0 || depth_limit == 0 || root.samples == 0 {
        return out;
    }
    place(root, 0, width, 0, depth_limit, &mut out);
    out
}

fn place(node: &Node, x: u16, width: u16, depth: u16, limit: u16, out: &mut Layout) {
    out.cells.push(Placed {
        depth,
        x,
        width,
        name: node.name.clone(),
        samples: node.samples,
        folded: 0,
    });
    if depth + 1 >= limit || width == 0 || node.children.is_empty() {
        return;
    }
    let total = node.samples.max(1) as u128;
    let mut consumed = 0u16;
    let mut narrow = 0u64;
    let mut narrow_frames = 0u64;
    for child in &node.children {
        // Each width is measured against the parent on its own, never against
        // the running total. Accumulating would let rounding hand a cell to
        // whichever equal sibling happened to be last, and hide the rest.
        let child_width = ((child.samples as u128 * width as u128) / total) as u16;
        let child_width = child_width.min(width.saturating_sub(consumed));
        if child_width == 0 {
            narrow += child.samples;
            narrow_frames += 1;
            continue;
        }
        place(child, x + consumed, child_width, depth + 1, limit, out);
        consumed += child_width;
    }
    // The gap left by this frame's own samples is where a stand-in can go.
    let spare = width.saturating_sub(consumed);
    if narrow_frames > 0 {
        if spare > 0 {
            out.cells.push(Placed {
                depth: depth + 1,
                x: x + consumed,
                width: spare,
                name: "…".into(),
                samples: narrow,
                folded: narrow_frames,
            });
        } else {
            out.hidden += narrow_frames;
        }
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
        let l = layout(&p.root, 40, 8);
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
        let l = layout(&p.root, 10, 8);
        assert!(
            !l.cells.iter().any(|c| c.name.starts_with('t')),
            "a sub-cell frame must not be rounded up into visibility"
        );
        let stand_in = l.cells.iter().find(|c| c.name == "…");
        match stand_in {
            Some(cell) => {
                assert_eq!(cell.folded, 3);
                assert_eq!(cell.samples, 3);
            }
            None => assert_eq!(l.hidden, 3),
        }
    }
    #[test]
    fn depth_limit_and_degenerate_areas_produce_nothing_unplaceable() {
        let p = profile(&[("a;b;c;d", 4)]);
        let l = layout(&p.root, 20, 2);
        assert_eq!(l.cells.iter().map(|c| c.depth).max(), Some(1));
        assert!(layout(&p.root, 0, 8).cells.is_empty());
        assert!(layout(&p.root, 20, 0).cells.is_empty());
        assert!(layout(&Node::default(), 20, 8).cells.is_empty());
    }
    #[test]
    fn zooming_selects_a_subtree_and_an_unknown_path_selects_nothing() {
        let p = profile(&[("a;b;c", 2)]);
        assert_eq!(p.root.at(&["a".into(), "b".into()]).unwrap().samples, 2);
        assert!(p.root.at(&["a".into(), "zz".into()]).is_none());
    }
}
