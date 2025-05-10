use crate::browser_api::HtmlDocument;
use crate::constants::{
    NOSTR_KIND_PRESIGNED_URL_REQ, NOSTR_KIND_PRESIGNED_URL_RESP, NOSTR_KIND_SERVER_REQUEST,
};
use crate::key_manager::UserIdentity;
use crate::relay_pool::NostrRelayPoolStore;
use lucide_yew::Plus;
use nostro2_signer::nostro2::note::NostrNote;
use upload_things::{UtPreSignedUrl, UtUpload};
use web_sys::{FileReader, FormData, HtmlInputElement};
use yew::{platform::spawn_local, prelude::*};

#[derive(Clone, Debug, Properties, PartialEq)]
pub struct ImageUploadInputProps {
    pub url_handle: UseStateHandle<Option<String>>,
    pub nostr_keys: UserIdentity,
    pub classes: Classes,
    pub image_classes: Classes,
    pub input_id: String,
    pub server_pubkey: String,
}

#[function_component(ImageUploadInput)]
pub fn image_upload_input(props: &ImageUploadInputProps) -> Html {
    let ImageUploadInputProps {
        url_handle,
        nostr_keys,
        classes,
        mut image_classes,
        input_id,
        server_pubkey,
    } = props.clone();
    let relay_pool = use_context::<NostrRelayPoolStore>().expect("No RelayPool Context found");
    let user_keys = nostr_keys.clone();
    let url_clone = url_handle.clone();
    let is_loading_new = use_state(|| false);
    let loading_handle = is_loading_new.clone();

    // Add subscription for presigned URL responses
    {
        let relay_ctx = relay_pool.clone();
        use_effect_with((), move |()| {
            // Create subscription for presigned URL responses
            let filter = nostro2_web_relay::nostro2::subscriptions::NostrSubscription {
                kinds: Some(vec![NOSTR_KIND_PRESIGNED_URL_RESP]),
                limit: Some(20),
                ..Default::default()
            };

            // Send subscription to relay
            gloo::console::log!("Subscribing to presigned URL responses (kind 20421)");
            relay_ctx.send(filter);

            || {}
        });
    }
    // Clone input_id for use in the effect
    let input_id_for_effect = input_id.clone();

    use_effect_with(relay_pool.unique_notes.clone(), move |notes| {
        if let Some(last_note) = notes.last().cloned() {
            if last_note.kind == NOSTR_KIND_PRESIGNED_URL_RESP {
                spawn_local(async move {
                    let decrypted_content = match user_keys.decrypt_nip44(&last_note).await {
                        Ok(content) => {
                            gloo::console::log!("Successfully decrypted response");
                            content
                        }
                        Err(e) => {
                            gloo::console::error!("Failed to decrypt note:", e);
                            return;
                        }
                    };
                    gloo::console::log!("Decrypted note content:", &decrypted_content);

                    let presigned_url: UtPreSignedUrl = match decrypted_content.try_into() {
                        Ok(url) => url,
                        Err(e) => {
                            gloo::console::error!("Failed to parse presigned url:", e.to_string());
                            return;
                        }
                    };

                    // Get document and input safely
                    let document = match HtmlDocument::new() {
                        Ok(doc) => doc,
                        Err(e) => {
                            gloo::console::error!("Failed to get document:", e);
                            return;
                        }
                    };

                    let input: HtmlInputElement =
                        match document.find_element_by_id(&input_id_for_effect) {
                            Ok(input) => input,
                            Err(e) => {
                                gloo::console::error!("Failed to find input element:", e);
                                return;
                            }
                        };

                    let Some(files) = input.files() else {
                        gloo::console::error!("No files found");
                        return;
                    };

                    let Some(file) = files.get(0) else {
                        gloo::console::error!("No file selected");
                        return;
                    };

                    // Create form data
                    let form_data = match FormData::new() {
                        Ok(form) => form,
                        Err(e) => {
                            gloo::console::error!("Failed to create form data:", e);
                            return;
                        }
                    };

                    if let Err(e) = form_data.append_with_blob("file", &file) {
                        gloo::console::error!("Failed to append file to form data:", e);
                        return;
                    }

                    // Create reader and set up upload
                    let reader = match FileReader::new() {
                        Ok(reader) => reader,
                        Err(e) => {
                            gloo::console::error!("Failed to create file reader:", e);
                            return;
                        }
                    };

                    let reader_handle = reader.clone();
                    let loading_handle = loading_handle.clone();
                    let url_handle_clone = url_handle.clone();
                    let loading_handle_clone = loading_handle;
                    let presigned_url_clone = presigned_url;
                    let form_data_clone = form_data;

                    let closure = web_sys::wasm_bindgen::closure::Closure::wrap(Box::new(
                        move |_: web_sys::ProgressEvent| {
                            if reader_handle.result().is_ok() {
                                let url_setter = url_handle_clone.clone();
                                let loading_setter = loading_handle_clone.clone();
                                let url = presigned_url_clone.clone();
                                let form_data = form_data_clone.clone();

                                spawn_local(async move {
                                    let url_req = match url.try_into_request(form_data) {
                                        Ok(req) => req,
                                        Err(e) => {
                                            gloo::console::error!("Failed to create request:", e);
                                            loading_setter.set(false);
                                            return;
                                        }
                                    };

                                    match crate::browser_api::BrowserFetch::request::<UtUpload>(
                                        &url_req,
                                    )
                                    .await
                                    {
                                        Ok(upload_url) => {
                                            gloo::console::log!(
                                                "Upload successful, setting URL:",
                                                &upload_url.url
                                            );
                                            url_setter.set(Some(upload_url.url));
                                        }
                                        Err(e) => {
                                            gloo::console::error!("Upload failed:", e);
                                        }
                                    }
                                    loading_setter.set(false);
                                });
                            }
                        },
                    )
                        as Box<dyn FnMut(web_sys::ProgressEvent)>);

                    reader.set_onloadend(
                        Some(web_sys::wasm_bindgen::JsCast::unchecked_ref(closure.as_ref())
                    ));
                    if let Err(e) = reader.read_as_array_buffer(&file) {
                        gloo::console::error!("Failed to read file:", e);
                        return;
                    }
                    closure.forget();
                });
            }
        }
    });

    let user_keys = nostr_keys;
    let sender = relay_pool;
    let loading_handle = is_loading_new.clone();
    let onchange = Callback::from(move |e: yew::Event| {
        let loading_handle = loading_handle.clone();
        loading_handle.set(true);
        let user_keys = user_keys.clone();
        let sender = sender.clone();
        let server_pubkey = server_pubkey.clone();
        spawn_local(async move {
            let pubkey = user_keys.get_pubkey().await.unwrap_or_default();

            // Get the file from the input element
            let Some(input) = e.target().map(
                web_sys::wasm_bindgen::JsCast::unchecked_into::<HtmlInputElement>
            ) else {
                gloo::console::error!("Failed to get input element");
                loading_handle.set(false);
                return;
            };

            let Some(files) = input.files() else {
                gloo::console::error!("No files found");
                loading_handle.set(false);
                return;
            };

            let Some(file) = files.get(0) else {
                gloo::console::error!("No file selected");
                loading_handle.set(false);
                return;
            };

            // Create the upload request
            let file_req = upload_things::UtRequest::from(&file);

            // Log the request for debugging
            gloo::console::log!(
                "Sending upload request:",
                serde_json::to_string(&file_req).unwrap()
            );

            // Create and sign the request note
            let mut req_note = NostrNote {
                content: file_req.to_string(),
                kind: NOSTR_KIND_PRESIGNED_URL_REQ,
                pubkey: pubkey.clone(),
                ..Default::default()
            };

            match user_keys.sign_nostr_note(&mut req_note).await {
                Ok(note) => note,
                Err(e) => {
                    gloo::console::error!("Failed to sign request note:", e);
                    loading_handle.set(false);
                    return;
                }
            };

            // Create and sign the giftwrap note
            let mut giftwrap = NostrNote {
                content: req_note.to_string(),
                kind: NOSTR_KIND_SERVER_REQUEST,
                pubkey,
                ..Default::default()
            };

            match user_keys
                .sign_nip44(&mut giftwrap, server_pubkey.to_string())
                .await
            {
                Ok(note) => note,
                Err(e) => {
                    gloo::console::error!("Failed to sign giftwrap:", e);
                    loading_handle.set(false);
                    return;
                }
            };
            gloo::console::log!(
                "Sending giftwrap note:",
                serde_json::to_string(&giftwrap).unwrap()
            );

            // Send the note to the relay
            gloo::console::log!("Sending giftwrap to relay");
            sender.send(giftwrap);
        });
    });

    let mut default_classes = classes!(
        "flex",
        "items-center",
        "justify-center",
        "cursor-pointer",
        "border-4",
        "border-dashed",
        "border-blue-500",
        "rounded-xl"
    );
    default_classes.extend(classes);
    let mut with_url = default_classes.clone();
    with_url.extend(classes!("bg-transparent", "absolute"));
    url_clone.as_ref().map_or_else(
        || {
            html! {
                <label for={input_id.clone()} class={default_classes}>
                    <input onchange={&onchange} id={input_id.clone()} type="file" accept="image/*" class="hidden" />
                    {if *is_loading_new {
                        html! {
                            <div class="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
                    }} else {
                        html! {
                            <Plus class="w-8 h-8 text-blue-500" />
                    }   }}
                </label>
            }
        },
        |url|{
            image_classes.push("relative");
            html! {
                <label for={input_id.clone()} class={image_classes}>
                    <img src={url.clone()} class="absolute inset-0 size-full object-cover" />
                    <input onchange={&onchange} id={input_id.clone()} type="file" accept="image/*" class="hidden" />
                    {if *is_loading_new {
                        html! {
                            <div class="animate-spin rounded-full h-8 w-8 border-t-2 border-b-2 border-blue-500"></div>
                    }} else { html! {} }}
                </label>
            }})
}
#[function_component(ImageUploadTestComponent)]
pub fn image_upload_test_component() -> Html {
    let url_handle = use_state(|| None::<String>);

    // Try to get Nostr context
    let nostr_ctx = use_context::<crate::key_manager::NostrIdStore>().expect("No Nostr context found");
    if nostr_ctx.loaded() {
        return html! { <p>{"Loading Nostr identity..."}</p> };
    };
    let content = nostr_ctx.get_identity().map_or_else(
        || html! { 
            <p class="text-yellow-600">{"No Nostr identity available. Login first to test uploads."}</p> 
        },
        |identity| {
            // Prepare URL display component separately
            let url_display = (*url_handle).clone().map_or_else(
                || html! { <p>{"No image uploaded yet. Click the + to upload."}</p> },
                |url| {
                    html! {
                        <>
                            <p class="text-green-600 font-bold">{"Image Uploaded!"}</p>
                            <p class="break-all text-xs mt-2">{url}</p>
                            </>
                    }
                },
            );

            // We have an identity, render the component
            html! {
                <>
                    <p class="mb-4 text-green-600">{"Nostr identity loaded. You can test uploading."}</p>
                    <div class="flex items-center gap-4">
                        <ImageUploadInput
                            url_handle={url_handle.clone()}
                            nostr_keys={identity.clone()}
                            classes={classes!("w-32", "h-32")}
                            input_id={"test-upload-input"}
                            server_pubkey={crate::constants::TEST_PUB_KEY.to_string()}
                            image_classes={classes!("w-32", "h-32")}
                            />
                        <div>
                            {url_display}
                        </div>
                    </div>
                </>
            }
        });

    // Main component structure
    html! {
        <div class="p-4 border rounded">
            <h2 class="text-xl mb-4">{"Image Upload Component Test"}</h2>
            {content}
        </div>
    }
}
