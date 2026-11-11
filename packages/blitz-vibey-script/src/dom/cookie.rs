//! `document.cookie`: a small in-memory cookie jar.
//!
//! This implements the parts of the cookie model scripts rely on: name/value
//! pairs, `Path`, `Domain`, `Max-Age`, `Secure` and `HttpOnly`. Cookies created
//! from JavaScript are never `HttpOnly` (the browser ignores that attribute on
//! script-set cookies).
//!
//! The jar lives in the document's runtime state. HTTP cookies are still owned
//! by the network layer; bridging the two is left to the embedder.

use boa_engine::{Context, JsResult, JsString, JsValue};
use web_time::{Duration, SystemTime};

use super::{dom_ctx, this_node_id};
use crate::state::Cookie;

/// `document.cookie` getter: the cookies visible to the current URL.
pub(crate) fn get_cookie(
    this: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    this_node_id(this)?;
    let ctx = dom_ctx(context)?;
    let Some(url) = ctx.state.borrow().base_url.clone() else {
        return Ok(JsString::from("").into());
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = url.path();
    let secure = url.scheme() == "https";
    let now = SystemTime::now();

    let state = ctx.state.borrow();
    let mut parts = Vec::new();
    for cookie in &state.cookies {
        if cookie.http_only || (cookie.secure && !secure) {
            continue;
        }
        if cookie.expires.is_some_and(|expires| expires <= now) {
            continue;
        }
        if cookie.host_only {
            if host != cookie.domain {
                continue;
            }
        } else if !domain_match(&host, &cookie.domain) {
            continue;
        }
        if !path_match(path, &cookie.path) {
            continue;
        }
        parts.push(format!("{}={}", cookie.name, cookie.value));
    }
    Ok(JsString::from(parts.join("; ")).into())
}

/// `document.cookie` setter: parse and store one `Set-Cookie`-style string.
pub(crate) fn set_cookie(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    this_node_id(this)?;
    let input = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    let ctx = dom_ctx(context)?;
    let Some(url) = ctx.state.borrow().base_url.clone() else {
        return Ok(JsValue::undefined());
    };
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Ok(JsValue::undefined());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.is_empty() {
        return Ok(JsValue::undefined());
    }

    let mut segments = input.split(';');
    let Some(pair) = segments.next() else {
        return Ok(JsValue::undefined());
    };
    let Some((name, value)) = pair.split_once('=') else {
        return Ok(JsValue::undefined());
    };
    let name = name.trim();
    let value = value.trim();
    if !valid_name(name) || !valid_value(value) {
        return Ok(JsValue::undefined());
    }

    let mut domain = host.clone();
    let mut host_only = true;
    let mut path = default_path(url.path());
    let mut secure = false;
    let mut expires = None;
    let mut delete = false;

    for segment in segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let (key, attribute) = match segment.split_once('=') {
            Some((key, value)) => (key, value),
            None => (segment, ""),
        };
        match key.trim().to_ascii_lowercase().as_str() {
            "domain" => {
                let candidate = attribute
                    .trim()
                    .trim_start_matches('.')
                    .to_ascii_lowercase();
                if candidate.is_empty() || !domain_match(&host, &candidate) {
                    return Ok(JsValue::undefined());
                }
                domain = candidate;
                host_only = false;
            }
            "path" => {
                let candidate = attribute.trim();
                if candidate.starts_with('/') {
                    path = candidate.to_string();
                }
            }
            "max-age" => {
                if let Ok(seconds) = attribute.trim().parse::<i64>() {
                    if seconds <= 0 {
                        delete = true;
                    } else {
                        expires = Some(SystemTime::now() + Duration::from_secs(seconds as u64));
                    }
                }
            }
            "secure" => secure = true,
            _ => {}
        }
    }

    if secure && scheme != "https" {
        return Ok(JsValue::undefined());
    }

    // Mirror the cookie into the network layer's jar. It is the same jar
    // reqwest uses for `Set-Cookie`, so it is sent with the next request and
    // survives navigation away from this document.
    ctx.doc
        .borrow()
        .net_provider
        .set_cookie(&url, input.as_str());

    let mut state = ctx.state.borrow_mut();
    state
        .cookies
        .retain(|cookie| !(cookie.name == name && cookie.domain == domain && cookie.path == path));
    if delete {
        return Ok(JsValue::undefined());
    }
    state.cookies.push(Cookie {
        name: name.to_string(),
        value: value.to_string(),
        domain,
        path,
        secure,
        http_only: false,
        host_only,
        expires,
    });
    Ok(JsValue::undefined())
}

/// Whether `domain` is equal to, or a parent domain of, `host`.
fn domain_match(host: &str, domain: &str) -> bool {
    host == domain
        || (host.len() > domain.len()
            && host.ends_with(domain)
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
}

/// The default cookie path for a request path: its directory.
fn default_path(path: &str) -> String {
    if !path.starts_with('/') {
        return "/".to_string();
    }
    match path.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(index) => path[..index].to_string(),
    }
}

/// RFC 6265 path matching.
fn path_match(request_path: &str, cookie_path: &str) -> bool {
    if request_path == cookie_path {
        return true;
    }
    if !request_path.starts_with(cookie_path) {
        return false;
    }
    cookie_path.ends_with('/') || request_path.as_bytes().get(cookie_path.len()) == Some(&b'/')
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.bytes().any(|byte| {
            byte <= 0x20
                || byte >= 0x7f
                || matches!(
                    byte,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                        | b'{'
                        | b'}'
                )
        })
}

fn valid_value(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte < 0x20 || byte == 0x7f || byte == b';' || byte == b',')
}
