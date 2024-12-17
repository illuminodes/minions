use crate::widgets::toastify::ToastifyOptions;
use std::collections::HashMap;

use nostro2::{
    notes::NostrNote,
    relays::{NoteEvent, RelayEvent, SubscribeEvent},
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use web_sys::wasm_bindgen::JsValue;
use yew::platform::spawn_local;
use yew::{prelude::*, props};

use super::nostr_relay::UserRelay;

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct RelayContextProps {
    pub children: Children,
    pub relays: Vec<UserRelay>,
}

pub enum RelayAction {
    Event(RelayEvent),
    UniqueNote(NostrNote),
    SendNote(NostrNote),
    Subscribe(SubscribeEvent),
    Unsubscribe(String),
    Close,
}

#[derive(Properties, Clone, PartialEq)]
pub struct NostrProps {
    pub relay_events: Vec<RelayEvent>,
    pub unique_notes: Vec<NostrNote>,
    pub send_note: Callback<NostrNote>,
    pub subscribe: Callback<SubscribeEvent>,
    pub unsubscribe: Callback<String>,
    pub close: Callback<()>,
}
pub struct RelayProvider {
    relay_events: Vec<RelayEvent>,
    unique_notes: Vec<NostrNote>,
    sender_channel: UnboundedSender<NostrNote>,
    filter_channel: UnboundedSender<SubscribeEvent>,
    unsubscribe_channel: UnboundedSender<String>,
    close_channel: UnboundedSender<()>,
    send_note_callback: Callback<NostrNote>,
    subscribe_callback: Callback<SubscribeEvent>,
    unsubscribe_callback: Callback<String>,
    close_callback: Callback<()>,
    children: Children,
}

impl Component for RelayProvider {
    type Message = RelayAction;
    type Properties = RelayContextProps;

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let props = self.build_props();
        html! {
            <>
                <ContextProvider<NostrProps> context={props}>
                    {self.children.clone()}
                </ContextProvider<NostrProps>>
            </>
        }
    }

    fn create(ctx: &Context<Self>) -> Self {
        let relays = ctx.props().relays.clone();
        let (sender_channel, filter_channel, unsubscribe_channel, close_channel) =
            Self::read_relays(
                ctx.link().callback(RelayAction::Event),
                ctx.link().callback(RelayAction::UniqueNote),
                relays,
            );
        let send_note_callback = ctx.link().callback(RelayAction::SendNote);
        let close_callback = ctx.link().callback(move |_| RelayAction::Close);
        let subscribe_callback = ctx.link().callback(RelayAction::Subscribe);
        let unsubscribe_callback = ctx.link().callback(RelayAction::Unsubscribe);
        let children = ctx.props().children.clone();
        let relay_events = Vec::new();
        let unique_notes = Vec::new();

        Self {
            relay_events,
            unique_notes,
            sender_channel,
            close_channel,
            filter_channel,
            unsubscribe_channel,
            send_note_callback,
            close_callback,
            subscribe_callback,
            unsubscribe_callback,
            children,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            RelayAction::SendNote(note) => match self.send_nostr_note(note) {
                Ok(_) => true,
                Err(e) => {
                    gloo::console::error!("Error sending note: {:?}", e);
                    false
                }
            },
            RelayAction::Subscribe(filter) => match self.subscribe(filter) {
                Ok(_) => true,
                Err(e) => {
                    gloo::console::error!("Error subscribing: {:?}", e);
                    false
                }
            },
            RelayAction::Close => {
                match self.close_ws() {
                    Ok(_) => (),
                    Err(e) => gloo::console::error!("Error closing websocket: {:?}", e),
                }
                false
            }
            RelayAction::Event(event) => {
                self.add_event(event);
                true
            }
            RelayAction::UniqueNote(note) => {
                self.add_unique_note(note);
                true
            }
            RelayAction::Unsubscribe(filter) => match self.unsubscribe(filter) {
                Ok(_) => true,
                Err(e) => {
                    gloo::console::error!("Error unsubscribing: {:?}", e);
                    false
                }
            },
        }
    }

    fn destroy(&mut self, _ctx: &Context<Self>) {
        match self.close_ws() {
            Ok(_) => (),
            Err(e) => gloo::console::error!("Error closing websocket: {:?}", e),
        }
    }
}

impl RelayProvider {
    fn read_relays(
        event_cb: Callback<RelayEvent>,
        note_cb: Callback<NostrNote>,
        relays: Vec<UserRelay>,
    ) -> (
        UnboundedSender<NostrNote>,
        UnboundedSender<SubscribeEvent>,
        UnboundedSender<String>,
        UnboundedSender<()>,
    ) {
        let (send_note_tx, mut send_note_rx) = unbounded_channel::<NostrNote>();
        let (filter_tx, mut filter_rx) = unbounded_channel::<SubscribeEvent>();
        let (unsubscribe_tx, mut unsubscribe_rx) = unbounded_channel::<String>();
        let (close_tx, mut close_rx) = unbounded_channel::<()>();

        spawn_local(async move {
            // Show initial connection attempt

            let mut relay_pool = match nostro2::relays::NostrRelayPool::new(
                relays.iter().map(|relay| relay.url.clone()).collect(),
            )
            .await
            {
                Ok(pool) => pool,
                Err(e) => {
                    ToastifyOptions::new_relay_error(&format!(
                        "Failed to create relay pool: {}",
                        e
                    ))
                    .show();
                    return;
                }
            };

            loop {
                tokio::select! {
                    Some(note) = relay_pool.listener.recv() => {
                        match note.1 {
                            RelayEvent::NewNote( NoteEvent(_, _, note)) => {
                                note_cb.emit(note);
                            }
                            event => {
                                event_cb.emit(event);
                            }
                        }
                    }
                    Some(note) = send_note_rx.recv() => {
                        if let Err(e) = relay_pool.writer.broadcast_note(note).await {
                            ToastifyOptions::new_relay_error(&format!("Error broadcasting note: {}", e))
                                .show();
                        }
                    }
                    Some(filter) = filter_rx.recv() => {
                        if let Err(e) = relay_pool.writer.subscribe(filter).await {
                            ToastifyOptions::new_relay_error(&format!("Error subscribing: {}", e))
                                .show();
                        }
                    }
                    Some(filter_id) = unsubscribe_rx.recv() => {
                        if let Err(e) = relay_pool.writer.cancel_subscription(filter_id).await {
                            ToastifyOptions::new_relay_error(&format!("Error unsubscribing: {}", e))
                                .show();
                        }
                    }
                    _ = close_rx.recv() => {
                        ToastifyOptions::new_relay_disconnected("Disconnecting from relay pool").show();
                        let _ = relay_pool.close();
                        break;
                    }
                    else => {
                        let _ = relay_pool.close();
                        break;
                    }
                }
            }
        });

        (send_note_tx, filter_tx, unsubscribe_tx, close_tx)
    }

    pub fn build_props(&self) -> NostrProps {
        let _unique_notes = self
            .relay_events
            .iter()
            .filter_map(|event| match event {
                RelayEvent::NewNote(NoteEvent(_, _, note)) => Some(note.clone()),
                _ => None,
            })
            .fold(HashMap::new(), |mut acc, note| {
                acc.insert(note.id.clone().unwrap(), note);
                acc
            });
        props!(NostrProps {
            relay_events: self.relay_events.clone(),
            unique_notes: self.unique_notes.clone(),
            send_note: self.send_note_callback.clone(),
            subscribe: self.subscribe_callback.clone(),
            unsubscribe: self.unsubscribe_callback.clone(),
            close: self.close_callback.clone(),
        })
    }

    fn send_nostr_note(&self, signed_note: NostrNote) -> Result<(), JsValue> {
        let _ = self
            .sender_channel
            .send(signed_note)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    fn subscribe(&self, filter: SubscribeEvent) -> Result<(), JsValue> {
        let _ = self
            .filter_channel
            .send(filter)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    fn unsubscribe(&self, filter: String) -> Result<(), JsValue> {
        let _ = self
            .unsubscribe_channel
            .send(filter)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }

    fn add_event(&mut self, event: RelayEvent) {
        self.relay_events.push(event);
    }
    fn add_unique_note(&mut self, note: NostrNote) {
        self.unique_notes.push(note);
    }

    fn close_ws(&self) -> Result<(), JsValue> {
        let _ = self
            .close_channel
            .send(())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(())
    }
}
