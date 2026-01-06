//! The `#[app_impl]` attribute macro for implementing the App trait.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, parse2};

use super::impl_common::{
    DispatchContextType, EventHandlerMethod, HandlerContexts, HandlerInfo, KeybindScope,
    KeybindsMethod, PageMethod, PartialImplBlock, RequestHandlerMethod, app_metadata_mod,
    detect_handler_contexts_from_sig, extract_handler_info, generate_config_impl,
    generate_element_impl, generate_event_dispatch, generate_handler_wrappers,
    generate_keybinds_closures_impl, generate_request_dispatch, get_type_name,
    parse_event_handler_metadata, parse_request_handler_metadata, reconstruct_method,
    reconstruct_method_stripped,
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

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // No attributes currently supported for app_impl
    let _ = attr;

    // Parse as PartialImplBlock to keep method bodies as raw tokens
    let partial_impl: PartialImplBlock = match parse2(item) {
        Ok(i) => i,
        Err(e) => return e.to_compile_error(),
    };

    // Get the type we're implementing for
    let self_ty = partial_impl.self_ty.clone();
    let type_name = match get_type_name(&self_ty) {
        Some(n) => n,
        None => {
            return syn::Error::new_spanned(self_ty, "Expected a simple type name")
                .to_compile_error();
        }
    };

    let metadata_mod = app_metadata_mod(&type_name);

    // Collect method information
    let mut keybinds_methods: Vec<(KeybindsMethod, TokenStream)> = Vec::new();
    let mut handler_contexts: HashMap<String, HandlerContexts> = HashMap::new();
    let mut handler_infos: Vec<HandlerInfo> = Vec::new();
    let mut event_handlers: Vec<EventHandlerMethod> = Vec::new();
    let mut request_handlers: Vec<RequestHandlerMethod> = Vec::new();
    let mut page_methods: Vec<PageMethod> = Vec::new();
    let mut has_element = false;
    let mut has_on_start = false;
    let mut has_on_foreground = false;
    let mut has_on_background = false;
    let mut has_on_close = false;
    let mut has_current_page = false;

    // Reconstructed methods for the impl block
    let mut reconstructed_methods: Vec<TokenStream> = Vec::new();

    for method in &partial_impl.methods {
        // Check for keybinds method
        if method.has_attr("keybinds") {
            let scope = parse_keybinds_scope(&method.attrs);
            keybinds_methods.push((
                KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope,
                },
                method.body.clone(),
            ));
            // Don't add keybinds methods to reconstructed output - they're consumed
            continue;
        }

        // Check for handler method
        if method.has_attr("handler") {
            let handler_info = extract_handler_info(&method.sig.ident, &method.sig);

            // Check for ModalContext usage - apps cannot use modal context
            if handler_info.contexts.modal_context {
                return syn::Error::new_spanned(
                    &method.sig,
                    "App handlers cannot use ModalContext. ModalContext is only available in modal handlers.",
                )
                .to_compile_error();
            }

            handler_contexts.insert(method.sig.ident.to_string(), handler_info.contexts.clone());
            handler_infos.push(handler_info);
        }

        // For event/request handlers, we need to convert to ImplItemFn temporarily
        // to use existing parse functions (they expect ImplItemFn)
        let reconstructed = reconstruct_method(method);
        if let Ok(impl_item) = syn::parse2::<syn::ImplItemFn>(reconstructed.clone()) {
            if let Some(event_handler) = parse_event_handler_metadata(&impl_item) {
                event_handlers.push(event_handler);
            }
            if let Some(request_handler) = parse_request_handler_metadata(&impl_item) {
                request_handlers.push(request_handler);
            }
        }

        // Check for page method
        if method.has_attr("page") {
            // Extract page name from attribute
            let page_name = method.attrs.iter().find_map(|attr| {
                if attr.path().is_ident("page") {
                    match &attr.meta {
                        syn::Meta::Path(_) => None,
                        syn::Meta::List(list) => {
                            let tokens = &list.tokens;
                            syn::parse2::<syn::Ident>(tokens.clone())
                                .ok()
                                .map(|n| n.to_string())
                        }
                        syn::Meta::NameValue(_) => None,
                    }
                } else {
                    None
                }
            });

            page_methods.push(PageMethod {
                name: method.sig.ident.clone(),
                page_name,
                body: method.body.clone(),
            });
        }

        // Check lifecycle methods using is_named()
        if method.is_named("element") {
            has_element = true;
        }
        if method.is_named("on_start") {
            has_on_start = true;
        }
        if method.is_named("on_foreground") {
            has_on_foreground = true;
        }
        if method.is_named("on_background") {
            has_on_background = true;
        }
        if method.is_named("on_close") {
            has_on_close = true;
        }
        if method.is_named("current_page") {
            has_current_page = true;
        }

        // Add to reconstructed methods (with custom attrs stripped)
        reconstructed_methods.push(reconstruct_method_stripped(method));
    }

    // Validate page methods for apps
    let mut seen_pages = std::collections::HashSet::new();
    for page in &page_methods {
        if let Some(ref name) = page.page_name {
            if !seen_pages.insert(name.clone()) {
                return syn::Error::new_spanned(
                    &partial_impl.self_ty,
                    format!("Duplicate page name: {}", name),
                )
                .to_compile_error();
            }
        }
    }

    // TODO: Process page_methods for DSL parsing
    let _ = &page_methods;

    // Generate trait method implementations
    let keybinds_impl =
        generate_keybinds_closures_impl(&keybinds_methods, &handler_contexts, &type_name);
    let element_impl = generate_element_impl(has_element, &self_ty);
    let config_impl = generate_config_impl(&type_name);

    // Generate handlers() method
    let handlers_impl = quote! {
        fn handlers(&self) -> &rafter::HandlerRegistry {
            &self.__handler_registry
        }
    };

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

    // Generate event/request dispatch methods
    let event_dispatch_impl = generate_event_dispatch(&event_handlers, DispatchContextType::App);
    let request_dispatch_impl = generate_request_dispatch(&request_handlers, DispatchContextType::App);

    // Generate handler wrapper methods
    let handler_wrappers = generate_handler_wrappers(&handler_infos);

    // Output the impl block plus App trait implementation
    let impl_generics = &partial_impl.generics;
    let impl_attrs = &partial_impl.attrs;

    quote! {
        #(#impl_attrs)*
        impl #impl_generics #self_ty {
            #(#reconstructed_methods)*

            // Handler wrappers for page! macro integration
            #handler_wrappers
        }

        impl #impl_generics rafter::App for #self_ty {
            #config_impl
            #keybinds_impl
            #handlers_impl
            #element_impl
            #current_page_impl
            #on_start_impl
            #on_foreground_impl
            #on_background_impl
            #on_close_impl
            #dirty_impl
            #panic_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }
    }
}
