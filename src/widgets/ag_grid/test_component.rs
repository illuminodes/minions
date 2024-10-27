use crate::relay_pool::relay_pool::NostrProps;
use crate::widgets::ag_grid::{AgGridComponent, create_column};
use nostro2::notes::SignedNote;
use serde::Serialize;
use yew::prelude::*;

#[derive(Clone, Serialize, PartialEq)]
struct NostrNoteRow {
    id: String,
    pubkey: String,
    content: String,
    created_at: u64,  
    kind: u32,        
}

impl From<&SignedNote> for NostrNoteRow {
    fn from(note: &SignedNote) -> Self {
        NostrNoteRow {
            id: note.get_id().to_string(),
            pubkey: note.get_pubkey().to_string(),
            content: note.get_content().to_string(),
            created_at: note.get_created_at(),
            kind: note.get_kind(),
        }
    }
}

#[function_component(NostrNotesGrid)]
pub fn nostr_notes_grid() -> Html {
    let relay_ctx = use_context::<NostrProps>().expect("No relay context found");
    let rows = use_state(Vec::new);
    {
        let rows = rows.clone();
        let notes = relay_ctx.unique_notes.clone();

        use_effect_with(
            notes,
            move |notes| {
                // Convert notes to row data
                let new_rows: Vec<NostrNoteRow> = notes
                    .iter()
                    .filter(|note| note.get_kind() == 1)
                    .map(NostrNoteRow::from)
                    .collect();
                
                rows.set(new_rows);
                || ()
            }
        );
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
        <div class="w-full h-full">
            <h2 class="text-xl mb-4">{"Nostr Text Notes (Kind 1)"}</h2>
            <AgGridComponent<NostrNoteRow>
                data={(*rows).clone()}
                columns={columns}
                class={classes!("h-[500px]")}
            />
        </div>
    }
}