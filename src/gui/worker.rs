//! Runs the app's actions against recdr. Knows nothing of windows, so the windows part
//! can call it on a thread and feed the event it returns back into `state::step`.

use std::path::Path;

use super::state::{Action, Event};
use crate::control;
use crate::model::FrameSelection;

/// Runs one action against recdr and returns the event that reports the outcome.
/// `Quit` returns None: the windows part handles it.
pub fn perform(out: &Path, action: Action) -> Option<Event> {
    let outcome = match action {
        Action::Quit => return None,
        Action::ListMonitors => control::list_monitors().map(|list| Event::Monitors(list.monitors)),
        Action::ListWindows => control::list_windows().map(|list| Event::Windows(list.windows)),
        Action::Start(request) => request
            .into_options()
            .and_then(control::start)
            .map(|info| Event::Started(info.id)),
        Action::AskStatus(id) => control::status(out, Some(&id)).map(|list| {
            list.recordings.first().map_or_else(
                || Event::Failed(format!("no status for recording {id}")),
                |entry| Event::Status(entry.status),
            )
        }),
        Action::Stop(id) => control::stop(out, &id).map(|result| Event::Stopped {
            id: result.id,
            width: result.width,
            height: result.height,
            duration_ms: result.duration_ms,
        }),
        Action::Preview { id, t_ms } => {
            control::frames(out, &id, &FrameSelection::At(vec![t_ms]), None, None).map(|list| {
                list.frames.into_iter().next().map_or_else(
                    || Event::Failed(format!("recording {id} has no stored frame")),
                    |frame| Event::Picture {
                        t_ms,
                        path: frame.path,
                    },
                )
            })
        }
        Action::Export(request) => {
            return Some(
                request
                    .into_options()
                    .and_then(control::export)
                    .map_or_else(
                        |error| Event::ExportFailed(error.to_string()),
                        |result| Event::Exported(result.path),
                    ),
            );
        }
    };
    Some(outcome.unwrap_or_else(|error| Event::Failed(error.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExportRequest;

    fn empty_out(name: &str) -> std::path::PathBuf {
        let out = std::env::temp_dir().join(format!("recdr-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&out).unwrap();
        out
    }

    #[test]
    fn an_unknown_recording_is_a_failure_that_names_it() {
        let out = empty_out("worker");
        let actions = [
            Action::AskStatus("nope".into()),
            Action::Stop("nope".into()),
            Action::Preview {
                id: "nope".into(),
                t_ms: 0,
            },
            Action::Export(ExportRequest {
                id: "nope".into(),
                file: out.join("a.gif"),
                from: None,
                until: None,
                crop: None,
                out: Some(out.clone()),
            }),
        ];
        for action in actions {
            match perform(&out, action.clone()) {
                Some(Event::Failed(text) | Event::ExportFailed(text)) => {
                    assert!(text.contains("nope"), "{action:?}: {text}");
                }
                other => panic!("{action:?} gave {other:?}"),
            }
        }
        assert_eq!(perform(&out, Action::Quit), None);
    }
}
