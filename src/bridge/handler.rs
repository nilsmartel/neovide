use std::sync::Arc;
use std::sync::mpsc::Sender;

use async_trait::async_trait;
use log::trace;
use nvim_rs::{compat::tokio::Compat, Handler, Neovim};
use rmpv::Value;
use tokio::sync::mpsc::UnboundedSender;
use tokio::process::ChildStdin;
use tokio::task;
use parking_lot::Mutex;

use super::events::{parse_redraw_event, RedrawEvent};
use super::ui_commands::UiCommand;
use crate::settings::SETTINGS;
use crate::error_handling::ResultPanicExplanation;

#[derive(Clone)]
pub struct NeovimHandler {
    ui_command_sender: Arc<Mutex<UnboundedSender<UiCommand>>>,
    redraw_event_sender: Arc<Mutex<Sender<RedrawEvent>>>
}

impl NeovimHandler {
    pub fn new(ui_command_sender: UnboundedSender<UiCommand>, redraw_event_sender: Sender<RedrawEvent>) -> NeovimHandler {
        NeovimHandler {
            ui_command_sender: Arc::new(Mutex::new(ui_command_sender)),
            redraw_event_sender: Arc::new(Mutex::new(redraw_event_sender))
        }
    }
}

#[async_trait]
impl Handler for NeovimHandler {
    type Writer = Compat<ChildStdin>;

    async fn handle_notify(
        &self,
        event_name: String,
        arguments: Vec<Value>,
        _neovim: Neovim<Compat<ChildStdin>>,
    ) {
        trace!("Neovim notification: {:?}", &event_name);

        let ui_command_sender = self.ui_command_sender.clone();
        let redraw_event_sender = self.redraw_event_sender.clone();
        task::spawn_blocking(move || match event_name.as_ref() {
            "redraw" => {
                for events in arguments {
                    let parsed_events = parse_redraw_event(events)
                        .unwrap_or_explained_panic("Could not parse event from neovim");

                    for parsed_event in parsed_events {
                        let redraw_event_sender = redraw_event_sender.lock();
                        redraw_event_sender.send(parsed_event).ok();
                    }
                }
            }
            "setting_changed" => {
                SETTINGS.handle_changed_notification(arguments);
            }
            #[cfg(windows)]
            "neovide.register_right_click" => {
                let ui_command_sender = ui_command_sender.lock();
                ui_command_sender.send(UiCommand::RegisterRightClick).ok();
            }
            #[cfg(windows)]
            "neovide.unregister_right_click" => {
                let ui_command_sender = ui_command_sender.lock();
                ui_command_sender.send(UiCommand::UnregisterRightClick).ok();
            }
            _ => {}
        })
        .await
        .ok();
    }
}
