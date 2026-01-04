//! The `#[app_impl]` attribute macro for implementing the App trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ImplItem, ImplItemFn, ItemImpl, parse2};

use super::impl_common::{
    EventHandlerMethod, HandlerContexts, HandlerMethod, KeybindScope, KeybindsMethod,
    RequestHandlerMethod, app_metadata_mod, generate_config_impl, generate_element_impl,
    generate_keybinds_impl, get_type_name, is_element_method, is_keybinds_method,
    parse_event_handler_metadata, parse_handler_metadata, parse_request_handler_metadata,
    strip_custom_attrs,
};

/// Parse keybinds scope from attributes
fn parse_keybinds_scope(attrs: &[Attribute]) -> KeybindScope {
    for attr in attrs {
        if attr.path().is_ident("keybinds") {
            let meta: syn::Meta = attr.meta.clone();
            if let syn::Meta::List(list) = meta {
                let mut scope = KeybindScope::Global;
                let _ = list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("page") {
                        let value: syn::Expr = meta.value()?.parse()?;
                        if let syn::Expr::Path(path) = value
                            && let Some(ident) = path.path.get_ident()
                        {
                            scope = KeybindScope::Page(ident.to_string());
                        }
                    } else if meta.path.is_ident("global") {
                        scope = KeybindScope::Global;
                    }
                    Ok(())
                });
                return scope;
            }
        }
    }
    KeybindScope::Global
}

/// Check if method is named "on_start"
fn is_on_start_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_start"
}

/// Check if method is named "on_foreground"
fn is_on_foreground_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_foreground"
}

/// Check if method is named "on_background"
fn is_on_background_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_background"
}

/// Check if method is named "on_close"
fn is_on_close_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_close"
}

/// Check if method is named "current_page"
fn is_current_page_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "current_page"
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // No attributes currently supported for app_impl
    let _ = attr;

    let mut impl_block: ItemImpl = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    // Get the type we're implementing for
    let self_ty = impl_block.self_ty.clone();
    let type_name = match get_type_name(&self_ty) {
        Some(n) => n,
        None => {
            return syn::Error::new_spanned(self_ty, "Expected a simple type name")
                .to_compile_error();
        }
    };

    let metadata_mod = app_metadata_mod(&type_name);

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut event_handlers = Vec::new();
    let mut request_handlers = Vec::new();
    let mut has_element = false;
    let mut has_on_start = false;
    let mut has_on_foreground = false;
    let mut has_on_background = false;
    let mut has_on_close = false;
    let mut has_current_page = false;

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if is_keybinds_method(method) {
                let scope = parse_keybinds_scope(&method.attrs);
                keybinds_methods.push(KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope,
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

            if is_element_method(method) {
                has_element = true;
            }

            if is_on_start_method(method) {
                has_on_start = true;
            }

            if is_on_foreground_method(method) {
                has_on_foreground = true;
            }

            if is_on_background_method(method) {
                has_on_background = true;
            }

            if is_on_close_method(method) {
                has_on_close = true;
            }

            if is_current_page_method(method) {
                has_current_page = true;
            }
        }
    }

    // Strip our custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate trait method implementations
    let keybinds_impl = generate_keybinds_impl(&keybinds_methods, &type_name);
    let element_impl = generate_element_impl(has_element, &self_ty);
    let config_impl = generate_config_impl(&type_name);

    // Generate lifecycle methods
    let on_start_impl = if has_on_start {
        quote! {
            fn on_start(&self) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_start(self)
            }
        }
    } else {
        quote! {}
    };

    let on_foreground_impl = if has_on_foreground {
        quote! {
            fn on_foreground(&self) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_foreground(self)
            }
        }
    } else {
        quote! {}
    };

    let on_background_impl = if has_on_background {
        quote! {
            fn on_background(&self) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_background(self)
            }
        }
    } else {
        quote! {}
    };

    let on_close_impl = if has_on_close {
        quote! {
            fn on_close(&self) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_close(self)
            }
        }
    } else {
        quote! {}
    };

    let current_page_impl = if has_current_page {
        quote! {
            fn current_page(&self) -> Option<String> {
                #self_ty::current_page(self)
            }
        }
    } else {
        quote! {}
    };

    // Generate dirty methods and wakeup installation
    let dirty_impl = quote! {
        fn is_dirty(&self) -> bool {
            #metadata_mod::is_dirty(self)
        }

        fn clear_dirty(&self) {
            #metadata_mod::clear_dirty(self)
        }

        fn install_wakeup(&self, sender: rafter::WakeupSender) {
            #metadata_mod::install_wakeup(self, sender)
        }
    };

    // Generate panic_behavior method
    let panic_impl = quote! {
        fn panic_behavior(&self) -> rafter::PanicBehavior {
            #metadata_mod::PANIC_BEHAVIOR
        }
    };

    // Generate dispatch methods
    let dispatch_impl = generate_app_dispatch(&handlers);
    let event_dispatch_impl = generate_event_dispatch(&event_handlers);
    let request_dispatch_impl = generate_request_dispatch(&request_handlers);

    // Output the impl block plus App trait implementation
    let impl_generics = &impl_block.generics;

    // Check if user already implements keybinds
    let user_has_keybinds = impl_block.items.iter().any(|item| {
        if let ImplItem::Fn(m) = item {
            m.sig.ident == "keybinds"
        } else {
            false
        }
    });

    let keybinds_final = if user_has_keybinds {
        quote! {}
    } else {
        keybinds_impl
    };

    quote! {
        #impl_block

        impl #impl_generics rafter::App for #self_ty {
            #config_impl
            #keybinds_final
            #element_impl
            #current_page_impl
            #on_start_impl
            #on_foreground_impl
            #on_background_impl
            #on_close_impl
            #dirty_impl
            #panic_impl
            #dispatch_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }
    }
}

/// Generate dispatch method for app handlers.
/// Apps receive both AppContext and GlobalContext.
fn generate_app_dispatch(handlers: &[HandlerMethod]) -> TokenStream {
    if handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();
            let call = generate_handler_call(name, &h.contexts, h.is_async, false);

            // Determine what to clone based on what the handler needs
            let clone_cx = h.contexts.needs_app_context();
            let clone_gx = h.contexts.needs_global_context();

            let clones = match (clone_cx, clone_gx) {
                (false, false) => quote! { let this = self.clone(); },
                (true, false) => quote! { let this = self.clone(); let cx = cx.clone(); },
                (false, true) => quote! { let this = self.clone(); let gx = gx.clone(); },
                (true, true) => quote! { let this = self.clone(); let cx = cx.clone(); let gx = gx.clone(); },
            };

            quote! {
                #name_str => {
                    #clones
                    tokio::spawn(async move {
                        #call
                    });
                }
            }
        })
        .collect();

    quote! {
        fn dispatch(&self, handler_id: &rafter::HandlerId, cx: &rafter::AppContext, gx: &rafter::GlobalContext) {
            log::debug!("Dispatching handler: {}", handler_id.0);
            match handler_id.0.as_str() {
                #(#dispatch_arms)*
                other => {
                    log::warn!("Unknown handler: {}", other);
                }
            }
        }
    }
}

/// Generate event dispatch methods for app event handlers
fn generate_event_dispatch(event_handlers: &[EventHandlerMethod]) -> TokenStream {
    if event_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();
            let call = generate_event_handler_call(name, &h.contexts);

            let clone_cx = h.contexts.needs_app_context();
            let clone_gx = h.contexts.needs_global_context();

            let clones = match (clone_cx, clone_gx) {
                (false, false) => quote! { let this = self.clone(); },
                (true, false) => quote! { let this = self.clone(); let cx = cx.clone(); },
                (false, true) => quote! { let this = self.clone(); let gx = gx.clone(); },
                (true, true) => quote! { let this = self.clone(); let cx = cx.clone(); let gx = gx.clone(); },
            };

            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => {
                    if let Ok(event) = event.downcast::<#event_type>() {
                        #clones
                        tokio::spawn(async move {
                            #call
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
            cx: &rafter::AppContext,
            gx: &rafter::GlobalContext,
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

/// Generate request dispatch methods for app request handlers
fn generate_request_dispatch(request_handlers: &[RequestHandlerMethod]) -> TokenStream {
    if request_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();
            let call = generate_request_handler_call(name, &h.contexts);

            let clone_cx = h.contexts.needs_app_context();
            let clone_gx = h.contexts.needs_global_context();

            let clones = match (clone_cx, clone_gx) {
                (false, false) => quote! { let this = self.clone(); },
                (true, false) => quote! { let this = self.clone(); let cx = cx.clone(); },
                (false, true) => quote! { let this = self.clone(); let gx = gx.clone(); },
                (true, true) => quote! { let this = self.clone(); let cx = cx.clone(); let gx = gx.clone(); },
            };

            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => {
                    if let Ok(request) = request.downcast::<#request_type>() {
                        #clones
                        return Some(Box::pin(async move {
                            let response = #call;
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
            cx: &rafter::AppContext,
            gx: &rafter::GlobalContext,
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

/// Generate the handler call with appropriate context parameters (varargs pattern).
/// For apps: can use AppContext and/or GlobalContext.
fn generate_handler_call(
    name: &syn::Ident,
    contexts: &HandlerContexts,
    is_async: bool,
    _is_modal: bool,
) -> TokenStream {
    let call = match (contexts.app_context, contexts.global_context) {
        (false, false) => quote! { this.#name() },
        (true, false) => quote! { this.#name(&cx) },
        (false, true) => quote! { this.#name(&gx) },
        (true, true) => quote! { this.#name(&cx, &gx) },
    };

    if is_async {
        quote! { #call.await; }
    } else {
        quote! { #call; }
    }
}

/// Generate event handler call with appropriate context parameters.
fn generate_event_handler_call(name: &syn::Ident, contexts: &HandlerContexts) -> TokenStream {
    match (contexts.app_context, contexts.global_context) {
        (false, false) => quote! { this.#name(*event).await; },
        (true, false) => quote! { this.#name(*event, &cx).await; },
        (false, true) => quote! { this.#name(*event, &gx).await; },
        (true, true) => quote! { this.#name(*event, &cx, &gx).await; },
    }
}

/// Generate request handler call with appropriate context parameters.
fn generate_request_handler_call(name: &syn::Ident, contexts: &HandlerContexts) -> TokenStream {
    match (contexts.app_context, contexts.global_context) {
        (false, false) => quote! { this.#name(*request).await },
        (true, false) => quote! { this.#name(*request, &cx).await },
        (false, true) => quote! { this.#name(*request, &gx).await },
        (true, true) => quote! { this.#name(*request, &cx, &gx).await },
    }
}
