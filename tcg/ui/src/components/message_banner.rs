use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MessageBannerProps {
    /// `(is_success, text)`.
    pub message: Option<(bool, String)>,
}

#[component]
pub fn MessageBanner(props: &MessageBannerProps) -> Html {
    let Some((ok, text)) = props.message.clone() else {
        return html! {};
    };
    html! {
        <p class={if ok { "message ok" } else { "message error" }}>{ text }</p>
    }
}
