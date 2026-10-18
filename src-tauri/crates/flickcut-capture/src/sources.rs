// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Taha Arefi
//! Capture source listing (`list_sources`).
//!
//! Returns screens and windows in the exact shape the IPC contract expects:
//! `{ screens: [{ id, name }], windows: [{ id, title }] }`.
//!
//! Source ids are stable strings (`"screen:<n>"` / `"window:<n>"`) that the
//! platform backend resolves back to a native target. On Linux the portal
//! owns enumeration, so a synthetic `"screen:portal"` source is offered and
//! the OS picker appears at capture start.

use crate::backends::create_backend;

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

/// Enumerate capturable screens and windows. Never panics; on failure (e.g.
/// no permission, no display server) the lists are empty.
pub fn list_sources() -> SourceList {
    let mut backend = create_backend();
    match backend.list_sources() {
        Ok(list) => {
            // Validate id shapes before handing them out.
            let mut clean = SourceList::default();
            for s in list.screens.into_iter().chain(list.windows.into_iter()) {
                if s.id.starts_with("screen:") || s.id.starts_with("window:") {
                    if s.id.starts_with("screen:") {
                        clean.screens.push(s);
                    } else {
                        clean.windows.push(s);
                    }
                }
            }
            // The portal entry must stay in `screens` even though its id is
            // special-cased above (it starts with "screen:").
            clean
        }
        Err(_) => SourceList::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_does_not_panic() {
        let list = list_sources();
        for s in list.screens.iter().chain(list.windows.iter()) {
            assert!(s.id.starts_with("screen:") || s.id.starts_with("window:"));
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn portal_source_listed() {
        // On Linux the portal entry must always be offered.
        let list = list_sources();
        assert!(list.screens.iter().any(|s| s.id == "screen:portal"));
    }
}
