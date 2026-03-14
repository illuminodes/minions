use nostro2::NostrSigner;
use nostro2_signer::nostro2::NostrNote;
use yew::prelude::*;

#[function_component(NostrIdLoginTestSuspense)]
pub fn nostr_id_login_test_suspense() -> Html {
    crate::use_nostr_key().map_or_else(
        || {
            html! {
                <>
                    <h1>{"Nostr Identity Login Test"}</h1>
                    <p>{"No key found"}</p>
                </>
            }
        },
        |key| {
            html! {
                <>
                    <h1>{"Nostr Identity Login Test"}</h1>
                    <p>{"Has key: "}{key.public_key()}</p>
                </>
            }
        },
    )
}

#[function_component(NostrIdLoginTest)]
pub fn nostr_id_login_test() -> Html {
    let ctx = crate::use_nostr_id_ctx();
    let relay_ctx = crate::use_nostr_relay_pool();
    let create_local_key = crate::use_create_local_key();
    let sign_onclick = {
        let ctx = ctx.clone();
        let relay_ctx = relay_ctx.clone();
        Callback::from(move |_| {
            let relay_ctx = relay_ctx.clone();

            let pubkey = ctx.get_pubkey().expect("No pubkey");
            let mut note = nostro2::NostrNote {
                content: "Test Note".to_string(),
                pubkey,
                ..Default::default()
            };
            match ctx.sign_note(&mut note) {
                Ok(()) => {
                    relay_ctx.send(note.clone());
                }
                Err(e) => web_sys::console::error_1(&format!("Error signing note: {e:#?}").into()),
            }
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
                match ctx.sign_encrypted_note(&mut note, &pubkey) {
                    Ok(()) => {
                        relay_ctx.send(note.clone());
                        let decrypted = ctx.decrypt_note(&note).expect("Decryption failed");
                        web_sys::console::log_1(&format!("Decrypted content: {decrypted}").into());
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Error signing note: {e:#?}").into());
                    }
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
                if let Err(e) = ctx.sign_note(&mut inner_note) {
                    web_sys::console::error_1(&format!("Error signing note: {e:#?}").into());
                    return;
                }

                // Create the giftwrapped note (unsigned)
                match ctx.create_giftwrap(
                    &mut inner_note,
                    &pubkey,
                    &nostro2_signer::keypair::GiftwrapScheme::Ephemeral,
                ) {
                    Ok(mut giftwrapped_note) => {
                        // Sign and encrypt the giftwrapped note
                        match ctx.sign_encrypted_note(&mut giftwrapped_note, &pubkey) {
                            Ok(()) => {
                                // Test unwrapping (decrypting) the note
                                match ctx.decrypt_note(&giftwrapped_note) {
                                    Ok(unwrapped_note) => {
                                        // Verify contents match
                                        if unwrapped_note == inner_note.content {
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
                                    Err(e) => web_sys::console::error_1(
                                        &format!("Error unwrapping note: {e:#?}").into(),
                                    ),
                                }
                            }
                            Err(e) => web_sys::console::error_1(
                                &format!("Error creating giftwrap: {e:#?}").into(),
                            ),
                        }
                    }
                    Err(e) => web_sys::console::error_1(
                        &format!("Error creating giftwrap: {e:#?}").into(),
                    ),
                }
            });
        })
    };

    html! {
        <>
            <h1>{"Nostr Identity Login Test"}</h1>
            <yew::suspense::Suspense fallback={html!{<p>{"Loading..."}</p>}}>
                <NostrIdLoginTestSuspense/>
            </yew::suspense::Suspense>
            <button onclick={
                let ctx = ctx.clone();
                Callback::from(move |_| ctx.dispatch(crate::key_manager::NostrIdAction::DeleteIdentity))
            }>
                {"Delete Identity"}
            </button>
            <button onclick={create_local_key.reform(|_| nostro2_signer::keypair::NostrKeypair::generate(true))}>
                {"New Local Identity"}
            </button>
            <button onclick={Callback::from(move |_| {})}>
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
