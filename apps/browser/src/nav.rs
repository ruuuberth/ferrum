use blitz_traits::navigation::NavigationOptions;
use blitz_traits::net::{Method, Request, Url};
use dioxus_native::prelude::{Key, NamedKey};

use crate::settings::SearchEngine;

pub fn req_from_string(url_s: &str, engine: SearchEngine) -> Option<Request> {
    if let Ok(url) = Url::parse(url_s) {
        return Some(Request::get(url));
    };

    let contains_space = url_s.contains(' ');
    let contains_dot = url_s.contains('.');
    if contains_dot && !contains_space {
        if let Ok(url) = Url::parse(&format!("https://{}", url_s)) {
            return Some(Request::get(url));
        }
    }

    Some(synthesize_search_req(engine, url_s))
}

fn synthesize_search_req(engine: SearchEngine, query: &str) -> Request {
    let url = engine.search_url(query);

    NavigationOptions::new(
        url,
        Some(String::from("application/x-www-form-urlencoded")),
        0,
    )
    .set_method(Method::GET)
    .into_request()
}

pub fn open_in_external_browser(req: &Request) {
    if req.method == Method::GET && matches!(req.url.scheme(), "http" | "https" | "mailto") {
        if let Err(err) = webbrowser::open(req.url.as_str()) {
            tracing::error!("Failed to open URL: {}", err);
        }
    }
}

pub fn is_enter_key(key: &Key) -> bool {
    matches!(key, Key::Named(NamedKey::Enter)) || matches!(key, Key::Character(s) if s == "\n")
}
