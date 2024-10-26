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
    
    // Convert notes to row data
    let rows: Vec<NostrNoteRow> = relay_ctx
        .unique_notes
        .iter()
        .filter(|note| note.get_kind() == 1) // Only kind 1 events
        .map(NostrNoteRow::from)
        .collect();

    // Define columns
    let columns = vec![
        {
            let mut col = create_column("content", "Content");
            col.width = Some(400);  // Use width instead of flex
            col
        },
        create_column("pubkey", "Author"),
        create_column("created_at", "Time"),
    ];

    html! {
        <div class="w-full h-full">
            <h2 class="text-xl mb-4">{"Nostr Text Notes (Kind 1)"}</h2>
            <AgGridComponent<NostrNoteRow>
                data={rows}
                columns={columns}
                class={classes!("h-[500px]")}
            />
        </div>
    }
}