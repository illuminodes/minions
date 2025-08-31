use yew::prelude::*;

pub const NOSTR_DB_NAME: &str = "nostr_db";
pub const NOSTR_DB_VERSION: u32 = 4;

pub enum NostrDbStoreName {
    UserIdentity,
    UserRelay,
}
impl AsRef<str> for NostrDbStoreName {
    fn as_ref(&self) -> &str {
        match self {
            Self::UserIdentity => "user_identity",
            Self::UserRelay => "user_relay",
        }
    }
}

#[derive(Clone, Debug)]
pub struct IdbManager {
    pub db: Option<std::rc::Rc<idb::Database>>,
}
impl PartialEq for IdbManager {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl IdbManager {
    pub async fn new() -> Result<Self, crate::MinionError> {
        let factory = idb::Factory::new()?;

        // Create an open request for the database
        let mut open_request = factory.open(NOSTR_DB_NAME, Some(NOSTR_DB_VERSION))?;

        // Add an upgrade handler for database
        open_request.on_upgrade_needed(|event| {
            // Get database instance from event
            let database = match idb::DatabaseEvent::database(&event) {
                Ok(db) => db,
                Err(e) => {
                    web_sys::console::error_1(&format!("Error getting database: {e:#?}").into());
                    return;
                }
            };

            let mut store_params_identity = idb::ObjectStoreParams::new();
            store_params_identity.key_path(Some(idb::KeyPath::new_single("pubkey")));
            if database
                .create_object_store(
                    NostrDbStoreName::UserIdentity.as_ref(),
                    store_params_identity,
                )
                .is_err()
            {
                web_sys::console::error_1(&"Error creating object store 'user_identity'".into());
                return;
            }

            // Create the 'user_relay' store with a 'relay_url' key
            // This store doesn't need an additional index since relay_url is the key
            let mut store_params_relay = idb::ObjectStoreParams::new();
            store_params_relay.key_path(Some(idb::KeyPath::new_single("url")));

            if database
                .create_object_store(NostrDbStoreName::UserRelay.as_ref(), store_params_relay)
                .is_err()
            {
                web_sys::console::error_1(&"Error creating object store 'user_relay'".into());
            }
        });

        // `await` open request
        Ok(Self {
            db: Some(std::rc::Rc::new(open_request.await?)),
        })
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
pub fn key_handler(props: &yew::html::ChildrenProps) -> Html {
    let ctx = use_reducer(|| IdbManager { db: None });

    let dispatcher = ctx.dispatcher();
    use_memo((), move |()| {
        yew::platform::spawn_local(async move {
            let manager = match IdbManager::new().await {
                Ok(db) => db,
                Err(e) => {
                    web_sys::console::error_1(&format!("Error getting database: {e:#?}").into());
                    return;
                }
            };
            dispatcher.dispatch(IdbManagerAction::Loaded(manager));
        });
    });
    html! {
        <ContextProvider<IdbStore> context={ctx}>
            {props.children.clone()}
        </ContextProvider<IdbStore>>
    }
}

#[hook]
pub fn use_idb_manager() -> Option<IdbStore> {
    use_context::<IdbStore>()
}

#[hook]
pub fn use_idb_database() -> Option<std::rc::Rc<idb::Database>> {
    let ctx = use_context::<IdbStore>();
    ctx.as_ref().and_then(|ctx| (ctx.db.clone()))
}
