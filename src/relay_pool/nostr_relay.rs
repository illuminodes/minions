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
            &[crate::idb_manager::NostrDbStoreName::UserRelay.as_ref()],
            idb::TransactionMode::ReadOnly,
        )?;
        let store =
            transaction.object_store(crate::idb_manager::NostrDbStoreName::UserRelay.as_ref())?;
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
        let string = web_sys::js_sys::JSON::stringify(&value)?
            .as_string()
            .ok_or(value)?;
        serde_json::from_str(&string).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
impl From<UserRelay> for JsValue {
    fn from(val: UserRelay) -> Self {
        let string = serde_json::to_string(&val).unwrap_or_default();
        web_sys::js_sys::JSON::parse(&string).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {

    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn _relay_idb_manager() -> Result<(), crate::MinionError> {
        Ok(())
    }
}
