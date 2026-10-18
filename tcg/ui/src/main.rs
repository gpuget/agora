mod api;
mod app;
mod components;
mod format;

use app::App;

fn main() {
    yew::Renderer::<App>::new().render();
}
