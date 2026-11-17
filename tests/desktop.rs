//! Checks that need a real desktop session. Run with `cargo test --test desktop -- --ignored`.

mod common;

use common::{json, recdr};

#[test]
#[ignore]
fn monitors_lists_index_zero_first() {
    let output = recdr().arg("monitors").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let monitors = json(&output.stdout)["monitors"].clone();
    assert!(!monitors.as_array().unwrap().is_empty());
    assert_eq!(monitors[0]["index"], 0);
}

#[test]
#[ignore]
fn windows_prints_a_list() {
    let output = recdr().arg("windows").output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(json(&output.stdout)["windows"].is_array());
}
