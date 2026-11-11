//! Temporary diagnostic: run the saved Google "enable JavaScript" page
//! through the engine and report which APIs it reaches. Not meant to be
//! committed.

use std::sync::{Arc, Mutex};

use blitz_dom::{Document, DocumentConfig};
use blitz_traits::navigation::{NavigationOptions, NavigationProvider};
use blitz_traits::net::{NetHandler, NetProvider, Request, Url};
use blitz_vibey_script::ScriptDocument;

#[derive(Default)]
struct Nav {
    urls: Arc<Mutex<Vec<String>>>,
}

impl NavigationProvider for Nav {
    fn navigate_to(&self, options: NavigationOptions) {
        self.urls.lock().unwrap().push(options.url.to_string());
    }
}

#[derive(Default)]
struct Net {
    cookies: Arc<Mutex<Vec<String>>>,
}

impl NetProvider for Net {
    fn fetch(&self, _: usize, _: Request, _: Box<dyn NetHandler>) {}

    fn set_cookie(&self, _: &Url, cookie: &str) {
        self.cookies.lock().unwrap().push(cookie.to_string());
    }
}

#[test]
fn probe_google_stub() {
    let path = std::env::var("GOOGLE_STUB").unwrap_or_else(|_| "stub.html".to_string());
    let html = std::fs::read_to_string(&path).expect("read challenge HTML");
    let nav = Nav::default();
    let net = Net::default();
    let urls = Arc::clone(&nav.urls);
    let cookies = Arc::clone(&net.cookies);
    let mut doc = ScriptDocument::from_html(
        &html,
        DocumentConfig {
            base_url: Some("https://www.google.com/search?q=rust&hl=es".to_string()),
            navigation_provider: Some(Arc::new(nav)),
            net_provider: Some(Arc::new(net)),
            ..Default::default()
        },
    )
    .with_virtual_time();
    eprintln!("[probe] execute_scripts");
    doc.execute_scripts();
    eprintln!("[probe] execute_scripts done");

    for _ in 0..100 {
        let Some(deadline) = doc.next_timer_deadline() else {
            eprintln!("[probe] no more timers");
            break;
        };
        let now = doc.clock_now();
        doc.advance_clock_to(deadline.max(now));
        doc.poll(None);
    }
    eprintln!("[probe] timer loop done");

    eprintln!("NAV: {:?}", urls.lock().unwrap());
    eprintln!("COOKIES: {:?}", cookies.lock().unwrap());
    eprintln!("ERRORS: {:#?}", doc.take_js_errors());
}
