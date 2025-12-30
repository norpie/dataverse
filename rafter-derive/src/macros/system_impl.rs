//! The `#[system_impl]` attribute macro for implementing the System trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{ImplItem, ItemImpl, parse2};

use super::handler::HandlerParams;
use super::impl_common::{
    EventHandlerMethod, HandlerMethod, KeybindScope, KeybindsMethod, RequestHandlerMethod,
    get_type_name, is_keybinds_method, parse_event_handler_metadata, parse_handler_metadata,
    parse_request_handler_metadata, strip_custom_attrs,
};

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

    let system_name_snake = to_snake_case(&type_name.to_string());

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut event_handlers = Vec::new();
    let mut request_handlers = Vec::new();

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
                handlers.push(handler);
            }

            if let Some(event_handler) = parse_event_handler_metadata(method) {
                event_handlers.push(event_handler);
            }

            if let Some(request_handler) = parse_request_handler_metadata(method) {
                request_handlers.push(request_handler);
            }
        }
    }

    // Strip our custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate trait method implementations
    let keybinds_impl = generate_system_keybinds(&keybinds_methods, &system_name_snake);
    let dispatch_impl = generate_system_dispatch(&handlers, &system_name_snake);
    let event_dispatch_impl = generate_system_event_dispatch(&event_handlers);
    let request_dispatch_impl = generate_system_request_dispatch(&request_handlers);

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Output the impl block plus System trait implementation
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
    }
}

/// Generate keybinds method for system
fn generate_system_keybinds(keybinds_methods: &[KeybindsMethod], system_name: &str) -> TokenStream {
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
                    Self::#name().with_id_prefix(#system_name)
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

/// Generate dispatch method for system handlers
fn generate_system_dispatch(handlers: &[HandlerMethod], _system_name: &str) -> TokenStream {
    if handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();
            // Handler ID is just the method name, not prefixed
            // (the keybind ID is prefixed, but handler ID is not)

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
                        compile_error!("Handler requests ModalContext but is defined in a system impl. Use AppContext instead.");
                    }
                }
            };

            let needs_cx = h.params.needs_app_context();

            if needs_cx {
                quote! {
                    #name_str => {
                        let this = self.clone();
                        let cx = cx.clone();
                        tokio::spawn(async move {
                            #call
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

/// Generate event dispatch methods for system event handlers
fn generate_system_event_dispatch(event_handlers: &[EventHandlerMethod]) -> TokenStream {
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

/// Generate request dispatch methods for system request handlers
fn generate_system_request_dispatch(request_handlers: &[RequestHandlerMethod]) -> TokenStream {
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
