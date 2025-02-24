use nostro2::notes::NostrNote;
use yew::prelude::*;

use crate::browser_api::IdbStoreManager;

use super::*;
//wasm_bindgen_test_configure!(run_in_browser);
//#[wasm_bindgen_test]
// async fn _user_identity_fb() -> Result<(), web_sys::wasm_bindgen::JsValue> {
//     init_nostr_db().unwrap();
//     let user_identity = UserIdentity::new_local_identity().await?;
//     let user_keys = user_identity.get_user_keys().await?;
//     let user_identity = UserIdentity::find_local_identity().await?;
//     let user_keys2 = user_identity.get_user_keys().await?;
//     assert_eq!(user_keys, user_keys2);
//     Ok(())
// }
#[function_component(NostrIdLoginTest)]
pub fn nostr_id_login_test() -> Html {
    let ctx = use_context::<crate::key_manager::NostrIdStore>().expect("NostrIdStore not found");
    let relay_ctx = use_context::<crate::relay_pool::NostrProps>().expect("No relay ctx found");
    let is_loading = ctx.loaded();
    if !is_loading {
        return html! {
            <p>{"Loading..."}</p>
        };
    }
    let has_identity = ctx.get_identity();
    let pubkey = ctx.get_pubkey();

    let sign_onclick = {
        let ctx = ctx.clone();
        let relay_ctx = relay_ctx.clone();
        Callback::from(move |_| {
            let ctx = ctx.clone();
            let relay_ctx = relay_ctx.clone();
            yew::platform::spawn_local(async move {
                let pubkey = ctx.get_pubkey().expect("No pubkey");
                let note = NostrNote {
                    content: "Test Note".to_string(),
                    pubkey,
                    ..Default::default()
                };
                match ctx.sign_note(note).await {
                    Ok(signed_note) => {
                        gloo::console::log!(format!("Signed note: {:?}", signed_note));
                        relay_ctx.send_note.emit(signed_note);
                    }
                    Err(e) => gloo::console::error!(e),
                }
            });
        })
    };

    let sign_encrypted = {
        let ctx = ctx.clone();
        let relay_ctx = relay_ctx.clone();
        Callback::from(move |_| {
            let ctx = ctx.clone();
            let relay_ctx = relay_ctx.clone();
            yew::platform::spawn_local(async move {
                let pubkey = ctx.get_pubkey().expect("No pubkey");
                let note = NostrNote {
                    content: "Test Note".to_string(),
                    pubkey: pubkey.clone(),
                    ..Default::default()
                };
                match ctx.sign_encrypted_note(note, pubkey).await {
                    Ok(signed_note) => {
                        gloo::console::log!(format!("Signed note: {:?}", signed_note));
                        relay_ctx.send_note.emit(signed_note.clone());
                        let decrypted = ctx
                            .decrypt_note(&signed_note)
                            .await
                            .expect("Decryption failed");
                        gloo::console::log!(format!("Decrypted note: {}", decrypted));
                    }
                    Err(e) => gloo::console::error!(e),
                }
            });
        })
    };

    html! {
        <>
            <h1>{"Nostr Identity Login Test"}</h1>
            <p>{format!("Has Identity: {}", has_identity.is_some())}</p>
            <p>{format!("Has Keys: {}", pubkey.is_some())}</p>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| ctx.dispatch(crate::key_manager::NostrIdAction::FinishedLoadingKey))
            }>
                {"Load Identity"}
            </button>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| ctx.dispatch(crate::key_manager::NostrIdAction::DeleteIdentity))
            }>
                {"Delete Identity"}
            </button>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| {
                    let ctx = ctx.clone();
                    yew::platform::spawn_local(async move {
                        match UserIdentity::new_local_identity().await {
                            Ok(id) => {
                                let pubkey = id.get_pubkey().await.unwrap();
                                id.clone().save_to_store().await.unwrap();
                                ctx.dispatch(NostrIdAction::LoadIdentity(pubkey ,id.clone()))},
                            Err(e) => gloo::console::error!(&e),
                        }
                    });
                })
            }>
                {"New Local Identity"}
            </button>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| {
                    let ctx = ctx.clone();
                    yew::platform::spawn_local(async move {
                        match UserIdentity::new_extension_identity().await {
                            Ok(id) => {
                                let pubkey = id.get_pubkey().await.unwrap();
                                id.clone().save_to_store().await.unwrap();
                                ctx.dispatch(NostrIdAction::LoadIdentity(pubkey,id))},
                            Err(e) => gloo::console::error!(&e),
                        }
                    });
                })
            }>
                {"Reload Identity"}
            </button>
            <button onclick={sign_onclick}>
                {"Sign Note"}
            </button>
            <button onclick={sign_encrypted}>
                {"Sign Encrypted Note"}
            </button>
        </>
    }
}
