use web_sys::wasm_bindgen::JsValue;

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct UserRelay {
    pub url: String,
    pub read: bool,
    pub write: bool,
}
impl UserRelay {
    pub async fn find_all_relays(
        db: std::rc::Rc<idb::Database>,
    ) -> Result<Vec<Self>, crate::MinionError> {
        let transaction = db.transaction(
            &[crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref()],
            idb::TransactionMode::ReadOnly,
        )?;
        let store = transaction
            .object_store(crate::browser::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
        let relays = store
            .get_all(None, None)?
            .await?
            .into_iter()
            .filter_map(|key| Self::try_from(key).ok())
            .collect();
        Ok(relays)
    }
}
impl TryFrom<JsValue> for UserRelay {
    type Error = JsValue;
    fn try_from(value: JsValue) -> Result<Self, Self::Error> {
        serde_wasm_bindgen::from_value(value).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
impl From<UserRelay> for JsValue {
    fn from(val: UserRelay) -> Self {
        serde_wasm_bindgen::to_value(&val).unwrap_or_default()
    }
}
