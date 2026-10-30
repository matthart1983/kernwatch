use std::process::Command;
#[test]
fn version_flags_print_the_crate_version_and_exit() {
    for flag in ["--version", "-V"] {
        let output = Command::new(env!("CARGO_BIN_EXE_kernwatch"))
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("kernwatch {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}
