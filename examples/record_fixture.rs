//! Cross-platform recording fixture for release replay checks.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("recording path required")?;
    let mut recorder = kernwatch::recording::Recorder::create(path.into())?;
    let mut snapshot = kernwatch::model::demo();
    recorder.push(&snapshot)?;
    snapshot.telemetry.at_ms += 1000;
    recorder.push(&snapshot)?;
    Ok(())
}
