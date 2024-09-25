use yew::prelude::*;
use yew_router::prelude::*;


#[derive(Clone, Routable, PartialEq)]
pub enum AppRoute {
    #[at("/")]
    Home,
    #[at("/leaflet")]
    Leaflet,
    #[at("/fullcalendar")]
    FullCalendar,
    #[at("/chartjs")]
    ChartJs,
    #[at("/draggable")]
    Draggable,
    #[at("/toastify")]
    Toastify,
}

#[function_component(ConsumerPages)]
pub fn consumer_pages() -> Html {
    html! {
        <Switch<AppRoute> render = { move |switch: AppRoute| {
                match switch {
                AppRoute::Home => html!{<></>},
                AppRoute::Leaflet => html!{<></>},
                AppRoute::FullCalendar => html!{<></>},
                AppRoute::ChartJs => html!{<></>},
                AppRoute::Draggable => html!{<></>},
                AppRoute::Toastify => html!{<></>},
                }
            }}
        />
    }
}
