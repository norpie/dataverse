//! Common utilities shared between app_impl and modal_impl macros.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ImplItem, ImplItemFn, ItemImpl, Type};

use super::handler::HandlerParams;

/// Keybind scope parsed from #[keybinds(page = X)] attribute
#[derive(Clone, Debug, Default)]
pub enum KeybindScope {
    /// No scope specified - global keybinds
    #[default]
    Global,
    /// Page-scoped keybinds
    Page(String),
}

/// Information about a keybinds method
pub struct KeybindsMethod {
    /// Method name
    pub name: Ident,
    /// Scope for these keybinds
    pub scope: KeybindScope,
}

/// Information about a handler method
pub struct HandlerMethod {
    /// Method name
    pub name: Ident,
    /// Handler parameter requirements
    pub params: HandlerParams,
    /// Handler is async
    pub is_async: bool,
}

/// Information about an event handler method
pub struct EventHandlerMethod {
    /// Method name
    pub name: Ident,
    /// Event type as a string (for codegen)
    pub event_type: String,
}

/// Information about a request handler method
pub struct RequestHandlerMethod {
    /// Method name
    pub name: Ident,
    /// Request type as a string (for codegen)
    pub request_type: String,
}

/// Check if a method has the #[keybinds] attribute
pub fn is_keybinds_method(method: &ImplItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("keybinds"))
}

/// Check if method has #[handler] attribute and extract metadata
pub fn parse_handler_metadata(method: &ImplItemFn) -> Option<HandlerMethod> {
    for attr in &method.attrs {
        if attr.path().is_ident("handler") {
            let params = detect_handler_params_from_impl_fn(method);
            let is_async = method.sig.asyncness.is_some();

            return Some(HandlerMethod {
                name: method.sig.ident.clone(),
                params,
                is_async,
            });
        }
    }
    None
}

/// Check if method has #[event_handler] attribute and extract metadata
pub fn parse_event_handler_metadata(method: &ImplItemFn) -> Option<EventHandlerMethod> {
    // First check for the attribute
    let has_attr = method
        .attrs
        .iter()
        .any(|a| a.path().is_ident("event_handler"));
    if !has_attr {
        return None;
    }

    // Extract event type from metadata doc attribute
    for attr in &method.attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            let value = s.value();
            if let Some(event_type) = value.strip_prefix("__rafter_event_handler:") {
                return Some(EventHandlerMethod {
                    name: method.sig.ident.clone(),
                    event_type: event_type.to_string(),
                });
            }
        }
    }
    None
}

/// Check if method has #[request_handler] attribute and extract metadata
pub fn parse_request_handler_metadata(method: &ImplItemFn) -> Option<RequestHandlerMethod> {
    // First check for the attribute
    let has_attr = method
        .attrs
        .iter()
        .any(|a| a.path().is_ident("request_handler"));
    if !has_attr {
        return None;
    }

    // Extract request type from metadata doc attribute
    for attr in &method.attrs {
        if attr.path().is_ident("doc")
            && let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) = &nv.value
        {
            let value = s.value();
            if let Some(request_type) = value.strip_prefix("__rafter_request_handler:") {
                return Some(RequestHandlerMethod {
                    name: method.sig.ident.clone(),
                    request_type: request_type.to_string(),
                });
            }
        }
    }
    None
}

/// Detect handler params from an impl method
pub fn detect_handler_params_from_impl_fn(method: &ImplItemFn) -> HandlerParams {
    let mut has_app_context = false;
    let mut has_modal_context = false;

    for arg in &method.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            if ty_str.contains("AppContext") {
                has_app_context = true;
            }
            if ty_str.contains("ModalContext") {
                has_modal_context = true;
            }
        }
    }

    match (has_app_context, has_modal_context) {
        (false, false) => HandlerParams::None,
        (true, false) => HandlerParams::AppContext,
        (false, true) => HandlerParams::ModalContext,
        (true, true) => HandlerParams::Both,
    }
}

/// Check if method is named "page"
pub fn is_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "page"
}

/// Extract the type name from a Type
pub fn get_type_name(ty: &Type) -> Option<Ident> {
    if let Type::Path(path) = ty {
        path.path.get_ident().cloned()
    } else {
        None
    }
}

/// Generate the metadata module name for an app
pub fn app_metadata_mod(type_name: &Ident) -> Ident {
    format_ident!(
        "__rafter_app_metadata_{}",
        type_name.to_string().to_lowercase()
    )
}

/// Generate the metadata module name for a modal
pub fn modal_metadata_mod(type_name: &Ident) -> Ident {
    format_ident!(
        "__rafter_modal_metadata_{}",
        type_name.to_string().to_lowercase()
    )
}

/// Strip custom rafter attributes from methods in an impl block
pub fn strip_custom_attrs(impl_block: &mut ItemImpl) {
    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|a| {
                !a.path().is_ident("keybinds")
                    && !a.path().is_ident("handler")
                    && !a.path().is_ident("event_handler")
                    && !a.path().is_ident("request_handler")
            });
            // Remove metadata doc attributes
            method.attrs.retain(|a| {
                if a.path().is_ident("doc")
                    && let syn::Meta::NameValue(nv) = &a.meta
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                {
                    let val = s.value();
                    return !val.starts_with("__rafter_handler:")
                        && !val.starts_with("__rafter_event_handler:")
                        && !val.starts_with("__rafter_request_handler:");
                }
                true
            });
        }
    }
}

/// Convert a PascalCase or camelCase identifier to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

/// Generate keybinds trait method implementation
pub fn generate_keybinds_impl(
    keybinds_methods: &[KeybindsMethod],
    type_name: &Ident,
) -> TokenStream {
    let app_name_snake = to_snake_case(&type_name.to_string());

    if keybinds_methods.is_empty() {
        quote! {
            fn keybinds(&self) -> rafter::keybinds::Keybinds {
                rafter::keybinds::Keybinds::new()
            }
        }
    } else {
        let merge_calls: Vec<_> = keybinds_methods
            .iter()
            .map(|m| {
                let name = &m.name;
                match &m.scope {
                    KeybindScope::Global => {
                        // ID prefix is just the app name for global keybinds
                        let id_prefix = &app_name_snake;
                        quote! {
                            __keybinds.merge(
                                Self::#name().with_id_prefix(#id_prefix)
                            );
                        }
                    }
                    KeybindScope::Page(view_name) => {
                        // ID prefix includes the page name for scoped keybinds
                        let view_name_snake = to_snake_case(view_name);
                        let id_prefix = format!("{}.{}", app_name_snake, view_name_snake);
                        quote! {
                            __keybinds.merge(
                                Self::#name()
                                    .with_id_prefix(#id_prefix)
                                    .with_scope(rafter::keybinds::KeybindScope::Page(#view_name.to_string()))
                            );
                        }
                    }
                }
            })
            .collect();

        quote! {
            fn keybinds(&self) -> rafter::keybinds::Keybinds {
                let mut __keybinds = rafter::keybinds::Keybinds::new();
                #(#merge_calls)*
                __keybinds
            }
        }
    }
}

/// Generate page trait method implementation
pub fn generate_view_impl(has_view: bool, self_ty: &Type) -> TokenStream {
    if has_view {
        quote! {
            fn page(&self) -> rafter::node::Node {
                #self_ty::page(self)
            }
        }
    } else {
        quote! {
            fn page(&self) -> rafter::node::Node {
                rafter::node::Node::empty()
            }
        }
    }
}

/// Generate name trait method implementation
pub fn generate_name_impl(type_name: &Ident) -> TokenStream {
    let type_name_str = type_name.to_string();
    quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    }
}

/// Generate config trait method implementation using metadata module
pub fn generate_config_impl(type_name: &Ident) -> TokenStream {
    let metadata_mod = app_metadata_mod(type_name);
    quote! {
        fn config() -> rafter::app::AppConfig {
            #metadata_mod::config()
        }
    }
}
