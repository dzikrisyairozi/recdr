mod common;

use common::{json, recdr};

#[test]
fn unknown_flag_is_a_usage_error_on_stderr() {
    let output = recdr().arg("--bogus").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(json(&output.stderr)["error"]["code"], "usage");
}
