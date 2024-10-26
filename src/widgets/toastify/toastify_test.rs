use yew::prelude::*;
use super::ToastifyOptions;

#[function_component(ToastifyTest)]
pub fn toastify_test() -> Html {
    use_effect_with((), move |_| {
        let timeout = 6000;
        gloo_timers::callback::Interval::new(timeout, move || {
            yew::platform::spawn_local(async move {
                let login_options = ToastifyOptions::new_login("Login Notification".to_string());
                let logout_options = ToastifyOptions::new_relay_connected("Relay Connected");
                let options = ToastifyOptions::new_success("Success Notification");
                let options2 = ToastifyOptions::new_relay_error("Error Notification");
                let options3 = ToastifyOptions::new_relay_disconnected("Relay Disconnected");
                let options4 = ToastifyOptions::new_event_received("Event Received");
                let interval = std::time::Duration::from_secs(1);
                
                login_options.clone().show();
                gloo_timers::future::sleep(interval).await;
                logout_options.clone().show();
                options.clone().show();
                gloo_timers::future::sleep(interval).await;
                options2.clone().show();
                gloo_timers::future::sleep(interval).await;
                options3.clone().show();
                gloo_timers::future::sleep(interval).await;
                options4.clone().show();
                gloo_timers::future::sleep(interval).await;
            });
        })
        .forget();
        || {}
    });
    
    html! {
        <div class="p-4">
            <h1 class="text-2xl font-bold mb-2">{"Toastify Test"}</h1>
            <p class="text-gray-600">{"Notifications will play on a loop"}</p>
        </div>
    }
}