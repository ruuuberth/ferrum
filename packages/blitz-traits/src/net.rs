//! Abstractions of networking so that custom networking implementations can be provided

/// A platform family with one self-consistent browser identity.
///
/// The `User-Agent` request header and `navigator.userAgent` come from the same
/// row, and `navigator.platform` / `navigator.oscpu` are derived from it as
/// well. Keeping them together matters: a page that sees an Android build
/// claiming to be desktop `X11; Linux x86_64`, or a Firefox version whose
/// `rv:` and `Firefox/` tokens disagree, has found a bot signal without any
/// fingerprinting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetOs {
    Android,
    Ios,
    MacOs,
    Windows,
    Linux,
}

impl TargetOs {
    /// `User-Agent` string for this platform. Firefox 81 is used everywhere so
    /// that the claimed feature set stays in step with what the engine can
    /// actually run; the tokens are the real ones for that release.
    pub const fn user_agent(self) -> &'static str {
        match self {
            // Mobile Firefox omits the `Gecko/20100101` build token.
            TargetOs::Android => {
                "Mozilla/5.0 (Android 11; Mobile; rv:81.0) Gecko/81.0 Firefox/81.0"
            }
            // Firefox for iOS is a WebKit shell, so it has an Apple UA.
            TargetOs::Ios => {
                "Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) FxiOS/81.0 Mobile/15E148 Safari/605.1.15"
            }
            TargetOs::MacOs => {
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:81.0) Gecko/20100101 Firefox/81.0"
            }
            TargetOs::Windows => {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:81.0) Gecko/20100101 Firefox/81.0"
            }
            TargetOs::Linux => {
                "Mozilla/5.0 (X11; Linux x86_64; rv:81.0) Gecko/20100101 Firefox/81.0"
            }
        }
    }

    /// `navigator.platform` for this platform.
    pub const fn platform(self) -> &'static str {
        match self {
            // This is what Firefox for Android reports, not "Android".
            TargetOs::Android => "Linux armv8l",
            TargetOs::Ios => "iPhone",
            TargetOs::MacOs => "MacIntel",
            TargetOs::Windows => "Win32",
            TargetOs::Linux => "Linux x86_64",
        }
    }

    /// `navigator.oscpu` for this platform. Firefox for iOS is WebKit, which
    /// does not expose the property at all.
    pub const fn oscpu(self) -> Option<&'static str> {
        match self {
            TargetOs::Android => Some("Linux armv8l"),
            TargetOs::Ios => None,
            TargetOs::MacOs => Some("Intel Mac OS X 10.15"),
            TargetOs::Windows => Some("Windows NT 10.0"),
            TargetOs::Linux => Some("Linux x86_64"),
        }
    }
}

#[cfg(target_os = "android")]
pub const TARGET_OS: TargetOs = TargetOs::Android;
#[cfg(target_os = "ios")]
pub const TARGET_OS: TargetOs = TargetOs::Ios;
#[cfg(target_os = "macos")]
pub const TARGET_OS: TargetOs = TargetOs::MacOs;
#[cfg(target_os = "windows")]
pub const TARGET_OS: TargetOs = TargetOs::Windows;
#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows"
)))]
pub const TARGET_OS: TargetOs = TargetOs::Linux;

/// The User-Agent Blitz sends with network requests and exposes to JavaScript
/// as `navigator.userAgent`.
pub const USER_AGENT: &str = TARGET_OS.user_agent();

pub use bytes::Bytes;
pub use http::{self, HeaderMap, Method};
use serde::{
    Serialize,
    ser::{SerializeSeq, SerializeTuple},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{ops::Deref, path::PathBuf};
pub use url::Url;

/// A type that fetches resources for a Document.
///
/// This may be over the network via http(s), via the filesystem, or some other method.
pub trait NetProvider: Send + Sync + 'static {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>);

    /// Whether this provider is a no-op (e.g. `DummyNetProvider`) that will never
    /// deliver resources. When true, callers must NOT register resources as
    /// "pending critical" — doing so blocks painting forever, since the
    /// completion callback never fires. Used by integrations that feed a
    /// pre-rendered DOM and perform no sub-fetches (e.g. aginxbrowser).
    fn is_noop(&self) -> bool {
        false
    }

    /// Store a cookie created by JavaScript (`document.cookie`) so that it is
    /// sent with subsequent HTTP requests. Providers without a cookie jar can
    /// leave the default implementation, which discards the cookie.
    fn set_cookie(&self, _url: &Url, _cookie: &str) {}
}

/// A type that parses raw bytes from a network request into a Data and then calls
/// the NetCallack with the result.
pub trait NetHandler: Send + Sync + 'static {
    fn bytes(self: Box<Self>, resolved_url: String, bytes: Bytes);

    /// Called when fetching fails, including HTTP error statuses and cancellation.
    fn error(self: Box<Self>, _url: String, _error: String) {}
}

/// A callback which gets called every time a network request completes
// Q: Should we use std::task::Waker for this?
pub trait NetWaker: Send + Sync + 'static {
    fn wake(&self, client_id: usize);
}

impl<F: Fn(usize) + Send + Sync + 'static> NetWaker for F {
    fn wake(&self, doc_id: usize) {
        self(doc_id)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
/// A request type loosely representing <https://fetch.spec.whatwg.org/#requests>
pub struct Request {
    pub url: Url,
    pub method: Method,
    pub content_type: Option<String>,
    pub headers: HeaderMap,
    pub body: Body,
    pub signal: Option<AbortSignal>,
}
impl Request {
    /// A get request to the specified Url and an empty body
    pub fn get(url: Url) -> Self {
        Self {
            url,
            method: Method::GET,
            content_type: None,
            headers: HeaderMap::new(),
            body: Body::Empty,
            signal: None,
        }
    }

    pub fn signal(mut self, signal: AbortSignal) -> Self {
        self.signal = Some(signal);
        self
    }
}

#[derive(Debug, Clone)]
pub enum Body {
    Bytes(Bytes),
    Form(FormData),
    Empty,
}

/// A list of form entries used for form submission
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FormData(pub Vec<Entry>);
impl FormData {
    /// Creates a new empty FormData
    pub fn new() -> Self {
        FormData(Vec::new())
    }
}
impl Serialize for FormData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq_serializer = serializer.serialize_seq(Some(self.len()))?;
        for entry in &self.0 {
            seq_serializer.serialize_element(entry)?;
        }
        seq_serializer.end()
    }
}
impl Deref for FormData {
    type Target = Vec<Entry>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A single form entry consisting of a name and value
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub value: EntryValue,
}
impl Serialize for Entry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_tuple(2)?;
        serializer.serialize_element(&self.name)?;
        match &self.value {
            EntryValue::String(s) => serializer.serialize_element(s)?,
            EntryValue::File(p) => serializer.serialize_element(p.to_str().unwrap_or_default())?,
            EntryValue::EmptyFile => serializer.serialize_element("")?,
        }
        serializer.end()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryValue {
    String(String),
    File(PathBuf),
    EmptyFile,
}
impl AsRef<str> for EntryValue {
    fn as_ref(&self) -> &str {
        match self {
            EntryValue::String(s) => s,
            EntryValue::File(p) => p.to_str().unwrap_or_default(),
            EntryValue::EmptyFile => "",
        }
    }
}

impl From<&str> for EntryValue {
    fn from(value: &str) -> Self {
        EntryValue::String(value.to_string())
    }
}
impl From<PathBuf> for EntryValue {
    fn from(value: PathBuf) -> Self {
        EntryValue::File(value)
    }
}

/// A default noop NetProvider
#[derive(Default)]
pub struct DummyNetProvider;
impl NetProvider for DummyNetProvider {
    fn fetch(&self, _doc_id: usize, _request: Request, _handler: Box<dyn NetHandler>) {}
    fn is_noop(&self) -> bool {
        true
    }
}

/// The AbortController interface represents a controller object that
/// allows you to abort one or more Web requests as and when desired.
///
/// <https://developer.mozilla.org/en-US/docs/Web/API/AbortController>
#[derive(Debug, Default)]
pub struct AbortController {
    pub signal: AbortSignal,
}

impl AbortController {
    /// The abort() method of the AbortController interface aborts
    /// an asynchronous operation before it has completed.
    /// This is able to abort fetch requests.
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/API/AbortController/abort>
    pub fn abort(self) {
        self.signal.0.store(true, Ordering::SeqCst);
    }
}

/// The AbortSignal interface represents a signal object that allows you to
/// communicate with an asynchronous operation (such as a fetch request) and
/// abort it if required via an AbortController object.
///
/// <https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal>
#[derive(Debug, Default, Clone)]
pub struct AbortSignal(Arc<AtomicBool>);

impl AbortSignal {
    /// The aborted read-only property returns a value that indicates whether
    /// the asynchronous operations the signal is communicating with are
    /// aborted (true) or not (false).
    ///
    /// <https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal/aborted>
    pub fn aborted(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
