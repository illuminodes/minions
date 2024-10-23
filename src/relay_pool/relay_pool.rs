use crate::widgets::toastify::ToastifyOptions;
use std::{collections::HashSet, sync::Arc};

use async_channel::{unbounded, Sender};
use nostro2::{
    notes::SignedNote,
    relays::{NostrRelay, NostrSubscription, RelayEvents},
};
use yew::platform::spawn_local;
use yew::{prelude::*, props};

pub enum RelayAction {
    Event(RelayEvents),
    SendNote(SignedNote),
    Subscribe(NostrSubscription),
    Unsubscribe(String),
    Close,
}

#[derive(Properties, Clone, PartialEq)]
pub struct NostrProps {
    pub relay_events: Vec<RelayEvents>,
    pub notes: Vec<SignedNote>,
    pub send_note: Callback<SignedNote>,
    pub subscribe: Callback<NostrSubscription>,
    pub unsubscribe: Callback<String>,
    pub close: Callback<()>,
}

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct RelayContextProps {
    pub children: Children,
    pub user_relays: Vec<super::nostr_relay::UserRelay>,
}

pub struct RelayPool {
    relay_events: Vec<RelayEvents>,
    new_notes: Vec<SignedNote>,
    unique_ids: HashSet<String>,
    sender_channel: Vec<Sender<SignedNote>>,
    filter_channel: Vec<Sender<NostrSubscription>>,
    unsubscribe_channel: Vec<Sender<String>>,
    close_channel: Vec<Sender<()>>,
    send_note_callback: Callback<SignedNote>,
    subscribe_callback: Callback<NostrSubscription>,
    unsubscribe_callback: Callback<String>,
    close_callback: Callback<()>,
    children: Children,
}

impl Component for RelayPool {
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
        let relays = ctx.props().user_relays.clone();
        let (sender_channel, filter_channel, unsubscribe_channel, close_channel) =
            Self::read_relay(ctx.link().callback(RelayAction::Event), relays);
        let send_note_callback = ctx.link().callback(RelayAction::SendNote);
        let close_callback = ctx.link().callback(move |_| RelayAction::Close);
        let subscribe_callback = ctx.link().callback(RelayAction::Subscribe);
        let unsubscribe_callback = ctx.link().callback(RelayAction::Unsubscribe);
        let children = ctx.props().children.clone();
        let relay_events = Vec::new();
        let new_notes = Vec::new();
        let unique_ids = HashSet::new();

        Self {
            relay_events,
            new_notes,
            unique_ids,
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
            RelayAction::SendNote(note) => {
                self.send_nostr_note(note);
                true
            }
            RelayAction::Subscribe(filter) => {
                self.subscribe(filter);
                true
            }
            RelayAction::Close => {
                self.close_ws();
                true
            }
            RelayAction::Event(event) => {
                if let RelayEvents::EVENT(_, ref note) = event {
                    if !self.unique_ids.contains(note.get_id()) {
                        self.unique_ids.insert(note.get_id().to_string());
                        self.new_notes.push(note.clone());
                        // Add notification for new event
                        ToastifyOptions::new_event_received("note").show();
                    }
                }
                self.add_event(event);
                true
            }
            RelayAction::Unsubscribe(filter) => {
                self.unsubscribe(filter);
                true
            }
        }
    }
    fn destroy(&mut self, _ctx: &Context<Self>) {
        self.close_ws();
    }
}

impl RelayPool {
    fn read_relay(
        note_cb: Callback<RelayEvents>,
        relays: Vec<super::nostr_relay::UserRelay>,
    ) -> (
        Vec<Sender<SignedNote>>,
        Vec<Sender<NostrSubscription>>,
        Vec<Sender<String>>,
        Vec<Sender<()>>,
    ) {
        let mut send_note_channels = vec![];
        let mut filter_channels = vec![];
        let mut unsubscribe_channels = vec![];
        let mut close_channels = vec![];

        for relay in relays {
            let relay_url = relay.url.clone(); // Original clone
            let (send_note_tx, send_note_rx) = unbounded::<SignedNote>();
            let (filter_tx, filter_rx) = unbounded::<NostrSubscription>();
            let (unsubscribe_tx, unsubscribe_rx) = unbounded::<String>();
            let (close_tx, close_rx) = unbounded::<()>();
            send_note_channels.push(send_note_tx);
            filter_channels.push(filter_tx);
            unsubscribe_channels.push(unsubscribe_tx);
            close_channels.push(close_tx);
            let note_cb = note_cb.clone();

            // Clone URLs for different async blocks
            let reader_url = relay_url.clone();
            let close_url = relay_url.clone();
            
            spawn_local(async move {
                let relay = NostrRelay::new(&relay_url).await;
                if let Err(_) = relay {
                    ToastifyOptions::new_relay_error(&format!("Failed to connect to relay: {}", relay_url))
                        .show();
                    return;
                };
                
                ToastifyOptions::new_relay_connected(&relay_url).show();
                
                let relay = relay.unwrap();
                let relay_arc = Arc::new(relay);

                let sender_relay = relay_arc.clone();
                spawn_local(async move {
                    while let Ok(note) = send_note_rx.recv().await {
                        if let Err(e) = sender_relay.send_note(note).await {
                            ToastifyOptions::new_relay_error(&format!("Error sending note: {}", e))
                                .show();
                            gloo::console::error!("Error sending note: ", format!("{:?}", e));
                        }
                    }
                });

                let filter_relay = relay_arc.clone();
                spawn_local(async move {
                    while let Ok(filter) = filter_rx.recv().await {
                        if let Err(e) = filter_relay.subscribe(&filter).await {
                            ToastifyOptions::new_relay_error(&format!("Error subscribing: {}", e))
                                .show();
                            gloo::console::error!("Error subscribing: ", format!("{:?}", e));
                        }
                    }
                });

                let unsubscribe_relay = relay_arc.clone();
                spawn_local(async move {
                    while let Ok(filter) = unsubscribe_rx.recv().await {
                        if let Err(e) = unsubscribe_relay.unsubscribe(filter).await {
                            ToastifyOptions::new_relay_error(&format!("Error unsubscribing: {}", e))
                                .show();
                            gloo::console::error!("Error unsubscribing: ", format!("{:?}", e));
                        }
                    }
                });

                let reader_relay = relay_arc.clone();
                spawn_local(async move {
                    while let Ok(event) = reader_relay.relay_event_reader().recv().await {
                        note_cb.emit(event);
                    }
                    ToastifyOptions::new_relay_disconnected(&reader_url).show();
                });

                let close_relay = relay_arc.clone();
                spawn_local(async move {
                    while let Ok(_) = close_rx.recv().await {
                        close_relay.close().await;
                        ToastifyOptions::new_relay_disconnected(&close_url).show();
                    }
                });
            });
        }

        (
            send_note_channels,
            filter_channels,
            unsubscribe_channels,
            close_channels,
        )
    }

    pub fn build_props(&self) -> NostrProps {
        props!(NostrProps {
            relay_events: self.relay_events.clone(),
            notes: self.new_notes.clone(),
            send_note: self.send_note_callback.clone(),
            subscribe: self.subscribe_callback.clone(),
            unsubscribe: self.unsubscribe_callback.clone(),
            close: self.close_callback.clone(),
        })
    }

    fn send_nostr_note(&self, signed_note: SignedNote) {
        self.sender_channel.iter().for_each(|channel| {
            if let Err(e) = channel.try_send(signed_note.clone()) {
                gloo::console::error!("Error sending note: ", format!("{:?}", e));
            }
        });
    }

    fn subscribe(&self, filter: NostrSubscription) {
        self.filter_channel.iter().for_each(|channel| {
            if let Err(e) = channel.try_send(filter.clone()) {
                gloo::console::error!("Error sending filter: ", format!("{:?}", e));
            }
        });
    }

    fn unsubscribe(&self, filter: String) {
        self.unsubscribe_channel.iter().for_each(|channel| {
            if let Err(e) = channel.try_send(filter.clone()) {
                gloo::console::error!("Error sending unsubscribe: ", format!("{:?}", e));
            }
        });
    }

    fn add_event(&mut self, event: RelayEvents) {
        self.relay_events.push(event);
    }

    fn close_ws(&self) {
        self.close_channel.iter().for_each(|channel| {
            if let Err(e) = channel.try_send(()) {
                gloo::console::error!("Error closing WS", format!("{:?}", e));
            }
        });
    }
}
