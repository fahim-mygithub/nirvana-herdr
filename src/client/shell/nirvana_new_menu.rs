//! Nirvana fork, plan 12 O4a: the sidebar's "new" opens a menu of the agents a space can start with.
//!
//! The owner, 2026-10-17 (plan 12 L7): "Sometimes I want a Orchistrator style model that tries to utilize both Claude
//! and ANtigravity and sometimes I just want Claude Code or agy traditional to tackle the project." So "new" offers
//! Nirvana, Claude Code and Antigravity; only Nirvana gets the plugin (the server side, `app/nirvana_new.rs`). A
//! fourth entry, Shell, is what "new" did before this menu, so nothing the owner had is lost.
//!
//! Choosing Nirvana also makes this client's Terminal window a Nirvana window (its own button and the folder's
//! letter) and shows the "Nirvana · <folder>" toast, unless the verb's window already is one.
//!
//! Plan 13 §6.1: "Fresh Nirvana Claude", below Shell, is Nirvana Claude that never resumes the folder's last session;
//! it makes the window Nirvana's too. Below Shell so the entries above keep their places.

use super::*;
use crate::nirvana_launch::NewKind;

pub(super) fn items() -> Vec<ClientContextMenuItem> {
    let mut items: Vec<_> = NewKind::MENU
        .iter()
        .map(|kind| ClientContextMenuItem {
            label: kind.label(),
            action: ClientContextMenuAction::NewSpace(*kind),
        })
        .collect();
    items.push(ClientContextMenuItem {
        label: "Shell",
        action: ClientContextMenuAction::NewShellSpace,
    });
    items.extend(
        NewKind::AFTER_SHELL
            .iter()
            .map(|kind| ClientContextMenuItem {
                label: kind.label(),
                action: ClientContextMenuAction::NewSpace(*kind),
            }),
    );
    items
}

impl ClientShellState {
    pub(super) fn open_new_space_menu(&mut self, x: u16, y: u16) {
        let workspace_id = self.workspace_action_id();
        let cwd = self.snapshot.as_deref().and_then(|snapshot| {
            let workspace_id = workspace_id.as_deref()?;
            snapshot
                .workspaces
                .iter()
                .find(|workspace| workspace.workspace_id == workspace_id)
                .map(|workspace| workspace.new_workspace_cwd.clone())
        });
        self.overlay = Some(ClientShellOverlay::ContextMenu(ClientContextMenuOverlay {
            target: ClientContextMenuTarget::NewSpace { workspace_id, cwd },
            x,
            y,
            highlighted: 0,
        }));
    }

    pub(super) fn activate_new_space_action(
        &mut self,
        workspace_id: Option<String>,
        cwd: Option<String>,
        action: ClientContextMenuAction,
        outcome: &mut ClientShellInput,
    ) {
        match action {
            ClientContextMenuAction::NewSpace(kind) => {
                self.push_endpoint_method(
                    crate::api::schema::Method::CommandInvoke(
                        crate::api::schema::CommandInvokeParams {
                            command_id: kind.command_id(),
                            workspace_id,
                            tab_id: None,
                            pane_id: None,
                            selection: None,
                        },
                    ),
                    outcome,
                );
                // Plan 13: the fresh entry is Nirvana Claude too, so it makes the window Nirvana's.
                let kind = if kind == NewKind::NirvanaFresh {
                    NewKind::Nirvana
                } else {
                    kind
                };
                if kind == NewKind::Nirvana {
                    if let Some(cwd) = cwd {
                        nirvana_window(std::path::PathBuf::from(cwd));
                    }
                }
            }
            ClientContextMenuAction::NewShellSpace => self.record_binding(
                crate::input::KeybindMatch::Action(crate::input::KeybindAction::NewWorkspace),
                outcome,
            ),
            _ => {}
        }
    }
}

/// Claims the window and shows the toast off the input thread: the toast waits up to 2 s on Windows' service. A
/// test records the folder instead, so no test ever touches the Terminal window it runs in.
fn nirvana_window(folder: std::path::PathBuf) {
    #[cfg(not(test))]
    {
        let _ = std::thread::Builder::new()
            .name("nirvana-window".into())
            .spawn(move || {
                crate::nirvana_taskbar::claim_menu_window(&folder);
                crate::nirvana_taskbar::toast_open(&folder);
            });
    }
    #[cfg(test)]
    CLAIMED_IN_TEST.with(|claimed| claimed.borrow_mut().push(folder));
}

#[cfg(test)]
thread_local! {
    pub(super) static CLAIMED_IN_TEST: std::cell::RefCell<Vec<std::path::PathBuf>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
