use gloo_net::http::Request;
use shared::Health;
use yew::prelude::*;

#[component]
fn App() -> Html {
    let status = use_state(|| "loading...".to_string());

    {
        let status = status.clone();
        use_effect_with((), move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                let text = match Request::get("/api/health").send().await {
                    Ok(resp) => match resp.json::<Health>().await {
                        Ok(h) => h.status,
                        Err(e) => format!("bad response: {e}"),
                    },
                    Err(e) => format!("request failed: {e}"),
                };
                status.set(text);
            });
        });
    }

    html! {
        <main>
            <h1>{ "Agora" }</h1>
            <p>{ format!("backend: {}", *status) }</p>
        </main>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
