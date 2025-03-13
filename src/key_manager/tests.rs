use nostro2::notes::NostrNote;
use yew::prelude::*;

use crate::browser_api::IdbStoreManager;

use super::*;
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
                let mut note = NostrNote {
                    content: "Test Note".to_string(),
                    pubkey,
                    ..Default::default()
                };
                match ctx.sign_note(&mut note).await {
                    Ok(_) => {
                        gloo::console::log!(format!("Signed note: {:?}", note));
                        relay_ctx.send_note.emit(note);
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
                let mut note = NostrNote {
                    content: "Test Note".to_string(),
                    pubkey: pubkey.clone(),
                    ..Default::default()
                };
                match ctx.sign_encrypted_note(&mut note, pubkey).await {
                    Ok(_) => {
                        gloo::console::log!(format!("Signed encrypted note: {:?}", note));
                        relay_ctx.send_note.emit(note.clone());
                        let decrypted = ctx.decrypt_note(&note).await.expect("Decryption failed");
                        gloo::console::log!(format!("Decrypted note: {}", decrypted));
                    }
                    Err(e) => gloo::console::error!(e),
                }
            });
        })
    };

    // onclick handler for testing giftwrapping
    let test_giftwrap = {
        let ctx = ctx.clone();
        let relay_ctx = relay_ctx.clone();
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
                    gloo::console::error!("Failed to sign inner note:", e);
                    return;
                }

                gloo::console::log!("Created and signed inner note");

                // Create the giftwrapped note (unsigned)
                let kind = 20001; // Example custom kind for giftwraps
                match ctx
                    .create_giftwrap(inner_note.clone(), pubkey.clone(), kind)
                    .await
                {
                    Ok(mut giftwrapped_note) => {
                        gloo::console::log!("Successfully created unsigned giftwrapped note");

                        // Sign and encrypt the giftwrapped note
                        match ctx
                            .sign_encrypted_note(&mut giftwrapped_note, pubkey.clone())
                            .await
                        {
                            Ok(_) => {
                                gloo::console::log!(
                                    "Successfully signed and encrypted giftwrapped note:"
                                );
                                gloo::console::log!(format!("Kind: {}", giftwrapped_note.kind));
                                gloo::console::log!(format!(
                                    "Content length: {}",
                                    giftwrapped_note.content.len()
                                ));

                                // Test unwrapping (decrypting) the note
                                match ctx.unwrap_giftwrap(&giftwrapped_note).await {
                                    Ok(unwrapped_note) => {
                                        gloo::console::log!("Successfully unwrapped note:");
                                        gloo::console::log!(format!(
                                            "Content: {}",
                                            unwrapped_note.content
                                        ));
                                        gloo::console::log!(format!(
                                            "Original content: {}",
                                            inner_note.content
                                        ));

                                        // Verify contents match
                                        if unwrapped_note.content == inner_note.content {
                                            gloo::console::log!("✅ TEST PASSED: Unwrapped content matches original");
                                        } else {
                                            gloo::console::error!("❌ TEST FAILED: Unwrapped content doesn't match original");
                                        }

                                        // Optionally send the giftwrapped note to demonstrate it in relay
                                        relay_ctx.send_note.emit(giftwrapped_note);
                                    }
                                    Err(e) => gloo::console::error!("Failed to unwrap note:", e),
                                }
                            }
                            Err(e) => gloo::console::error!(
                                "Failed to sign and encrypt giftwrapped note:",
                                e
                            ),
                        }
                    }
                    Err(e) => gloo::console::error!("Failed to create giftwrapped note:", e),
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
            // Add the new button for testing giftwrapping
            <button onclick={test_giftwrap}>
                {"Test Giftwrapping"}
            </button>
        </>
    }
}
