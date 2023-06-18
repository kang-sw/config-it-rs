#![allow(non_snake_case)]
// import the prelude to get access to the `rsx!` macro and the `Scope` and `Element` types
use dioxus::prelude::*;

fn main() {
    // launch the web app
    dioxus_web::launch(App);
}

pub(crate) mod base {
    use lazy_static::lazy_static;

    pub fn get_base_url() -> &'static str {
        lazy_static! {
            static ref BASE_URL: String = web_sys::window().unwrap().location().origin().unwrap();
        }

        &BASE_URL
    }

    #[macro_export]
    macro_rules! base_url {
        ($($args:tt)*) => {
            format!("{}{}", crate::base::get_base_url(), format_args!($($args)*))
        };
    }
}

fn App(cx: Scope) -> Element {
    use_future!(cx, || async move {
        reqwest::Client::new()
            .get(base_url!("/api/test"))
            .send()
            .await
            .unwrap();
    });

    cx.render(rsx! {
        div { class: "w-full h-screen bg-gray-300 flex items-center justify-center", "Hello, world!" }
    })
}
