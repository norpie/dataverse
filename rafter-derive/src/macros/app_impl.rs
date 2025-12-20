//! The `#[app_impl]` attribute macro for implementing the App trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, ImplItem, ImplItemFn, ItemImpl, parse2};

use super::handler::HandlerParams;
use super::impl_common::{
    HandlerMethod, KeybindScope, KeybindsMethod, app_metadata_mod, generate_keybinds_impl,
    generate_name_impl, generate_view_impl, get_type_name, is_keybinds_method, is_view_method,
    parse_handler_metadata, strip_custom_attrs,
};

/// Parse keybinds scope from attributes
fn parse_keybinds_scope(attrs: &[Attribute]) -> KeybindScope {
    for attr in attrs {
        if attr.path().is_ident("keybinds") {
            let meta: syn::Meta = attr.meta.clone();
            if let syn::Meta::List(list) = meta {
                let mut scope = KeybindScope::Global;
                let _ = list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("view") {
                        let value: syn::Expr = meta.value()?.parse()?;
                        if let syn::Expr::Path(path) = value
                            && let Some(ident) = path.path.get_ident()
                        {
                            scope = KeybindScope::View(ident.to_string());
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

/// Check if method is named "current_view"
fn is_current_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "current_view"
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
    let mut has_view = false;
    let mut has_on_start = false;
    let mut has_on_stop = false;
    let mut has_current_view = false;

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

            if is_view_method(method) {
                has_view = true;
            }

            if is_on_start_method(method) {
                has_on_start = true;
            }

            if is_on_stop_method(method) {
                has_on_stop = true;
            }

            if is_current_view_method(method) {
                has_current_view = true;
            }
        }
    }

    // Strip our custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate trait method implementations
    let keybinds_impl = generate_keybinds_impl(&keybinds_methods, &type_name);
    let view_impl = generate_view_impl(has_view, &self_ty);
    let name_impl = generate_name_impl(&type_name);

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

    // Generate current_view method
    let current_view_impl = if has_current_view {
        quote! {
            fn current_view(&self) -> Option<String> {
                #self_ty::current_view(self)
            }
        }
    } else {
        quote! {}
    };

    // Generate dirty methods
    let dirty_impl = quote! {
        fn is_dirty(&self) -> bool {
            #metadata_mod::is_dirty(self)
        }

        fn clear_dirty(&self) {
            #metadata_mod::clear_dirty(self)
        }
    };

    // Generate panic_behavior method
    let panic_impl = quote! {
        fn panic_behavior(&self) -> rafter::app::PanicBehavior {
            #metadata_mod::PANIC_BEHAVIOR
        }
    };

    // Generate dispatch method
    let dispatch_impl = generate_app_dispatch(&handlers);

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
            #name_impl
            #keybinds_final
            #view_impl
            #current_view_impl
            #on_start_impl
            #on_stop_impl
            #dirty_impl
            #panic_impl
            #dispatch_impl
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
