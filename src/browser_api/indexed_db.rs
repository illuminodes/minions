use std::future::Future;

use gloo::console::error;
use web_sys::wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{IdbObjectStore, IdbTransactionMode};
use yew::platform::pinned::oneshot::{self};

pub struct IdbStoreConfig {
    pub db_name: &'static str,
    pub db_version: u32,
    pub store_name: &'static str,
    pub document_key: &'static str,
}

pub trait IdbStoreManager {
    fn config() -> IdbStoreConfig;
    fn key(&self) -> JsValue;
    fn save_to_store(self) -> impl Future<Output = Result<(), JsValue>>
    where
        Self: Into<web_sys::wasm_bindgen::JsValue> + Sized,
    {
        async {
            let object_store_request = Self::request_store_open().await?;
            let request = object_store_request.put(self.into().as_ref())?;
            let req_clone = request.clone();
            let (sender, receiver) = oneshot::channel();
            let on_success =
                Closure::once_into_js(move |_: web_sys::Event| match req_clone.result() {
                    Ok(_) => {
                        let _ = sender.send(());
                    }
                    Err(e) => {
                        error!(&e);
                    }
                });
            let on_error = Closure::once_into_js(move |event: web_sys::Event| {
                error!(&event);
            });
            request.set_onsuccess(Some(on_success.dyn_ref().unwrap()));
            request.set_onerror(Some(on_error.dyn_ref().unwrap()));
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    fn retrieve_from_store<T>(key: &JsValue) -> impl Future<Output = Result<T, JsValue>>
    where
        T: TryFrom<web_sys::wasm_bindgen::JsValue> + 'static,
    {
        async move {
            let object_store = Self::request_store_open().await?;
            let (success_sender, success_receiver) = oneshot::channel::<T>();
            let request = object_store.get(key)?;
            let req_clone = request.clone();
            let on_success =
                Closure::once_into_js(move |_event: web_sys::Event| match req_clone.result() {
                    Ok(result) => {
                        if result.is_null() || result.is_undefined() {
                            error!("Result is null or undefined");
                            return;
                        }
                        match result.try_into() {
                            Ok(value) => {
                                let _ = success_sender.send(value);
                            }
                            Err(_) => {
                                error!("Error converting to T");
                            }
                        }
                    }
                    Err(e) => {
                        error!(&e);
                    }
                });
            let on_error = Closure::once_into_js(move |event: web_sys::Event| {
                gloo::console::log!("Error retrieving from store");
                error!(&event);
            });
            request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
            request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            success_receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    fn retrieve_all_from_store() -> impl Future<Output = Result<Vec<Self>, JsValue>>
    where
        Self: TryFrom<JsValue, Error = JsValue> + 'static + serde::de::DeserializeOwned,
    {
        async {
            let object_store = Self::request_store_open().await?;
            let request = object_store.get_all()?;
            let req_clone = request.clone();
            let (sender, receiver) = oneshot::channel();
            let on_success = Closure::once_into_js(move |_event: web_sys::Event| {
                let result: JsValue = req_clone.result().unwrap();
                let js_array: web_sys::js_sys::Array = result.dyn_into().unwrap();
                let result: Vec<Self> = js_array
                    .iter()
                    .map(|value| {
                        let value: JsValue = value;
                        value.try_into().unwrap()
                    })
                    .collect();
                let _ = sender.send(result);
            });
            request.set_onsuccess(Some(on_success.dyn_ref().unwrap()));
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    fn delete_from_store(&self) -> impl Future<Output = Result<(), JsValue>> {
        async {
            let object_store_request = Self::request_store_open().await?;
            let request = object_store_request.delete(&self.key())?;
            let req_clone = request.clone();
            let (sender, receiver) = oneshot::channel();
            let on_success = Closure::once_into_js(move |_event: web_sys::Event| {
                let _result: JsValue = req_clone.result().unwrap();
                let _ = sender.send(());
            });
            request.set_onsuccess(Some(on_success.dyn_ref().unwrap()));
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    fn request_store_open() -> impl Future<Output = Result<IdbObjectStore, JsValue>> {
        async {
            let db = Self::request_db_open().await?;
            let store_name_str = Self::config().store_name;
            let transaction =
                db.transaction_with_str_and_mode(store_name_str, IdbTransactionMode::Readwrite)?;
            let object_store = transaction.object_store(store_name_str)?;
            Ok(object_store)
        }
    }
    fn request_db_open() -> impl Future<Output = Result<web_sys::IdbDatabase, JsValue>> {
        async {
            let window = web_sys::window().ok_or(JsValue::from_str("No window available."))?;
            let idb_factory = window
                .indexed_db()?
                .ok_or(JsValue::from_str("No IndexedDB"))?;
            let idb_open_request =
                idb_factory.open_with_u32(Self::config().db_name, Self::config().db_version)?;
            let on_upgrade_needed = Closure::once_into_js(move |event: web_sys::Event| {
                let target = event
                    .target()
                    .ok_or(JsValue::from_str("Error upgrading database"))?;
                let db = target
                    .dyn_into::<web_sys::IdbOpenDbRequest>()?
                    .result()?
                    .dyn_into::<web_sys::IdbDatabase>()?;
                if let Err(e) = Self::create_data_store(&db) {
                    error!(&e);
                }
                Ok::<(), JsValue>(())
            });
            let on_error = Closure::once_into_js(move |event: web_sys::Event| {
                error!(&event);
            });

            let db_handle = idb_open_request.clone();
            let (sender, receiver) = oneshot::channel();
            let on_success = Closure::once_into_js(move |_: web_sys::Event| {
                match db_handle.result() {
                    Ok(result) => {
                        if result.is_null() || result.is_undefined() {
                            return Err(JsValue::from_str("Result is null or undefined"));
                        }
                        let db: web_sys::IdbDatabase = result.dyn_into()?;
                        sender.send(db)?;
                    }
                    Err(e) => {
                        error!(&e);
                        drop(sender);
                    }
                }
                Ok::<(), JsValue>(())
            });
            idb_open_request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
            idb_open_request.set_onupgradeneeded(Some(on_upgrade_needed.as_ref().unchecked_ref()));
            idb_open_request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
            let db = receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            Ok(db)
        }
    }
    fn create_data_store(db: &web_sys::IdbDatabase) -> Result<(), JsValue> {
        let user_relay_params = web_sys::IdbObjectStoreParameters::new();
        user_relay_params.set_key_path(&JsValue::from_str(Self::config().document_key));
        db.create_object_store_with_optional_parameters(
            Self::config().store_name,
            &user_relay_params,
        )?;
        Ok(())
    }
}
