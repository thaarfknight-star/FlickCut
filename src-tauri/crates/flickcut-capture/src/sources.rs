//! Capture source listing (`list_sources`).
//!
//! Returns screens and windows in the exact shape the IPC contract expects:
//! `{ screens: [{ id, name }], windows: [{ id, title }] }`.
//!
//! Source ids are stable strings (`"screen:<n>"` / `"window:<n>"`) that
//! [`crate::recorder::Session::start`] resolves back to the scap target.

/// A single capture source reference (IPC shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    /// Opaque id, e.g. `"screen:0"` or `"window:2"`.
    pub id: String,
    /// Display name (screens) or window title (windows).
    pub name: String,
}

/// Which kind of source a [`SourceRef`] points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Screen,
    Window,
}

/// The `list_sources()` result.
#[derive(Debug, Clone, Default)]
pub struct SourceList {
    pub screens: Vec<SourceRef>,
    pub windows: Vec<SourceRef>,
}

/// Enumerate capturable screens and windows. Never panics; on a headless
/// host the lists may simply be empty.
///
/// Note (Linux): scap leaves target selection to the XDG Desktop Portal
/// picker, so `get_all_targets()` is always empty there. We therefore expose
/// a synthetic `"screen:portal"` source; starting it pops the OS source
/// picker at capture time.
pub fn list_sources() -> SourceList {
    let mut out = SourceList::default();
    let (mut si, mut wi) = (0u32, 0u32);
    for target in scap::get_all_targets() {
        match target {
            scap::Target::Display(d) => {
                out.screens.push(SourceRef {
                    id: format!("screen:{si}"),
                    name: if d.title.is_empty() {
                        format!("Screen {si}")
                    } else {
                        d.title
                    },
                });
                si += 1;
            }
            scap::Target::Window(w) => {
                out.windows.push(SourceRef {
                    id: format!("window:{wi}"),
                    name: if w.title.is_empty() {
                        format!("Window {wi}")
                    } else {
                        w.title
                    },
                });
                wi += 1;
            }
        }
    }
    #[cfg(target_os = "linux")]
    if out.screens.is_empty() {
        // Portal picker entry (see module docs).
        out.screens.push(SourceRef {
            id: "screen:portal".into(),
            name: "Choose source… (system picker)".into(),
        });
    }
    out
}

/// What a source id resolved to.
pub(crate) enum ResolvedTarget {
    /// A concrete scap target.
    Scap(scap::Target),
    /// Linux portal picker: build the capturer with `target: None` and the
    /// OS shows its source picker when capture starts.
    #[cfg(target_os = "linux")]
    PortalPicker,
}

/// Resolve a source id. Returns `None` for malformed ids or when the target
/// no longer exists.
pub(crate) fn resolve_target(id: &str) -> Option<ResolvedTarget> {
    #[cfg(target_os = "linux")]
    if id == "screen:portal" {
        return Some(ResolvedTarget::PortalPicker);
    }
    let (kind, idx) = id.split_once(':')?;
    let idx: usize = idx.parse().ok()?;
    let mut seen = 0usize;
    for target in scap::get_all_targets() {
        let matches = matches!(
            (&target, kind),
            (scap::Target::Display(_), "screen") | (scap::Target::Window(_), "window")
        );
        if matches {
            if seen == idx {
                return Some(ResolvedTarget::Scap(target));
            }
            seen += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_does_not_panic() {
        let list = list_sources();
        // Ids must round-trip through the expected format.
        for s in list.screens.iter().chain(list.windows.iter()) {
            assert!(s.id.starts_with("screen:") || s.id.starts_with("window:"));
        }
    }

    #[test]
    fn resolve_rejects_garbage() {
        assert!(resolve_target("").is_none());
        assert!(resolve_target("bogus").is_none());
        assert!(resolve_target("screen:").is_none());
        assert!(resolve_target("screen:abc").is_none());
        assert!(resolve_target("screen:999999").is_none());
        assert!(resolve_target("window:999999").is_none());
    }

    #[test]
    fn listed_ids_resolve() {
        // Every listed source id must resolve back to a target of the
        // right kind (on hosts with no display the lists are empty and
        // this is a no-op).
        for s in list_sources()
            .screens
            .iter()
            .chain(list_sources().windows.iter())
        {
            let t = resolve_target(&s.id)
                .unwrap_or_else(|| panic!("id {} did not resolve", s.id));
            match s.id.split_once(':').unwrap().0 {
                "screen" => match t {
                    ResolvedTarget::Scap(scap::Target::Display(_)) => {}
                    #[cfg(target_os = "linux")]
                    ResolvedTarget::PortalPicker => {} // "screen:portal"
                    _ => panic!("id {} resolved to wrong kind", s.id),
                },
                "window" => assert!(matches!(
                    t,
                    ResolvedTarget::Scap(scap::Target::Window(_))
                )),
                _ => unreachable!(),
            }
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn portal_picker_source_exists() {
        // On Linux the portal entry must always be offered.
        let list = list_sources();
        assert!(list.screens.iter().any(|s| s.id == "screen:portal"));
        assert!(matches!(
            resolve_target("screen:portal"),
            Some(ResolvedTarget::PortalPicker)
        ));
    }
}
