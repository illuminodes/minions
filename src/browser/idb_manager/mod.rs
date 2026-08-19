mod db;
pub use db::NostrDbStoreName;
pub use db::NostrIdb;
use yew::prelude::*;

#[derive(Clone, Debug)]
pub struct IdbManager {
    pub db: db::NostrIdb,
}
impl PartialEq for IdbManager {
    fn eq(&self, other: &Self) -> bool {
        self.db == other.db
    }
}
impl IdbManager {
    pub async fn new() -> Result<Self, crate::MinionError> {
        let db = db::NostrIdb::new().await?;
        Ok(Self { db })
    }
}

pub enum IdbManagerAction {
    Loaded(IdbManager),
}

impl Reducible for IdbManager {
    type Action = IdbManagerAction;
    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            IdbManagerAction::Loaded(db) => std::rc::Rc::new(db),
        }
    }
}

pub type IdbStore = UseReducerHandle<IdbManager>;

#[function_component(IdbManagerProvider)]
pub fn idb_manager_provider(props: &yew::html::ChildrenProps) -> HtmlResult {
    let db = yew::suspense::use_future_with((), |_| async move {
        crate::browser::idb_manager::IdbManager::new().await
    })?;
    let Ok(db) = (db).as_ref().cloned() else {
        // Log the error so developers can diagnose IndexedDB failures
        // (private browsing, storage quota, unsupported browser, etc.)
        // Render children without context so downstream hooks return None
        // and components can degrade gracefully instead of showing a blank screen.
        if let Err(e) = db.as_ref() {
            web_sys::console::error_1(&format!("nostr-minions: IndexedDB unavailable: {e}").into());
        }
        return Ok(html! { {props.children.clone()} });
    };

    let ctx = use_reducer(|| db);

    Ok(html! {
        <ContextProvider<IdbStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<IdbStore>>
    })
}

#[hook]
pub fn use_idb_manager() -> Option<IdbStore> {
    use_context::<IdbStore>()
}

#[hook]
pub fn use_idb_database() -> Option<db::NostrIdb> {
    let ctx = use_context::<IdbStore>()?;
    Some(ctx.db.clone())
}
