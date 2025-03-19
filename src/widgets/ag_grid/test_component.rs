use crate::relay_pool::NostrPoolStore;
use crate::widgets::ag_grid::{create_column, AgGridComponent, AgGridTheme};
use nostro2_web_relay::nostro2::note::NostrNote;
use serde::Serialize;
use yew::prelude::*;

#[derive(Clone, Serialize, PartialEq)]
struct NostrNoteRow {
    id: String,
    pubkey: String,
    content: String,
    created_at: i64,
    kind: u32,
}

impl From<&NostrNote> for NostrNoteRow {
    fn from(note: &NostrNote) -> Self {
        NostrNoteRow {
            id: note.id.clone().unwrap_or_default(),
            pubkey: note.pubkey.clone(),
            content: note.content.clone(),
            created_at: note.created_at,
            kind: note.kind,
        }
    }
}

#[function_component(NostrNotesGrid)]
pub fn nostr_notes_grid() -> Html {
    let relay_ctx = use_context::<NostrPoolStore>().expect("No relay context found");
    use_effect_with(relay_ctx.subscribe.clone(), move |relay_clone| {
        let filter = nostro2_web_relay::nostro2::subscriptions::NostrSubscription {
            kinds: Some(vec![1]),
            limit: Some(10),
            ..Default::default()
        };
        relay_clone.emit(filter.into());
        || ()
    });
    let rows = use_state(Vec::new);
    {
        let rows = rows.clone();

        use_effect_with(relay_ctx.unique_notes.clone(), move |notes| {
            gloo::console::log!("Unique notes:", notes.len());
            if let Some(note) = notes.last() {
                let mut new_rows = (*rows).clone();
                new_rows.push(NostrNoteRow::from(note));
                rows.set(new_rows.to_vec());
            }
            || ()
        });
    }

    let columns = vec![
        {
            let mut col = create_column("content", "Content");
            col.width = Some(400);
            col
        },
        create_column("pubkey", "Author"),
        create_column("created_at", "Time"),
    ];

    html! {
        <div class="w-full h-fit p-4 m-4">
            <h2 class="text-xl mb-4">{"Nostr Text Notes (Kind 1)"}</h2>
            <AgGridComponent<NostrNoteRow>
                data={(*rows).clone()}
                columns={columns}
                theme={AgGridTheme::Quartz}
                class={classes!("h-[500px]")}
            />
        </div>
    }
}
