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
pub struct NostrIdb {
    pub db: std::rc::Rc<idb::Database>,
}
impl PartialEq for NostrIdb {
    fn eq(&self, other: &Self) -> bool {
        std::rc::Rc::ptr_eq(&self.db, &other.db)
    }
}
impl NostrIdb {
    /// Creates a new instance of the database, and performs an upgrade if needed.
    /// # Errors
    /// Returns an error if the database fails, which can be caused by the browser not supporting `IndexedDB`.
    pub async fn new() -> Result<Self, crate::MinionError> {
        let factory = idb::Factory::new()?;

        let mut open_request = factory.open(NOSTR_DB_NAME, Some(NOSTR_DB_VERSION))?;

        open_request.on_upgrade_needed(|event| {
            let _ = Self::upgrade(&event);
        });

        let db = open_request.await?;
        Ok(Self {
            db: std::rc::Rc::new(db),
        })
    }
    /// Upgrades the database to the latest version. Checks for existing stores
    /// before creating them to support upgrades from any previous version.
    ///
    /// Stores:
    /// - `user_identity`: Stores the user's identity information, including their public key and private key.
    /// - `user_relay`: Stores the user's relay information, including their URL and read/write permissions.
    ///
    /// Keypaths:
    /// - `user_identity`: pubkey
    /// - `user_relay`: url
    ///
    /// # Errors
    /// Returns an error if the database fails.
    fn upgrade(event: &idb::event::VersionChangeEvent) -> Result<(), crate::MinionError> {
        let database = idb::DatabaseEvent::database(event)?;
        let existing_stores = database.store_names();

        if !existing_stores
            .iter()
            .any(|n| n == NostrDbStoreName::UserIdentity.as_ref())
        {
            let mut store_params_identity = idb::ObjectStoreParams::new();
            store_params_identity.key_path(Some(idb::KeyPath::new_single("pubkey")));
            database.create_object_store(
                NostrDbStoreName::UserIdentity.as_ref(),
                store_params_identity,
            )?;
        }

        if !existing_stores
            .iter()
            .any(|n| n == NostrDbStoreName::UserRelay.as_ref())
        {
            let mut store_params_relay = idb::ObjectStoreParams::new();
            store_params_relay.key_path(Some(idb::KeyPath::new_single("url")));
            database
                .create_object_store(NostrDbStoreName::UserRelay.as_ref(), store_params_relay)?;
        }

        Ok(())
    }

    pub async fn get_relays(
        &self,
    ) -> Result<Vec<crate::browser::relay_pool::UserRelay>, crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref()],
            idb::TransactionMode::ReadOnly,
        )?;
        let store =
            transaction.object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
        let relays = store
            .get_all(None, None)?
            .await?
            .into_iter()
            .filter_map(|key| crate::browser::relay_pool::UserRelay::try_from(key).ok())
            .collect();
        Ok(relays)
    }
    pub async fn add_relay(
        &self,
        relay: crate::browser::relay_pool::UserRelay,
    ) -> Result<(), crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref()],
            idb::TransactionMode::ReadWrite,
        )?;
        let store =
            transaction.object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
        store
            .put(&serde_wasm_bindgen::to_value(&relay)?, None)?
            .await?;
        transaction.commit()?.await?;
        Ok(())
    }
    pub async fn remove_relay(&self, relay_url: String) -> Result<(), crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref()],
            idb::TransactionMode::ReadWrite,
        )?;
        let store =
            transaction.object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
        store
            .delete(wasm_bindgen::JsValue::from_str(relay_url.as_str()))?
            .await?;
        transaction.commit()?.await?;
        Ok(())
    }

    pub async fn get_identity(&self) -> Result<crate::IdbKeypairEntry, crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref()],
            idb::TransactionMode::ReadOnly,
        )?;
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref())?;
        let keys = store.get_all(None, Some(1))?.await?;
        let Some(keys) = keys
            .into_iter()
            .next()
            .and_then(|key| serde_wasm_bindgen::from_value::<crate::IdbKeypairEntry>(key).ok())
        else {
            return Err(crate::MinionError::NoIdentityFound);
        };
        Ok(keys)
    }
    pub async fn add_identity(
        &self,
        identity: crate::IdbKeypairEntry,
    ) -> Result<(), crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref()],
            idb::TransactionMode::ReadWrite,
        )?;
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref())?;
        store
            .put(&serde_wasm_bindgen::to_value(&identity)?, None)?
            .await?;
        transaction.commit()?.await?;
        Ok(())
    }
    pub async fn remove_identity(&self, pubkey: String) -> Result<(), crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref()],
            idb::TransactionMode::ReadWrite,
        )?;
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref())?;
        store
            .delete(wasm_bindgen::JsValue::from_str(pubkey.as_str()))?
            .await?;
        transaction.commit()?.await?;
        Ok(())
    }
    pub async fn load_identity(
        &self,
    ) -> Result<Option<nostro2_signer::NostrKeypair>, crate::MinionError> {
        let transaction = self.db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref()],
            idb::TransactionMode::ReadOnly,
        )?;
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserIdentity.as_ref())?;
        let keys = store.get_all(None, Some(1))?.await?;
        let Some(keys) = keys
            .into_iter()
            .next()
            .and_then(|key| serde_wasm_bindgen::from_value::<crate::IdbKeypairEntry>(key).ok())
        else {
            return Ok(None);
        };

        let secret_array = crate::browser::crypto::export_raw_key(keys.keypair).await?;
        let secret_slice = web_sys::js_sys::Uint8Array::new(&secret_array);
        let secret: [u8; 32] = secret_slice
            .to_vec()
            .try_into()
            .map_err(|_| crate::MinionError::NoNostrKeyFound)?;
        let keypair = nostro2_signer::NostrKeypair::from_secret_bytes(&secret)?;
        Ok(Some(keypair))
    }
}
