//! `atob` / `btoa`: the HTML base64 helpers used by many scripts.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use boa_engine::{Context, JsNativeError, JsResult, JsString, JsValue};

/// `atob(data)`: decode base64 into a binary (Latin-1) string.
pub(super) fn atob(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let input = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    // The HTML spec ignores ASCII whitespace anywhere in the input.
    let compact: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = STANDARD.decode(compact.as_bytes()).map_err(|_| {
        JsNativeError::typ().with_message("The string to be decoded is not correctly encoded.")
    })?;
    let decoded: String = bytes.into_iter().map(char::from).collect();
    Ok(JsString::from(decoded).into())
}

/// `btoa(data)`: encode a binary (Latin-1) string as base64.
pub(super) fn btoa(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let input = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    let mut bytes = Vec::with_capacity(input.len());
    for ch in input.chars() {
        let code = ch as u32;
        if code > u8::MAX as u32 {
            return Err(JsNativeError::typ()
                .with_message(
                    "The string to be encoded contains characters outside of the Latin1 range.",
                )
                .into());
        }
        bytes.push(code as u8);
    }
    Ok(JsString::from(STANDARD.encode(bytes)).into())
}
