mod db;
pub use db::*;

use yew::prelude::*;

#[derive(Clone, Debug)]
pub struct IdbManager {
    pub db: db::NostrIdb,
}
impl PartialEq for IdbManager {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
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
pub fn key_handler(props: &yew::html::ChildrenProps) -> HtmlResult {
    let db = yew::suspense::use_future_with((), |_| async move {
        crate::idb_manager::IdbManager::new().await
    })?;
    let Ok(db) = (db).as_ref().cloned() else {
        web_sys::console::error_1(&"No Idb Manager".into());
        return Ok(html! {
            <yew::suspense::Suspense />
        });
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
pub fn use_idb_database() -> db::NostrIdb {
    let ctx = use_context::<IdbStore>().expect("No IdbStore context found");
    ctx.db.clone()
}
