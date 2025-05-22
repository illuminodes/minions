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
            let object_store = Self::request_store_open().await?;
            let request = object_store.put(self.into().as_ref())?;
            let (sender, receiver) = oneshot::channel();
            Self::handle_request(
                &request,
                move |_| {
                    let _ = sender.send(());
                },
                move |e| {
                    error!(&e);
                },
            );
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }

    #[must_use]
    fn retrieve_from_store<T>(key: &JsValue) -> impl Future<Output = Result<T, JsValue>>
    where
        T: TryFrom<JsValue> + 'static,
    {
        async move {
            let object_store = Self::request_store_open().await?;
            let request = object_store.get(key)?;
            let (sender, receiver) = oneshot::channel();
            Self::handle_request(
                &request,
                move |result| {
                    if result.is_null() || result.is_undefined() {
                        error!("Result is null or undefined");
                        return;
                    }
                    match result.try_into() {
                        Ok(value) => {
                            let _ = sender.send(value);
                        }
                        Err(_) => {
                            error!("Error converting to T");
                        }
                    }
                },
                move |e| {
                    error!(format!("Error retrieving from store: {e:?}"));
                },
            );

            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    #[must_use]
    fn retrieve_all_from_store() -> impl Future<Output = Result<Vec<Self>, JsValue>>
    where
        Self: TryFrom<JsValue, Error = JsValue> + 'static + serde::de::DeserializeOwned,
    {
        async {
            let object_store = Self::request_store_open().await?;
            let request = object_store.get_all()?;
            let (sender, receiver) = oneshot::channel();
            Self::handle_request(
                &request,
                move |result| {
                    let Ok(js_array) = result
                        .dyn_into::<web_sys::js_sys::Array>()
                        .map_err(|_| JsValue::from_str("Expected an array"))
                    else {
                        error!("Error converting to array");
                        return;
                    };
                    let result: Vec<Self> = js_array
                        .iter()
                        .filter_map(|value| value.try_into().ok())
                        .collect();
                    let _ = sender.send(result);
                },
                move |e| {
                    error!(format!("Error retrieving all from store: {e:?}"));
                },
            );
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    fn delete_from_store(&self) -> impl Future<Output = Result<(), JsValue>> {
        async {
            let object_store = Self::request_store_open().await?;
            let request = object_store.delete(&self.key())?;
            let (sender, receiver) = oneshot::channel();

            Self::handle_request(
                &request,
                move |_| {
                    let _ = sender.send(());
                },
                move |e| {
                    error!("Error deleting from store: ", e);
                },
            );

            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
    #[must_use]
    fn clear_store() -> impl Future<Output = Result<(), JsValue>> {
        async {
            let object_store = Self::request_store_open().await?;
            let request = object_store.clear()?;
            let (sender, receiver) = oneshot::channel();
            Self::handle_request(
                &request,
                move |_| {
                    let _ = sender.send(());
                },
                move |e| error!(e),
            );
            receiver
                .await
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }

    #[must_use]
    fn request_store_open() -> impl Future<Output = Result<IdbObjectStore, JsValue>> {
        async {
            let db = Self::request_db_open().await.ok_or_else(|| {
                error!("Failed to open database");
                JsValue::from_str("Failed to open database")
            })?;
            let store_name_str = Self::config().store_name;
            let transaction =
                db.transaction_with_str_and_mode(store_name_str, IdbTransactionMode::Readwrite)?;
            let object_store = transaction.object_store(store_name_str)?;
            Ok(object_store)
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
    fn handle_request(
        request: &web_sys::IdbRequest,
        on_success: impl FnOnce(JsValue) + 'static,
        on_error: impl FnOnce(JsValue) + 'static,
    ) {
        let error = request.clone();
        let result = request.clone();
        let success_closure = Closure::once(move |_: web_sys::Event| {
            let Ok(result) = result.result() else {
                error!("Error retrieving from store");
                return;
            };
            on_success(result);
        });
        let error_closure = Closure::once(move |_: web_sys::Event| {
            let Ok(Some(error)) = error.error() else {
                on_error(JsValue::from_str("Unknown error"));
                return;
            };
            on_error(error.into());
        });
        request.set_onsuccess(Some(success_closure.as_ref().unchecked_ref()));
        request.set_onerror(Some(error_closure.as_ref().unchecked_ref()));
        success_closure.forget();
        error_closure.forget();
    }
    #[must_use]
    fn request_db_open() -> impl Future<Output = Option<web_sys::IdbDatabase>> {
        async {
            let window = web_sys::window()?;
            let idb_factory = window.indexed_db().ok()??;

            let config = Self::config();
            let request = idb_factory
                .open_with_u32(config.db_name, config.db_version)
                .ok()?;

            let (sender, receiver) = oneshot::channel();

            // Handle upgrade: create store only if it doesn't exist
            let on_upgrade_needed = Closure::wrap(Box::new(move |event: web_sys::Event| {
                let Some(target) = event.target() else {
                    error!("Upgrade event target missing");
                    return;
                };
                let Ok(request) = target.dyn_into::<web_sys::IdbOpenDbRequest>() else {
                    error!("Failed to cast to IdbOpenDbRequest");
                    return;
                };
                let Ok(db) = request
                    .result()
                    .and_then(web_sys::wasm_bindgen::JsCast::dyn_into::<web_sys::IdbDatabase>)
                else {
                    error!("Failed to get DB result");
                    return;
                };

                if !db
                    .object_store_names()
                    .unchecked_into::<web_sys::js_sys::Array>()
                    .iter()
                    .any(|s| s == *config.store_name)
                {
                    let store_params = web_sys::IdbObjectStoreParameters::new();
                    store_params.set_key_path(&JsValue::from_str(config.document_key));
                    if let Err(e) = db.create_object_store_with_optional_parameters(
                        config.store_name,
                        &store_params,
                    ) {
                        error!("Error creating store: ", e);
                    }
                }
            }) as Box<dyn FnMut(_)>);

            // Handle success
            let request_clone = request.clone();
            let on_success =
                Closure::once_into_js(move |_event: web_sys::Event| match request_clone.result() {
                    Ok(db_value) if !db_value.is_null() && !db_value.is_undefined() => {
                        if let Ok(db) = db_value.dyn_into::<web_sys::IdbDatabase>() {
                            let _ = sender.send(db);
                        } else {
                            error!("Failed to cast result into IdbDatabase");
                        }
                    }
                    _ => {
                        error!("DB open success, but result is null/undefined");
                    }
                });

            let on_error = Closure::wrap(Box::new(move |event: web_sys::Event| {
                error!("Database open error: ", event);
            }) as Box<dyn FnMut(_)>);

            // Set handlers
            request.set_onupgradeneeded(Some(on_upgrade_needed.as_ref().unchecked_ref()));
            request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
            request.set_onerror(Some(on_error.as_ref().unchecked_ref()));

            // Forget to leak closures
            on_upgrade_needed.forget();
            on_error.forget();

            receiver.await.ok()
        }
    }
}
