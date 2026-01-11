//! The `#[system_impl]` attribute macro for implementing the System trait.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;

use super::impl_common::{
    DispatchContextType, EventHandlerMethod, HandlerContexts, HandlerInfo, KeybindScope,
    KeybindsMethod, PageMethod, PartialImplBlock, RequestHandlerMethod,
    extract_handler_info, generate_async_lifecycle_impl, generate_event_dispatch,
    generate_handler_wrappers, generate_keybinds_closures_impl, generate_request_dispatch,
    get_type_name, parse_event_handler_metadata, parse_request_handler_metadata,
    reconstruct_method, reconstruct_method_stripped, system_metadata_mod,
};

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    // No attributes currently supported for system_impl
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

    let metadata_mod = system_metadata_mod(&type_name);

    // Collect method information
    let mut keybinds_methods: Vec<(KeybindsMethod, TokenStream)> = Vec::new();
    let mut handler_contexts: HashMap<String, HandlerContexts> = HashMap::new();
    let mut handler_infos: Vec<HandlerInfo> = Vec::new();
    let mut event_handlers: Vec<EventHandlerMethod> = Vec::new();
    let mut request_handlers: Vec<RequestHandlerMethod> = Vec::new();
    let mut page_methods: Vec<PageMethod> = Vec::new();
    let mut has_element = false;
    let mut has_on_start = false;
    let mut has_overlay = false;

    // Reconstructed methods for the impl block
    let mut reconstructed_methods: Vec<TokenStream> = Vec::new();

    for method in &partial_impl.methods {
        // Check for keybinds method
        if method.has_attr("keybinds") {
            keybinds_methods.push((
                KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope: KeybindScope::Global, // Systems don't support page scope
                },
                method.body.clone(),
            ));
            // Don't add keybinds methods to reconstructed output - they're consumed
            continue;
        }

        // Check for handler method
        if method.has_attr("handler") {
            let handler_info = extract_handler_info(&method.sig.ident, &method.sig);

            // Check for ModalContext usage - systems cannot use modal context
            if handler_info.contexts.modal_context {
                return syn::Error::new_spanned(
                    &method.sig,
                    "System handlers cannot use ModalContext. ModalContext is only available in modal handlers.",
                )
                .to_compile_error();
            }

            // Check for AppContext usage - systems only get GlobalContext
            if handler_info.contexts.app_context {
                return syn::Error::new_spanned(
                    &method.sig,
                    "System handlers cannot use AppContext. Systems only have access to GlobalContext.",
                )
                .to_compile_error();
            }

            handler_contexts.insert(method.sig.ident.to_string(), handler_info.contexts.clone());
            handler_infos.push(handler_info);
        }

        // For event/request handlers, we need to convert to ImplItemFn temporarily
        let reconstructed = reconstruct_method(method);
        if let Ok(impl_item) = syn::parse2::<syn::ImplItemFn>(reconstructed.clone()) {
            if method.has_attr("event_handler") {
                if let Some(event_handler) = parse_event_handler_metadata(&impl_item) {
                    event_handlers.push(event_handler);
                }
            }
            if method.has_attr("request_handler") {
                if let Some(request_handler) = parse_request_handler_metadata(&impl_item) {
                    request_handlers.push(request_handler);
                }
            }
        }

        // Check for page method - systems don't support #[page]
        if method.has_attr("page") {
            page_methods.push(PageMethod {
                name: method.sig.ident.clone(),
                page_name: None,
                body: method.body.clone(),
            });
        }

        // Check special methods using is_named()
        if method.is_named("element") {
            has_element = true;
        }
        if method.is_named("on_start") {
            has_on_start = true;
        }
        if method.is_named("overlay") {
            has_overlay = true;
        }

        // Add to reconstructed methods (with custom attrs stripped)
        reconstructed_methods.push(reconstruct_method_stripped(method));
    }

    // Systems don't support #[page] - they use overlays instead
    if !page_methods.is_empty() {
        return syn::Error::new_spanned(
            &partial_impl.self_ty,
            "Systems don't support #[page]. Use overlay() method instead.",
        )
        .to_compile_error();
    }

    // Generate trait method implementations
    let keybinds_impl =
        generate_keybinds_closures_impl(&keybinds_methods, &handler_contexts, &type_name);

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Generate handlers() method
    let handlers_impl = quote! {
        fn handlers(&self) -> &rafter::HandlerRegistry {
            &self.__handler_registry
        }
    };

    // Generate overlay method
    let overlay_impl = if has_overlay {
        quote! {
            fn overlay(&self) -> Option<rafter::Overlay> {
                #self_ty::overlay(self)
            }
        }
    } else if has_element {
        quote! {}
    } else {
        quote! {}
    };

    // Generate on_start method
    let on_start_impl = generate_async_lifecycle_impl("on_start", has_on_start, &self_ty);

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

    // Generate event/request dispatch methods
    let event_dispatch_impl = generate_event_dispatch(&event_handlers, DispatchContextType::System);
    let request_dispatch_impl = generate_request_dispatch(&request_handlers, DispatchContextType::System);

    // Generate handler wrapper methods
    let handler_wrappers = generate_handler_wrappers(&handler_infos);

    // Output the impl block plus System trait implementation
    let impl_generics = &partial_impl.generics;
    let impl_attrs = &partial_impl.attrs;

    // Suppress unused variable warning for has_element
    let _ = has_element;

    quote! {
        #(#impl_attrs)*
        impl #impl_generics #self_ty {
            #(#reconstructed_methods)*

            // Handler wrappers for overlay page! macro integration
            #handler_wrappers
        }

        impl #impl_generics rafter::System for #self_ty {
            #name_impl
            #keybinds_impl
            #handlers_impl
            #overlay_impl
            #on_start_impl
            #dirty_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }
    }
}
