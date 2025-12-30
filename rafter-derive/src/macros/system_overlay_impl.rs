//! The `#[system_overlay_impl]` attribute macro for implementing SystemOverlay trait.
//!
//! This macro generates implementations for both the `System` trait (keybinds, handlers)
//! and the `SystemOverlay` trait (view, position, dirty tracking).

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItem, ImplItemFn, ItemImpl, parse2};

use super::handler::HandlerParams;
use super::impl_common::{
    EventHandlerMethod, HandlerMethod, KeybindScope, KeybindsMethod, RequestHandlerMethod,
    get_type_name, is_keybinds_method, parse_event_handler_metadata, parse_handler_metadata,
    parse_request_handler_metadata, strip_custom_attrs,
};

/// Check if method is named "view".
fn is_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "view"
}

/// Check if method is named "on_init".
fn is_on_init_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_init"
}

/// Convert PascalCase to snake_case.
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

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // No attributes currently supported
    let _ = attr;

    let mut impl_block: ItemImpl = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    let self_ty = impl_block.self_ty.clone();
    let type_name = match get_type_name(&self_ty) {
        Some(n) => n,
        None => {
            return syn::Error::new_spanned(self_ty, "Expected a simple type name")
                .to_compile_error();
        }
    };

    let metadata_mod = format_ident!(
        "__rafter_system_overlay_metadata_{}",
        type_name.to_string().to_lowercase()
    );
    let overlay_name_snake = to_snake_case(&type_name.to_string());

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut event_handlers = Vec::new();
    let mut request_handlers = Vec::new();
    let mut has_view = false;
    let mut has_on_init = false;

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if is_keybinds_method(method) {
                keybinds_methods.push(KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope: KeybindScope::Global,
                });
            }

            if let Some(handler) = parse_handler_metadata(method) {
                handlers.push(handler);
            }

            if let Some(event_handler) = parse_event_handler_metadata(method) {
                event_handlers.push(event_handler);
            }

            if let Some(request_handler) = parse_request_handler_metadata(method) {
                request_handlers.push(request_handler);
            }

            if is_view_method(method) {
                has_view = true;
            }

            if is_on_init_method(method) {
                has_on_init = true;
            }
        }
    }

    if !has_view {
        return syn::Error::new_spanned(
            &impl_block,
            "#[system_overlay_impl] requires a `fn view(&self) -> Node` method",
        )
        .to_compile_error();
    }

    // Strip custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate System trait methods
    let keybinds_impl = generate_keybinds(&keybinds_methods, &overlay_name_snake);
    let dispatch_impl = generate_dispatch(&handlers);
    let event_dispatch_impl = generate_event_dispatch(&event_handlers);
    let request_dispatch_impl = generate_request_dispatch(&request_handlers);

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Generate SystemOverlay trait methods
    let position_impl = quote! {
        fn position(&self) -> rafter::layers::SystemOverlayPosition {
            #metadata_mod::POSITION
        }
    };

    let view_impl = quote! {
        fn view(&self) -> rafter::node::Node {
            #self_ty::view(self)
        }
    };

    // Generate on_init impl - either call user's method or use default no-op
    let on_init_impl = if has_on_init {
        quote! {
            fn on_init(&self, cx: &rafter::context::AppContext) {
                #self_ty::on_init(self, cx)
            }
        }
    } else {
        quote! {
            // Default no-op - user didn't define on_init
        }
    };

    let dirty_impl = quote! {
        fn is_dirty(&self) -> bool {
            #metadata_mod::is_dirty(self)
        }

        fn clear_dirty(&self) {
            #metadata_mod::clear_dirty(self)
        }

        fn install_wakeup(&self, sender: rafter::runtime::wakeup::WakeupSender) {
            #metadata_mod::install_wakeup(self, sender)
        }
    };

    let impl_generics = &impl_block.generics;

    quote! {
        #impl_block

        impl #impl_generics rafter::system::System for #self_ty {
            #name_impl
            #keybinds_impl
            #dispatch_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }

        impl #impl_generics rafter::layers::SystemOverlay for #self_ty {
            #position_impl
            #view_impl
            #on_init_impl
            #dirty_impl
        }
    }
}

/// Generate keybinds method.
fn generate_keybinds(keybinds_methods: &[KeybindsMethod], overlay_name: &str) -> TokenStream {
    if keybinds_methods.is_empty() {
        return quote! {
            fn keybinds(&self) -> rafter::keybinds::Keybinds {
                rafter::keybinds::Keybinds::new()
            }
        };
    }

    let merge_calls: Vec<_> = keybinds_methods
        .iter()
        .map(|m| {
            let name = &m.name;
            quote! {
                __keybinds.merge(
                    Self::#name().with_id_prefix(#overlay_name)
                );
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

/// Generate dispatch method.
fn generate_dispatch(handlers: &[HandlerMethod]) -> TokenStream {
    if handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();

            let call = match h.params {
                HandlerParams::None => {
                    if h.is_async {
                        quote! { this.#name().await; }
                    } else {
                        quote! { this.#name(); }
                    }
                }
                HandlerParams::AppContext | HandlerParams::Both => {
                    if h.is_async {
                        quote! { this.#name(&cx).await; }
                    } else {
                        quote! { this.#name(&cx); }
                    }
                }
                HandlerParams::ModalContext => {
                    quote! {
                        compile_error!("Handler requests ModalContext but is defined in a system_overlay impl. Use AppContext instead.");
                    }
                }
            };

            let needs_cx = h.params.needs_app_context();

            if needs_cx {
                quote! {
                    #name_str => {
                        let this = self.clone();
                        let cx = cx.clone();
                        // Capture trigger_widget_id before spawning to avoid race condition
                        let trigger_widget_id = cx.trigger_widget_id();
                        tokio::spawn(async move {
                            // Restore trigger_widget_id inside the spawned task
                            if let Some(ref id) = trigger_widget_id {
                                cx.set_trigger_widget_id(id);
                            }
                            #call
                            // Clear it after handler completes
                            cx.clear_trigger_widget_id();
                        });
                    }
                }
            } else {
                quote! {
                    #name_str => {
                        let this = self.clone();
                        tokio::spawn(async move {
                            #call
                        });
                    }
                }
            }
        })
        .collect();

    quote! {
        fn dispatch(&self, handler_id: &rafter::keybinds::HandlerId, cx: &rafter::context::AppContext) {
            log::debug!("SystemOverlay dispatching handler: {}", handler_id.0);
            match handler_id.0.as_str() {
                #(#dispatch_arms)*
                other => {
                    log::warn!("Unknown system overlay handler: {}", other);
                }
            }
        }
    }
}

/// Generate event dispatch methods.
fn generate_event_dispatch(event_handlers: &[EventHandlerMethod]) -> TokenStream {
    if event_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();

            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => {
                    if let Ok(event) = event.downcast::<#event_type>() {
                        let this = self.clone();
                        let cx = cx.clone();
                        tokio::spawn(async move {
                            this.#name(*event, &cx).await;
                        });
                        return true;
                    }
                    false
                }
            }
        })
        .collect();

    let has_handler_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();
            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => true,
            }
        })
        .collect();

    quote! {
        fn dispatch_event(
            &self,
            event_type: std::any::TypeId,
            event: Box<dyn std::any::Any + Send + Sync>,
            cx: &rafter::context::AppContext,
        ) -> bool {
            match event_type {
                #(#dispatch_arms)*
                _ => false,
            }
        }

        fn has_event_handler(&self, event_type: std::any::TypeId) -> bool {
            match event_type {
                #(#has_handler_arms)*
                _ => false,
            }
        }
    }
}

/// Generate request dispatch methods.
fn generate_request_dispatch(request_handlers: &[RequestHandlerMethod]) -> TokenStream {
    if request_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();

            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => {
                    if let Ok(request) = request.downcast::<#request_type>() {
                        let this = self.clone();
                        let cx = cx.clone();
                        return Some(Box::pin(async move {
                            let response = this.#name(*request, &cx).await;
                            Box::new(response) as Box<dyn std::any::Any + Send + Sync>
                        }));
                    }
                    None
                }
            }
        })
        .collect();

    let has_handler_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();
            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => true,
            }
        })
        .collect();

    quote! {
        fn dispatch_request(
            &self,
            request_type: std::any::TypeId,
            request: Box<dyn std::any::Any + Send + Sync>,
            cx: &rafter::context::AppContext,
        ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>>> {
            match request_type {
                #(#dispatch_arms)*
                _ => None,
            }
        }

        fn has_request_handler(&self, request_type: std::any::TypeId) -> bool {
            match request_type {
                #(#has_handler_arms)*
                _ => false,
            }
        }
    }
}
