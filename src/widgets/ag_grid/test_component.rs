use crate::relay_pool::NostrProps;
use crate::widgets::ag_grid::{create_column, AgGridComponent, AgGridTheme};
use nostro2::notes::NostrNote;
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
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let subscriber = relay_ctx.subscribe.clone();
    use_effect_with((), move |_| {
        let filter = nostro2::relays::NostrSubscription {
            kinds: Some(vec![1]),
            limit: Some(10),
            ..Default::default()
        };
        subscriber.emit(filter.into());
        || ()
    });
    let rows = use_state(Vec::new);
    {
        let rows = rows.clone();
        let notes = relay_ctx.unique_notes.clone();

        use_effect_with(notes, move |notes| {
            // Convert notes to row data
            let new_rows: Vec<NostrNoteRow> = notes
                .iter()
                .filter(|note| note.kind == 1)
                .map(NostrNoteRow::from)
                .collect();

            rows.set(new_rows);
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
