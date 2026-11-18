mod common;

use common::{json, recdr};

#[test]
fn gui_is_refused_where_there_is_no_app() {
    let output = recdr().arg("gui").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        json(&output.stderr)["error"]["code"],
        "unsupported_platform"
    );
}

#[test]
fn help_lists_export_and_gui() {
    let output = recdr().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("export"), "{help}");
    assert!(help.contains("gui"), "{help}");
}
