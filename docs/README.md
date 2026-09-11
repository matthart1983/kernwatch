# kernwatch design documents

- [Graph redraw consistency](GRAPH_RENDERING.md): shared time, scale, color and frame-presentation rules.

- [Implemented tab fixes](TAB_FIXES.md): current changes, validation and remaining scope.

- [Current tab/table audit](TAB_GAP_AUDIT.md): 12 September source and live-data review covering all 13 screens.
- [Current completion plan](TAB_IMPLEMENTATION_PLAN.md): ordered work packages, dependencies and measurable acceptance gates.

- [Product specification](SPEC.md): the target application authored from the thirteen diagrams, with corrected measurement semantics and explicit screen, interaction and backend requirements.
- [Implementation plan](IMPLEMENTATION_PLAN.md): original gap analysis, reusable NetWatch components, architecture, ordered milestones and acceptance gates.
- [Original reference images](reference/renders-kernwatch): unchanged PNGs from `NetWatch btop redesign(2).zip`.

The Rust/Ratatui implementation, compiled probes, regression captures and validation reports are in this repository. See [validation evidence](VALIDATION.md) and the [requirement ledger](COMPLETION.md) for historical acceptance evidence; the current tab/table audit supersedes broad completion claims; validation is scoped to the tested kernels and recorded workloads.
