pub const NOSTR_DB_NAME: &str = "nostr_db";
pub const NOSTR_DB_VERSION: u32 = 4;

/// Store names used before database version 4. The version 4 upgrade created
/// the stores below under new names and left these in place, still holding
/// whatever an earlier release saved.
const LEGACY_IDENTITY_STORE: &str = "user_identities";
const LEGACY_RELAY_STORE: &str = "user_relays";

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
        }
        .migrated()
        .await)
    }

    /// Move what releases before database version 4 saved into the current
    /// stores, then hand the database back.
    ///
    /// This runs after the open and not inside the upgrade, because recovering
    /// the legacy identity needs `exportKey`, and an upgrade transaction
    /// commits as soon as it is left waiting on anything but its own requests.
    ///
    /// Best effort: a legacy store is emptied only after its contents are
    /// safely in the current one, so a failure leaves it untouched and the
    /// next load tries again. The emptied stores stay (removing them needs
    /// another version bump), but they no longer hold a secret.
    async fn migrated(self) -> Self {
        Self::report_migration(self.migrate_legacy_relays().await);
        Self::report_migration(self.migrate_legacy_identity().await);
        self
    }

    fn report_migration(outcome: Result<(), crate::MinionError>) {
        if let Err(e) = outcome {
            crate::DevConsole::error(format_args!(
                "nostr-minions: legacy store migration failed: {e}"
            ));
        }
    }

    async fn migrate_legacy_relays(&self) -> Result<(), crate::MinionError> {
        let pending = self.unmigrated_relays(LEGACY_RELAY_STORE).await?;
        self.save_relays(pending).await?;
        self.clear_legacy(LEGACY_RELAY_STORE).await
    }

    /// The relays of a legacy store that are not yet in the current one.
    /// Their record shape is the current one; a record that is not a relay
    /// is passed over and goes when the legacy store is emptied. A relay saved since the upgrade
    /// wins over its legacy record, the same rule the identity follows.
    async fn unmigrated_relays(
        &self,
        legacy_store: &str,
    ) -> Result<Vec<crate::browser::relay_pool::UserRelay>, crate::MinionError> {
        let current = self.get_relays().await?;
        let legacy = self.read_legacy(legacy_store).await?;
        Ok(legacy
            .into_iter()
            .filter_map(|record| crate::browser::relay_pool::UserRelay::try_from(record).ok())
            .filter(|relay| !current.iter().any(|known| known.url == relay.url))
            .collect())
    }

    async fn save_relays(
        &self,
        relays: Vec<crate::browser::relay_pool::UserRelay>,
    ) -> Result<(), crate::MinionError> {
        for relay in relays {
            self.add_relay(relay).await?;
        }
        Ok(())
    }

    async fn migrate_legacy_identity(&self) -> Result<(), crate::MinionError> {
        let entry = self.unmigrated_identity().await?;
        self.move_identity(entry).await
    }

    /// Save a recovered legacy identity and only then empty its old store.
    /// With nothing to move, the legacy store is left exactly as it is.
    ///
    /// The save is read back before the old store is touched. A commit can
    /// abort (quota, disk error) after its `put` succeeded, and that is not
    /// reported as an error, so a returned `Ok` alone does not prove the key
    /// is stored.
    async fn move_identity(
        &self,
        entry: Option<crate::IdbKeypairEntry>,
    ) -> Result<(), crate::MinionError> {
        let Some(entry) = entry else {
            return Ok(());
        };
        let pubkey = entry.pubkey.clone();
        self.add_identity(entry).await?;
        if !self.identity_saved(&pubkey).await {
            return Err(crate::MinionError::NoIdentityFound);
        }
        self.clear_legacy(LEGACY_IDENTITY_STORE).await
    }

    /// Whether the current store really holds the identity with this pubkey.
    async fn identity_saved(&self, pubkey: &str) -> bool {
        self.get_identity()
            .await
            .is_ok_and(|saved| saved.pubkey == pubkey)
    }

    /// The legacy identity as a current entry, when there is one to move.
    ///
    /// An identity saved since the upgrade wins; the legacy record is then
    /// left alone rather than destroyed.
    async fn unmigrated_identity(
        &self,
    ) -> Result<Option<crate::IdbKeypairEntry>, crate::MinionError> {
        if self.has_identity().await {
            return Ok(None);
        }
        let records = self.read_legacy(LEGACY_IDENTITY_STORE).await?;
        Self::entry_from_legacy(records).await
    }

    async fn has_identity(&self) -> bool {
        !matches!(
            self.get_identity().await,
            Err(crate::MinionError::NoIdentityFound)
        )
    }

    /// Rebuild the legacy identity as a current entry.
    ///
    /// Legacy records are `{ pubkey: "privateKey", crypto_key: CryptoKey }`:
    /// the key field has another name and `pubkey` is a fixed label, so the
    /// real public key is derived from the secret.
    async fn entry_from_legacy(
        records: Vec<wasm_bindgen::JsValue>,
    ) -> Result<Option<crate::IdbKeypairEntry>, crate::MinionError> {
        let Some(crypto_key) = Self::legacy_crypto_key(records) else {
            return Ok(None);
        };
        let keypair = Self::keypair_from(crypto_key).await?;
        Ok(Some(crate::IdbKeypairEntry::from_keypair(keypair).await?))
    }

    /// The first legacy record holding a local `CryptoKey`. The old extension
    /// and bunker placeholders stored a string there and are passed over.
    fn legacy_crypto_key(records: Vec<wasm_bindgen::JsValue>) -> Option<web_sys::CryptoKey> {
        use wasm_bindgen::JsCast as _;
        let field = wasm_bindgen::JsValue::from_str("crypto_key");
        records
            .into_iter()
            .filter_map(|record| web_sys::js_sys::Reflect::get(&record, &field).ok())
            .find_map(|value| value.dyn_into::<web_sys::CryptoKey>().ok())
    }

    /// Recover the Nostr keypair held in a stored `CryptoKey`.
    async fn keypair_from(
        crypto_key: web_sys::CryptoKey,
    ) -> Result<nostro2_signer::NostrKeypair, crate::MinionError> {
        let secret_array = crate::browser::crypto::export_raw_key(crypto_key).await?;
        let secret: [u8; 32] = web_sys::js_sys::Uint8Array::new(&secret_array)
            .to_vec()
            .try_into()
            .map_err(|_| crate::MinionError::NoNostrKeyFound)?;
        Ok(nostro2_signer::NostrKeypair::from_secret_bytes(&secret)?)
    }

    fn has_store(&self, store_name: &str) -> bool {
        self.db.store_names().iter().any(|name| name == store_name)
    }

    /// Every record of a legacy store, or nothing when the store is absent
    /// (a database first created at version 4 never had one).
    async fn read_legacy(
        &self,
        store_name: &str,
    ) -> Result<Vec<wasm_bindgen::JsValue>, crate::MinionError> {
        if !self.has_store(store_name) {
            return Ok(Vec::new());
        }
        let (_transaction, store) = self.open_store(store_name, idb::TransactionMode::ReadOnly)?;
        Self::all_records(store).await
    }

    /// Empty a legacy store, if the database has it.
    async fn clear_legacy(&self, store_name: &str) -> Result<(), crate::MinionError> {
        if !self.has_store(store_name) {
            return Ok(());
        }
        let (transaction, store) = self.open_store(store_name, idb::TransactionMode::ReadWrite)?;
        Self::emptied(transaction, store).await
    }

    fn open_store(
        &self,
        store_name: &str,
        mode: idb::TransactionMode,
    ) -> Result<(idb::Transaction, idb::ObjectStore), crate::MinionError> {
        let transaction = self.db.transaction(&[store_name], mode)?;
        let store = transaction.object_store(store_name)?;
        Ok((transaction, store))
    }

    async fn all_records(
        store: idb::ObjectStore,
    ) -> Result<Vec<wasm_bindgen::JsValue>, crate::MinionError> {
        Ok(store.get_all(None, None)?.await?)
    }

    async fn emptied(
        transaction: idb::Transaction,
        store: idb::ObjectStore,
    ) -> Result<(), crate::MinionError> {
        store.clear()?.await?;
        Self::committed(transaction).await
    }

    async fn committed(transaction: idb::Transaction) -> Result<(), crate::MinionError> {
        transaction.commit()?.await?;
        Ok(())
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
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
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
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
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
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
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

        Ok(Some(Self::keypair_from(keys.keypair).await?))
    }
}
