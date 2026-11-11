//! Global registration and the ordered JavaScript bootstrap.

use blitz_traits::net::USER_AGENT;
use boa_engine::object::ObjectInitializer;
use boa_engine::object::builtins::JsArray;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction, js_string};
use url::Url;

use super::{css, encoding, events, fetch, storage, timers, window};
use crate::dom::node_wrapper;
use crate::state::DomCtx;

pub(super) const GEOMETRY_JS: &str = include_str!("../geometry.js");

// Concatenate into one IIFE to preserve shared lexical bindings, installation
// order, and short-circuiting on errors. Geometry remains a separate evaluation.
pub(super) const BOOTSTRAP_JS: &str = concat!(
    "\n(function () {\n",
    include_str!("bootstrap/url_search_params.js"),
    "\n",
    include_str!("bootstrap/window.js"),
    "\n",
    include_str!("bootstrap/storage.js"),
    "\n",
    include_str!("bootstrap/compat.js"),
    "\n",
    include_str!("bootstrap/fetch.js"),
    "\n",
    include_str!("bootstrap/dom_exception.js"),
    "\n",
    include_str!("bootstrap/style.js"),
    "\n",
    include_str!("bootstrap/interfaces.js"),
    "\n",
    include_str!("bootstrap/css.js"),
    "\n",
    include_str!("bootstrap/stylesheets.js"),
    "\n",
    include_str!("bootstrap/fonts.js"),
    "\n",
    include_str!("bootstrap/node_constants.js"),
    "})();\n",
);

pub(super) fn register_globals(ctx: &DomCtx, base_url: Option<&Url>, context: &mut Context) {
    register_global_fn(
        context,
        "__blitz_parse_search_params",
        1,
        parse_search_params,
    );
    // HTML base64 helpers. Boa does not provide them and real pages (including
    // Google's botguard challenge) call `atob`/`btoa` directly.
    register_global_fn(context, "atob", 1, encoding::atob);
    register_global_fn(context, "btoa", 1, encoding::btoa);
    // `document`
    let root_id = ctx.doc.borrow().root_node().id;
    let document_wrapper = node_wrapper(ctx, root_id, context);
    register_global(context, "document", document_wrapper.into());

    // `window` and friends (aliases for the global object)
    let global: JsValue = context.global_object().into();
    register_global(context, "window", global.clone());
    register_global(context, "self", global.clone());
    // There is only ever a single frame, so `parent` and `top` refer to the
    // window itself and `opener` is null
    register_global(context, "parent", global.clone());
    register_global(context, "top", global);
    register_global(context, "opener", JsValue::null());

    // `location`
    let location = window::build_location(base_url, context);
    register_global(context, "location", location);

    // `navigator`
    let languages = JsArray::from_iter(
        [
            JsValue::from(JsString::from("es-ES")),
            JsValue::from(JsString::from("es")),
        ],
        context,
    );
    let hardware_concurrency = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);
    let max_touch_points = if cfg!(any(target_os = "android", target_os = "ios")) {
        5
    } else {
        0
    };
    let navigator = ObjectInitializer::new(context)
        .property(
            js_string!("userAgent"),
            JsString::from(USER_AGENT),
            Attribute::all(),
        )
        .property(
            js_string!("appName"),
            js_string!("Netscape"),
            Attribute::all(),
        )
        .property(
            js_string!("appCodeName"),
            js_string!("Mozilla"),
            Attribute::all(),
        )
        .property(js_string!("product"), js_string!("Gecko"), Attribute::all())
        .property(
            js_string!("language"),
            js_string!("es-ES"),
            Attribute::all(),
        )
        .property(js_string!("languages"), languages, Attribute::all())
        .property(
            js_string!("platform"),
            JsString::from(navigator_platform()),
            Attribute::all(),
        )
        .property(js_string!("vendor"), js_string!(""), Attribute::all())
        .property(js_string!("cookieEnabled"), true, Attribute::all())
        .property(js_string!("onLine"), true, Attribute::all())
        .property(js_string!("webdriver"), false, Attribute::all())
        .property(
            js_string!("hardwareConcurrency"),
            JsValue::from(hardware_concurrency as f64),
            Attribute::all(),
        )
        .property(
            js_string!("maxTouchPoints"),
            JsValue::from(max_touch_points as f64),
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(storage::send_beacon),
            js_string!("sendBeacon"),
            2,
        )
        .build();
    register_global(context, "navigator", navigator.into());

    // Web Storage, `performance`, `matchMedia`, `screen` and
    // `requestIdleCallback` natives used by the JS bootstrap.
    register_global_fn(context, "__blitz_storage_get", 2, storage::storage_get);
    register_global_fn(context, "__blitz_storage_set", 3, storage::storage_set);
    register_global_fn(
        context,
        "__blitz_storage_remove",
        2,
        storage::storage_remove,
    );
    register_global_fn(context, "__blitz_storage_clear", 1, storage::storage_clear);
    register_global_fn(context, "__blitz_storage_keys", 1, storage::storage_keys);
    register_global_fn(
        context,
        "__blitz_performance_now",
        0,
        storage::performance_now,
    );
    register_global_fn(
        context,
        "__blitz_performance_time_origin",
        0,
        storage::performance_time_origin,
    );

    // Timers and window event listeners
    register_global_fn(context, "setTimeout", 2, timers::set_timeout);
    register_global_fn(context, "clearTimeout", 1, timers::clear_timer);
    register_global_fn(context, "setInterval", 2, timers::set_interval);
    register_global_fn(context, "clearInterval", 1, timers::clear_timer);
    register_global_fn(
        context,
        "requestAnimationFrame",
        1,
        timers::request_animation_frame,
    );
    register_global_fn(context, "cancelAnimationFrame", 1, timers::clear_timer);
    register_global_fn(
        context,
        "addEventListener",
        2,
        events::window_add_event_listener,
    );
    register_global_fn(
        context,
        "removeEventListener",
        2,
        events::window_remove_event_listener,
    );

    // Embedder message channel (see `ScriptDocument::take_messages`)
    register_global_fn(context, "__blitz_send_message", 1, window::send_message);

    // Synchronous resource fetch backing the bootstrap's `fetch()`
    register_global_fn(context, "__blitz_fetch_sync", 1, fetch::fetch_sync);

    // CSS property support check, used by the style Proxy's `has` trap
    register_global_fn(
        context,
        "__blitz_css_property_names",
        0,
        css::css_property_names,
    );
    register_global_fn(
        context,
        "__blitz_css_property_supported",
        1,
        css::css_property_supported,
    );

    // `getComputedStyle`
    register_global_fn(context, "getComputedStyle", 1, css::get_computed_style);
    register_global_fn(
        context,
        "getSelection",
        0,
        crate::dom::selection::get_selection,
    );
    register_global_fn(context, "__blitz_parse_transform", 1, css::parse_transform);

    // CSSOM stylesheet natives (`__blitz_sheet_*`), used by the bootstrap's
    // `CSSStyleSheet` / `CSSRule` implementation
    crate::dom::stylesheet::register(context);

    // Viewport dimensions
    register_global_accessor(context, "innerWidth", window::inner_width);
    register_global_accessor(context, "innerHeight", window::inner_height);
    register_global_accessor(context, "outerWidth", window::inner_width);
    register_global_accessor(context, "outerHeight", window::inner_height);
    register_global_accessor(context, "devicePixelRatio", window::device_pixel_ratio);

    // Viewport scrolling
    register_global_accessor(context, "scrollX", window::scroll_x);
    register_global_accessor(context, "scrollY", window::scroll_y);
    register_global_accessor(context, "pageXOffset", window::scroll_x);
    register_global_accessor(context, "pageYOffset", window::scroll_y);
    register_global_fn(context, "scroll", 2, window::window_scroll_to);
    register_global_fn(context, "scrollTo", 2, window::window_scroll_to);
    register_global_fn(context, "scrollBy", 2, window::window_scroll_by);

    // The `CSS` namespace object. `CSS.escape` is defined in the JS bootstrap.
    let css_namespace = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(css::css_supports),
            js_string!("supports"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(css::css_register_property),
            js_string!("registerProperty"),
            1,
        )
        .build();
    register_global(context, "CSS", css_namespace.into());
}

fn parse_search_params(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    use boa_engine::object::builtins::JsArray;

    let input = args[0].to_string(context)?.to_std_string_lossy();
    let pairs = url::form_urlencoded::parse(input.as_bytes())
        .map(|(name, value)| {
            JsArray::from_iter(
                [
                    JsValue::from(JsString::from(name.as_ref())),
                    JsValue::from(JsString::from(value.as_ref())),
                ],
                context,
            )
            .into()
        })
        .collect::<Vec<JsValue>>();
    Ok(JsArray::from_iter(pairs, context).into())
}

fn navigator_platform() -> &'static str {
    if cfg!(target_os = "android") {
        "Linux armv8l"
    } else if cfg!(target_os = "ios") {
        "iPhone"
    } else if cfg!(target_os = "macos") {
        "MacIntel"
    } else if cfg!(target_os = "windows") {
        "Win32"
    } else {
        "Linux x86_64"
    }
}

fn register_global(context: &mut Context, name: &str, value: JsValue) {
    context
        .register_global_property(
            JsString::from(name),
            value,
            Attribute::WRITABLE.union(Attribute::CONFIGURABLE),
        )
        .expect("failed to register global");
}

fn register_global_accessor(
    context: &mut Context,
    name: &str,
    getter: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    use boa_engine::object::FunctionObjectBuilder;
    use boa_engine::property::{PropertyDescriptor, PropertyKey};

    let getter_fn =
        FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(getter))
            .name(JsString::from(format!("get {name}")))
            .length(0)
            .build();
    context
        .global_object()
        .define_property_or_throw(
            PropertyKey::from(JsString::from(name)),
            PropertyDescriptor::builder()
                .get(getter_fn)
                .enumerable(false)
                .configurable(true)
                .build(),
            context,
        )
        .expect("failed to register global accessor");
}

fn register_global_fn(
    context: &mut Context,
    name: &str,
    length: usize,
    body: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>,
) {
    context
        .register_global_callable(
            JsString::from(name),
            length,
            NativeFunction::from_fn_ptr(body),
        )
        .expect("failed to register global function");
}
