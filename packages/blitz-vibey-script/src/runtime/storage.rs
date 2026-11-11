//! Web Storage (`localStorage` / `sessionStorage`), `navigator.sendBeacon`
//! and `performance` natives.

use boa_engine::object::builtins::JsArray;
use boa_engine::{Context, JsNativeError, JsResult, JsString, JsValue};
use web_time::{SystemTime, UNIX_EPOCH};

use crate::dom::dom_ctx;

fn storage_name(args: &[JsValue], context: &mut Context) -> JsResult<String> {
    let name = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    if name == "local" || name == "session" {
        Ok(name)
    } else {
        Err(JsNativeError::typ()
            .with_message("unknown storage area")
            .into())
    }
}

fn storage_key(args: &[JsValue], context: &mut Context) -> JsResult<String> {
    Ok(args
        .get(1)
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy())
}

/// `__blitz_storage_get(area, key)`: the stored string, or `null`.
pub(super) fn storage_get(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let area = storage_name(args, context)?;
    let key = storage_key(args, context)?;
    let ctx = dom_ctx(context)?;
    let state = ctx.state.borrow();
    let map = match area.as_str() {
        "local" => &state.local_storage,
        _ => &state.session_storage,
    };
    Ok(match map.get(&key) {
        Some(value) => JsValue::from(JsString::from(value.as_str())),
        None => JsValue::null(),
    })
}

/// `__blitz_storage_set(area, key, value)`
pub(super) fn storage_set(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let area = storage_name(args, context)?;
    let key = storage_key(args, context)?;
    let value = args
        .get(2)
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    let ctx = dom_ctx(context)?;
    let mut state = ctx.state.borrow_mut();
    let map = match area.as_str() {
        "local" => &mut state.local_storage,
        _ => &mut state.session_storage,
    };
    map.insert(key, value);
    Ok(JsValue::undefined())
}

/// `__blitz_storage_remove(area, key)`
pub(super) fn storage_remove(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let area = storage_name(args, context)?;
    let key = storage_key(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut state = ctx.state.borrow_mut();
    let map = match area.as_str() {
        "local" => &mut state.local_storage,
        _ => &mut state.session_storage,
    };
    map.remove(&key);
    Ok(JsValue::undefined())
}

/// `__blitz_storage_clear(area)`
pub(super) fn storage_clear(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let area = storage_name(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut state = ctx.state.borrow_mut();
    let map = match area.as_str() {
        "local" => &mut state.local_storage,
        _ => &mut state.session_storage,
    };
    map.clear();
    Ok(JsValue::undefined())
}

/// `__blitz_storage_keys(area)`: the stored keys, in a stable order.
pub(super) fn storage_keys(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let area = storage_name(args, context)?;
    let ctx = dom_ctx(context)?;
    let state = ctx.state.borrow();
    let map = match area.as_str() {
        "local" => &state.local_storage,
        _ => &state.session_storage,
    };
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let keys: Vec<JsValue> = keys
        .into_iter()
        .map(|key| JsValue::from(JsString::from(key.as_str())))
        .collect();
    Ok(JsArray::from_iter(keys, context).into())
}

/// `navigator.sendBeacon`: telemetry only. Returning `true` matches a browser
/// which accepted the request into its queue; Blitz has no background request
/// queue, so the data is intentionally discarded.
pub(super) fn send_beacon(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(true))
}

/// `__blitz_performance_now()`: milliseconds since the document started.
pub(super) fn performance_now(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let state = ctx.state.borrow();
    let start = state
        .performance_start
        .unwrap_or_else(web_time::Instant::now);
    let elapsed = state.clock.now().saturating_duration_since(start);
    Ok(JsValue::from(elapsed.as_secs_f64() * 1000.0))
}

/// `__blitz_performance_time_origin()`: Unix epoch milliseconds.
pub(super) fn performance_time_origin(
    _: &JsValue,
    _: &[JsValue],
    _: &mut Context,
) -> JsResult<JsValue> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok(JsValue::from(elapsed.as_secs_f64() * 1000.0))
}
