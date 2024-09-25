use minions::browser_api::service_worker::AppServiceWorker;
use yew::prelude::*;

fn main() {
    yew::Renderer::<App>::new().render();
}

#[function_component(App)]
fn app() -> Html {
    use_effect_with((), move |_| {
        yew::platform::spawn_local(async move {
            AppServiceWorker::new()
                .expect("No SW on this platform")
                .install("serviceWorker.js")
                .await
                .expect("SW File not found");
        });
        || {}
    });
    html! {
        <div>
            <h1>{"Hello, Yew!"}</h1>
            <p>{"This is a simple Yew app."}</p>
        </div>
    }
}
