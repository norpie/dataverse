//! The `#[system_impl]` attribute macro for implementing the System trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItem, ImplItemFn, ItemImpl, parse2};

use super::impl_common::{
    EventHandlerMethod, HandlerMethod, KeybindScope, KeybindsMethod, PageMethod,
    RequestHandlerMethod, get_type_name, is_element_method, is_keybinds_method,
    parse_event_handler_metadata, parse_handler_metadata, parse_page_metadata,
    parse_request_handler_metadata, strip_custom_attrs, system_metadata_mod, to_snake_case,
};

/// Check if method is named "on_init"
fn is_on_init_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_init"
}

/// Check if method is named "overlay"
fn is_overlay_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "overlay"
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // No attributes currently supported for system_impl
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

    let metadata_mod = system_metadata_mod(&type_name);
    let system_name_snake = to_snake_case(&type_name.to_string());

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut event_handlers = Vec::new();
    let mut request_handlers = Vec::new();
    let mut page_methods: Vec<PageMethod> = Vec::new();
    let mut has_element = false;
    let mut has_on_init = false;
    let mut has_overlay = false;

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if is_keybinds_method(method) {
                // Systems don't support page scope - always global
                keybinds_methods.push(KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope: KeybindScope::Global,
                });
            }

            if let Some(handler) = parse_handler_metadata(method) {
                // Check for ModalContext usage - systems cannot use modal context
                if handler.contexts.modal_context {
                    return syn::Error::new_spanned(
                        &method.sig,
                        "System handlers cannot use ModalContext. ModalContext is only available in modal handlers.",
                    )
                    .to_compile_error();
                }
                // Check for AppContext usage - systems only get GlobalContext
                if handler.contexts.app_context {
                    return syn::Error::new_spanned(
                        &method.sig,
                        "System handlers cannot use AppContext. Systems only have access to GlobalContext.",
                    )
                    .to_compile_error();
                }
                handlers.push(handler);
            }

            if let Some(event_handler) = parse_event_handler_metadata(method) {
                event_handlers.push(event_handler);
            }

            if let Some(request_handler) = parse_request_handler_metadata(method) {
                request_handlers.push(request_handler);
            }

            if let Some(page) = parse_page_metadata(method) {
                page_methods.push(page);
            }

            if is_element_method(method) {
                has_element = true;
            }

            if is_on_init_method(method) {
                has_on_init = true;
            }

            if is_overlay_method(method) {
                has_overlay = true;
            }
        }
    }

    // Systems don't support #[page] - they use overlays instead
    if !page_methods.is_empty() {
        return syn::Error::new_spanned(
            &impl_block.self_ty,
            "Systems don't support #[page]. Use overlay() method instead.",
        )
        .to_compile_error();
    }

    // Strip our custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate trait method implementations
    let keybinds_impl = generate_system_keybinds(&keybinds_methods, &system_name_snake);

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Generate overlay method (uses element() if defined)
    let overlay_impl = if has_overlay {
        quote! {
            fn overlay(&self) -> Option<rafter::Overlay> {
                #self_ty::overlay(self)
            }
        }
    } else if has_element {
        // If element() is defined but overlay() is not, we can't automatically
        // create an overlay since we don't know the position. User must define overlay().
        quote! {}
    } else {
        quote! {}
    };

    // Generate on_init method
    let on_init_impl = if has_on_init {
        quote! {
            fn on_init(&self) {
                #self_ty::on_init(self)
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

    // Generate dispatch methods - systems only get GlobalContext
    let dispatch_impl = generate_system_dispatch(&handlers);
    let event_dispatch_impl = generate_system_event_dispatch(&event_handlers);
    let request_dispatch_impl = generate_system_request_dispatch(&request_handlers);

    // Output the impl block plus System trait implementation
    let impl_generics = &impl_block.generics;

    // Suppress unused variable warning for has_element
    let _ = has_element;

    quote! {
        #impl_block

        impl #impl_generics rafter::System for #self_ty {
            #name_impl
            #keybinds_impl
            #overlay_impl
            #on_init_impl
            #dirty_impl
            #dispatch_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }
    }
}

/// Generate keybinds method for system
fn generate_system_keybinds(keybinds_methods: &[KeybindsMethod], system_name: &str) -> TokenStream {
    if keybinds_methods.is_empty() {
        return quote! {
            fn keybinds(&self) -> rafter::Keybinds {
                rafter::Keybinds::new()
            }
        };
    }

    let merge_calls: Vec<_> = keybinds_methods
        .iter()
        .map(|m| {
            let name = &m.name;
            quote! {
                __keybinds.merge(
                    Self::#name().with_id_prefix(#system_name)
                );
            }
        })
        .collect();

    quote! {
        fn keybinds(&self) -> rafter::Keybinds {
            let mut __keybinds = rafter::Keybinds::new();
            #(#merge_calls)*
            __keybinds
        }
    }
}

/// Generate dispatch method for system handlers.
/// Systems only receive GlobalContext (no AppContext).
fn generate_system_dispatch(handlers: &[HandlerMethod]) -> TokenStream {
    if handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();

            // Systems only get GlobalContext
            let call = if h.contexts.global_context {
                if h.is_async {
                    quote! { this.#name(&gx).await; }
                } else {
                    quote! { this.#name(&gx); }
                }
            } else {
                if h.is_async {
                    quote! { this.#name().await; }
                } else {
                    quote! { this.#name(); }
                }
            };

            let clones = if h.contexts.global_context {
                quote! { let this = self.clone(); let gx = gx.clone(); }
            } else {
                quote! { let this = self.clone(); }
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
        fn dispatch(&self, handler_id: &rafter::HandlerId, gx: &rafter::GlobalContext) {
            log::debug!("System dispatching handler: {}", handler_id.0);
            match handler_id.0.as_str() {
                #(#dispatch_arms)*
                other => {
                    log::warn!("Unknown system handler: {}", other);
                }
            }
        }
    }
}

/// Generate event dispatch methods for system event handlers.
/// Systems only receive GlobalContext.
fn generate_system_event_dispatch(event_handlers: &[EventHandlerMethod]) -> TokenStream {
    if event_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();

            let call = if h.contexts.global_context {
                quote! { this.#name(*event, &gx).await; }
            } else {
                quote! { this.#name(*event).await; }
            };

            let clones = if h.contexts.global_context {
                quote! { let this = self.clone(); let gx = gx.clone(); }
            } else {
                quote! { let this = self.clone(); }
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

/// Generate request dispatch methods for system request handlers.
/// Systems only receive GlobalContext.
fn generate_system_request_dispatch(request_handlers: &[RequestHandlerMethod]) -> TokenStream {
    if request_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();

            let call = if h.contexts.global_context {
                quote! { this.#name(*request, &gx).await }
            } else {
                quote! { this.#name(*request).await }
            };

            let clones = if h.contexts.global_context {
                quote! { let this = self.clone(); let gx = gx.clone(); }
            } else {
                quote! { let this = self.clone(); }
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
