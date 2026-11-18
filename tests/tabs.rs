//! `recdr tabs` and the tab flags, without a browser. Browser tests are in browser.rs.

mod common;

#[cfg(not(windows))]
mod unsupported {
    use super::common::{json, recdr};

    #[test]
    fn desktop_commands_are_unsupported_off_windows() {
        for args in [
            &["windows"][..],
            &["monitors"],
            &["record", "--monitor", "0", "--duration", "1s"],
        ] {
            let output = recdr().args(args).output().unwrap();
            assert_eq!(output.status.code(), Some(1), "{args:?}");
            assert_eq!(
                json(&output.stderr)["error"]["code"],
                "unsupported_platform",
                "{args:?}"
            );
        }
    }
}
