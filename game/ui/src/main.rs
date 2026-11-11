//! Web ui of the demo game: a Yew app compiled to WebAssembly and built with Trunk.
//!
//! The ui calls the game server only through relative `/api/` URLs, with the types of
//! `game-dto`. `Trunk.toml` serves the app on port 3000 and proxies `/api/` to the server
//! at `127.0.0.1:8080`. The catalog and the houses load once at startup. The players, the
//! money taken out of the game and the listings of the selected house reload every second.
//! There is no login: the ui acts as the player picked in its header.

mod api;
mod app;
mod components;
mod format;

use app::App;

fn main() {
    yew::Renderer::<App>::new().render();
}
