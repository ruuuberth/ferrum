//! Window location, named elements, viewport, scrolling, and embedder messages.

use blitz_dom::NodeId;
use blitz_traits::navigation::NavigationOptions;
use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsResult, JsString, JsValue, NativeFunction, js_string};
use url::Url;

use super::ScriptRuntime;
use crate::dom::{dom_ctx, node_wrapper};

impl ScriptRuntime {
    /// HTML named element access: expose elements with an `id` attribute as
    /// properties of the global (`window`) object, without overriding existing
    /// globals. Should be called before evaluating scripts so that newly
    /// parsed/created ids are visible.
    pub fn sync_named_element_globals(&mut self) {
        use boa_engine::property::{PropertyDescriptor, PropertyKey};

        let ids: Vec<(String, NodeId)> = {
            let doc = self.ctx.doc.borrow();
            let mut ids = Vec::new();
            let mut stack = vec![doc.root_node().id];
            while let Some(node_id) = stack.pop() {
                let Some(node) = doc.get_node(node_id) else {
                    continue;
                };
                if let Some(id) = node
                    .element_data()
                    .and_then(|element| element.attr(blitz_dom::local_name!("id")))
                {
                    if !id.is_empty() {
                        ids.push((id.to_string(), node_id));
                    }
                }
                stack.extend(node.children.iter().rev().copied());
            }
            ids
        };

        let global = self.context.global_object();
        for (id, node_id) in ids {
            let key = PropertyKey::from(JsString::from(id));
            let already_defined = global
                .has_own_property(key.clone(), &mut self.context)
                .unwrap_or(true);
            if already_defined {
                continue;
            }
            let wrapper = node_wrapper(&self.ctx, node_id, &mut self.context);
            let _ = global.define_property_or_throw(
                key,
                PropertyDescriptor::builder()
                    .value(wrapper)
                    .writable(true)
                    .enumerable(false)
                    .configurable(true)
                    .build(),
                &mut self.context,
            );
        }
    }
}

pub(super) fn build_location(base_url: Option<&Url>, context: &mut Context) -> JsValue {
    let (href, protocol, host, pathname, search, hash, origin) = match base_url {
        Some(url) => (
            url.to_string(),
            format!("{}:", url.scheme()),
            url.host_str().unwrap_or_default().to_string(),
            url.path().to_string(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default(),
            url.fragment().map(|f| format!("#{f}")).unwrap_or_default(),
            url.origin().ascii_serialization(),
        ),
        None => (
            "about:blank".to_string(),
            "about:".to_string(),
            String::new(),
            "blank".to_string(),
            String::new(),
            String::new(),
            "null".to_string(),
        ),
    };
    ObjectInitializer::new(context)
        .property(js_string!("href"), JsString::from(href), Attribute::all())
        .property(
            js_string!("protocol"),
            JsString::from(protocol),
            Attribute::all(),
        )
        .property(
            js_string!("host"),
            JsString::from(host.clone()),
            Attribute::all(),
        )
        .property(
            js_string!("hostname"),
            JsString::from(host),
            Attribute::all(),
        )
        .property(
            js_string!("pathname"),
            JsString::from(pathname),
            Attribute::all(),
        )
        .property(
            js_string!("search"),
            JsString::from(search),
            Attribute::all(),
        )
        .property(js_string!("hash"), JsString::from(hash), Attribute::all())
        .property(
            js_string!("origin"),
            JsString::from(origin),
            Attribute::all(),
        )
        .function(
            NativeFunction::from_fn_ptr(location_assign),
            js_string!("assign"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(location_replace),
            js_string!("replace"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(location_reload),
            js_string!("reload"),
            0,
        )
        .build()
        .into()
}

/// `location.assign(url)` / `location.replace(url)`: ask the embedder to
/// navigate to `url`. `replace` is treated as a normal navigation because
/// [`NavigationProvider`](blitz_traits::navigation::NavigationProvider) does
/// not expose history replacement.
fn navigate_from_arg(args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let Some(value) = args.first() else {
        return Ok(JsValue::undefined());
    };
    if value.is_undefined() || value.is_null() {
        return Ok(JsValue::undefined());
    }
    let target = value.to_string(context)?.to_std_string_lossy();
    let ctx = dom_ctx(context)?;
    let url = {
        let doc = ctx.doc.borrow();
        doc.base_url().join(&target).ok()
    };
    if let Some(url) = url {
        navigate(&ctx, url);
    }
    Ok(JsValue::undefined())
}

pub(super) fn location_assign(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    navigate_from_arg(args, context)
}

pub(super) fn location_replace(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    navigate_from_arg(args, context)
}

pub(super) fn location_reload(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let url = { ctx.doc.borrow().base_url().clone() };
    navigate(&ctx, url);
    Ok(JsValue::undefined())
}

fn navigate(ctx: &crate::state::DomCtx, url: Url) {
    let doc = ctx.doc.borrow();
    doc.navigation_provider
        .navigate_to(NavigationOptions::new(url, None, doc.id()));
}

// === Viewport dimensions ===

pub(super) fn inner_width(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    let viewport = doc.viewport();
    Ok(JsValue::from(
        viewport.window_size.0 as f64 / viewport.scale() as f64,
    ))
}

pub(super) fn inner_height(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let doc = ctx.doc.borrow();
    let viewport = doc.viewport();
    Ok(JsValue::from(
        viewport.window_size.1 as f64 / viewport.scale() as f64,
    ))
}

pub(super) fn device_pixel_ratio(
    _: &JsValue,
    _: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let scale = ctx.doc.borrow().viewport().scale();
    Ok(JsValue::from(scale as f64))
}

// === Viewport scrolling ===

pub(super) fn scroll_x(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(JsValue::from(ctx.doc.borrow().viewport_scroll().x))
}

pub(super) fn scroll_y(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    Ok(JsValue::from(ctx.doc.borrow().viewport_scroll().y))
}

/// `window.scrollTo`/`window.scroll`: scroll the viewport, which blitz-dom
/// models as a programmatic scroll of the root element.
pub(super) fn window_scroll_to(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = crate::dom::element::parse_scroll_to_args(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    let Some(root_id) = doc.try_root_element().map(|root| root.id) else {
        return Ok(JsValue::undefined());
    };
    let current = doc.viewport_scroll();
    doc.scroll_to(
        root_id,
        parsed.left.unwrap_or(current.x),
        parsed.top.unwrap_or(current.y),
        parsed.behavior,
    );
    Ok(JsValue::undefined())
}

pub(super) fn window_scroll_by(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let parsed = crate::dom::element::parse_scroll_to_args(args, context)?;
    let ctx = dom_ctx(context)?;
    let mut doc = ctx.doc.borrow_mut();
    doc.resolve(0.0);
    let Some(root_id) = doc.try_root_element().map(|root| root.id) else {
        return Ok(JsValue::undefined());
    };
    doc.scroll_by(
        root_id,
        parsed.left.unwrap_or(0.0),
        parsed.top.unwrap_or(0.0),
        parsed.behavior,
    );
    Ok(JsValue::undefined())
}

pub(super) fn send_message(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let message = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_lossy();
    ctx.state.borrow_mut().outbound_messages.push(message);
    Ok(JsValue::undefined())
}
