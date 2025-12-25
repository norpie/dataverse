//! The `#[app_impl]` attribute macro for implementing the App trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ImplItem, ImplItemFn, ItemImpl, parse2};

use super::handler::HandlerParams;
use super::impl_common::{
    EventHandlerMethod, HandlerMethod, KeybindScope, KeybindsMethod, RequestHandlerMethod,
    app_metadata_mod, generate_config_impl, generate_keybinds_impl, generate_view_impl,
    get_type_name, is_keybinds_method, is_view_method, parse_event_handler_metadata,
    parse_handler_metadata, parse_request_handler_metadata, strip_custom_attrs,
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

/// Check if method is named "on_stop"
fn is_on_stop_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "on_stop"
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
    let mut has_view = false;
    let mut has_on_start = false;
    let mut has_on_stop = false;
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

            if is_view_method(method) {
                has_view = true;
            }

            if is_on_start_method(method) {
                has_on_start = true;
            }

            if is_on_stop_method(method) {
                has_on_stop = true;
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
    let view_impl = generate_view_impl(has_view, &self_ty);
    let config_impl = generate_config_impl(&type_name);

    // Generate on_start method
    let on_start_impl = if has_on_start {
        quote! {
            fn on_start(&self, cx: &rafter::context::AppContext) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_start(self, cx)
            }
        }
    } else {
        quote! {
            fn on_start(&self, _cx: &rafter::context::AppContext) -> impl std::future::Future<Output = ()> + Send {
                async {}
            }
        }
    };

    // Generate on_stop method
    let on_stop_impl = if has_on_stop {
        quote! {
            fn on_stop(&self, cx: &rafter::context::AppContext) -> impl std::future::Future<Output = ()> + Send {
                #self_ty::on_stop(self, cx)
            }
        }
    } else {
        quote! {
            fn on_stop(&self, _cx: &rafter::context::AppContext) -> impl std::future::Future<Output = ()> + Send {
                async {}
            }
        }
    };

    // Generate current_page method
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

        fn install_wakeup(&self, sender: rafter::runtime::wakeup::WakeupSender) {
            #metadata_mod::install_wakeup(self, sender)
        }
    };

    // Generate panic_behavior method
    let panic_impl = quote! {
        fn panic_behavior(&self) -> rafter::app::PanicBehavior {
            #metadata_mod::PANIC_BEHAVIOR
        }
    };

    // Generate restart method
    let restart_impl = quote! {
        fn restart(&self, instance_id: rafter::app::InstanceId) -> Option<Box<dyn rafter::app::AnyAppInstance>> {
            if #metadata_mod::PANIC_BEHAVIOR == rafter::app::PanicBehavior::Restart {
                Some(Box::new(rafter::app::AppInstance::new_with_id(Self::default(), instance_id)))
            } else {
                None
            }
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

        impl #impl_generics rafter::app::App for #self_ty {
            #config_impl
            #keybinds_final
            #view_impl
            #current_page_impl
            #on_start_impl
            #on_stop_impl
            #dirty_impl
            #panic_impl
            #restart_impl
            #dispatch_impl
            #event_dispatch_impl
            #request_dispatch_impl
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
            let name_str = name.to_string();
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();

            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => {
                    if let Ok(event) = event.downcast::<#event_type>() {
                        let this = self.clone();
                        let cx = cx.clone();
                        let handler_name = #name_str.to_string();
                        tokio::spawn(async move {
                            let result = std::panic::AssertUnwindSafe(async {
                                this.#name(*event, &cx).await;
                            })
                            .catch_unwind()
                            .await;

                            if let Err(panic) = result {
                                if let Some(instance_id) = cx.instance_id() {
                                    let message = rafter::app::extract_panic_message(&panic);
                                    cx.report_error(rafter::app::AppError {
                                        app_name: <Self as rafter::app::App>::config().name,
                                        instance_id,
                                        kind: rafter::app::AppErrorKind::Panic {
                                            handler_name,
                                            message,
                                        },
                                    });
                                }
                            }
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
            use std::panic::AssertUnwindSafe;
            use futures::FutureExt;

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
            let name_str = name.to_string();
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();

            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => {
                    if let Ok(request) = request.downcast::<#request_type>() {
                        let this = self.clone();
                        let cx = cx.clone();
                        let handler_name = #name_str.to_string();
                        return Some(Box::pin(async move {
                            let result = std::panic::AssertUnwindSafe(async {
                                this.#name(*request, &cx).await
                            })
                            .catch_unwind()
                            .await;

                            match result {
                                Ok(response) => Box::new(response) as Box<dyn std::any::Any + Send + Sync>,
                                Err(panic) => {
                                    if let Some(instance_id) = cx.instance_id() {
                                        let message = rafter::app::extract_panic_message(&panic);
                                        cx.report_error(rafter::app::AppError {
                                            app_name: <Self as rafter::app::App>::config().name,
                                            instance_id,
                                            kind: rafter::app::AppErrorKind::Panic {
                                                handler_name,
                                                message,
                                            },
                                        });
                                    }
                                    // Return a unit type as placeholder - the caller won't get a valid response
                                    Box::new(()) as Box<dyn std::any::Any + Send + Sync>
                                }
                            }
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
            use std::panic::AssertUnwindSafe;
            use futures::FutureExt;

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

/// Generate dispatch method for app handlers
fn generate_app_dispatch(handlers: &[HandlerMethod]) -> TokenStream {
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
                        compile_error!("Handler requests ModalContext but is defined in an app impl. Use AppContext instead.");
                    }
                }
            };

            let needs_cx = h.params.needs_app_context();

            if needs_cx {
                quote! {
                    #name_str => {
                        let this = self.clone();
                        let cx = cx.clone();
                        let handler_name = #name_str.to_string();
                        tokio::spawn(async move {
                            let result = std::panic::AssertUnwindSafe(async {
                                #call
                            })
                            .catch_unwind()
                            .await;

                            if let Err(panic) = result {
                                if let Some(instance_id) = cx.instance_id() {
                                    let message = rafter::app::extract_panic_message(&panic);
                                    cx.report_error(rafter::app::AppError {
                                        app_name: <Self as rafter::app::App>::config().name,
                                        instance_id,
                                        kind: rafter::app::AppErrorKind::Panic {
                                            handler_name,
                                            message,
                                        },
                                    });
                                }
                            }
                        });
                    }
                }
            } else {
                // Handler doesn't need cx, but we still need it for error reporting
                quote! {
                    #name_str => {
                        let this = self.clone();
                        let cx = cx.clone();
                        let handler_name = #name_str.to_string();
                        tokio::spawn(async move {
                            let result = std::panic::AssertUnwindSafe(async {
                                #call
                            })
                            .catch_unwind()
                            .await;

                            if let Err(panic) = result {
                                if let Some(instance_id) = cx.instance_id() {
                                    let message = rafter::app::extract_panic_message(&panic);
                                    cx.report_error(rafter::app::AppError {
                                        app_name: <Self as rafter::app::App>::config().name,
                                        instance_id,
                                        kind: rafter::app::AppErrorKind::Panic {
                                            handler_name,
                                            message,
                                        },
                                    });
                                }
                            }
                        });
                    }
                }
            }
        })
        .collect();

    quote! {
        fn dispatch(&self, handler_id: &rafter::keybinds::HandlerId, cx: &rafter::context::AppContext) {
            use std::panic::AssertUnwindSafe;
            use futures::FutureExt;

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
