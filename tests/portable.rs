use kernwatch::{model, recording};
#[test]
fn recording_and_report_export_work_on_the_host_platform() {
    let snapshot = model::demo();
    let path = std::env::temp_dir().join(format!("kernwatch-portable-{}.kwr", recording::stamp()));
    {
        let mut writer = recording::Recorder::create(path.clone()).unwrap();
        writer.push(&snapshot).unwrap();
        assert!(recording::Recorder::create(path.clone()).is_err());
    }
    let frames = recording::read(&path).unwrap();
    assert_eq!(frames.len(), 1);
    assert!(frames[0].demo);
    std::fs::remove_file(path).unwrap();
    let report = recording::export(&snapshot).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["files"].as_array().unwrap().len(), 9);
    for entry in manifest["files"].as_array().unwrap() {
        assert!(report.join(entry["file"].as_str().unwrap()).is_file());
    }
    // The folded stacks travel with the report so a capture can be read by
    // flamegraph.pl or speedscope without the terminal.
    let folded = std::fs::read_to_string(report.join("stacks.folded")).unwrap();
    // Every line is a path and a count. A one-frame path is legitimate: it is
    // what a stack whose frame pointer chain ended immediately folds to.
    assert!(!folded.is_empty(), "the demo profile is not empty");
    for line in folded.lines() {
        let (path, count) = line.rsplit_once(' ').expect("a path and a count");
        assert!(!path.is_empty());
        assert!(
            count.parse::<u64>().is_ok(),
            "{line:?} does not end in a count"
        );
    }
    assert!(
        folded.lines().any(|l| l.contains(';')),
        "the demo profile contains multi-frame stacks"
    );
    std::fs::remove_dir_all(report).unwrap();
}
