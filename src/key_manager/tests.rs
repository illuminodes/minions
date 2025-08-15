use nostro2_signer::nostro2::NostrNote;
use yew::prelude::*;

use crate::browser_api::IdbStoreManager;

use super::{NostrIdAction, UserIdentity};
#[function_component(NostrIdLoginTest)]
pub fn nostr_id_login_test() -> Html {
    let ctx = use_context::<crate::key_manager::NostrIdStore>().expect("NostrIdStore not found");
    let relay_ctx =
        use_context::<crate::relay_pool::NostrRelayPoolStore>().expect("No relay ctx found");
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
                let mut note = nostro2::NostrNote {
                    content: "Test Note".to_string(),
                    pubkey,
                    ..Default::default()
                };
                match ctx.sign_note(&mut note).await {
                    Ok(()) => {
                        relay_ctx.send(note.clone());
                    }
                    Err(e) => web_sys::console::error_1(&e),
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
                let mut note = NostrNote {
                    content: "Test Note".to_string(),
                    pubkey: pubkey.clone(),
                    ..Default::default()
                };
                match ctx.sign_encrypted_note(&mut note, pubkey).await {
                    Ok(()) => {
                        relay_ctx.send(note.clone());
                        let decrypted = ctx.decrypt_note(&note).await.expect("Decryption failed");
                        web_sys::console::log_1(&format!("Decrypted content: {decrypted}").into());
                    }
                    Err(e) => web_sys::console::error_1(&e),
                }
            });
        })
    };

    // onclick handler for testing giftwrapping
    let test_giftwrap = {
        let ctx = ctx.clone();
        let relay_ctx = relay_ctx;
        Callback::from(move |_| {
            let ctx = ctx.clone();
            let relay_ctx = relay_ctx.clone();
            yew::platform::spawn_local(async move {
                let pubkey = ctx.get_pubkey().expect("No pubkey");

                // Create an inner test note
                let mut inner_note = NostrNote {
                    content: "This is a secret message inside a giftwrapped note".to_string(),
                    pubkey: pubkey.clone(),
                    kind: 1, // Using kind 1 for the inner note (text note)
                    ..Default::default()
                };

                // Sign the inner note
                if let Err(e) = ctx.sign_note(&mut inner_note).await {
                    web_sys::console::error_1(&e);
                    return;
                }

                // Create the giftwrapped note (unsigned)
                let kind = 20001; // Example custom kind for giftwraps
                match ctx.create_giftwrap(inner_note.clone(), kind).await {
                    Ok(mut giftwrapped_note) => {
                        // Sign and encrypt the giftwrapped note
                        match ctx
                            .sign_encrypted_note(&mut giftwrapped_note, pubkey.clone())
                            .await
                        {
                            Ok(()) => {
                                // Test unwrapping (decrypting) the note
                                match ctx.unwrap_giftwrap(&giftwrapped_note).await {
                                    Ok(unwrapped_note) => {
                                        // Verify contents match
                                        if unwrapped_note.content == inner_note.content {
                                            web_sys::console::log_1(
                                                &"Giftwrap test passed: Contents match".into(),
                                            );
                                        } else {
                                            web_sys::console::error_1(
                                                &"Giftwrap test failed: Contents do not match"
                                                    .into(),
                                            );
                                        }

                                        // Optionally send the giftwrapped note to demonstrate it in relay
                                        relay_ctx.send(giftwrapped_note);
                                    }
                                    Err(e) => web_sys::console::error_1(&e),
                                }
                            }
                            Err(e) => web_sys::console::error_1(&e),
                        }
                    }
                    Err(e) => web_sys::console::error_1(&e),
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
                                ctx.dispatch(NostrIdAction::LoadIdentity(pubkey ,id.clone()));},
                            Err(e) => web_sys::console::error_1(&e.into()),
                        }
                    });
                })
            }>
                {"New Local Identity"}
            </button>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| {
                    let _ctx = ctx.clone();
                    yew::platform::spawn_local(async move {
                        // match UserIdentity::new_extension_identity().await {
                        //     Ok(id) => {
                        //         let pubkey = id.get_pubkey().await.unwrap();
                        //         id.clone().save_to_store().await.unwrap();
                        //         ctx.dispatch(NostrIdAction::LoadIdentity(pubkey,id));},
                        //     Err(e) => gloo::console::error!(&e),
                        // }
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
            // Add the new button for testing giftwrapping
            <button onclick={test_giftwrap}>
                {"Test Giftwrapping"}
            </button>
        </>
    }
}
