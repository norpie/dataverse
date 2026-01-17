//! The `#[app_impl]` attribute macro for implementing the App trait.
//!
//! Supports attributes:
//! - `#[app_impl]` - basic implementation
//! - `#[app_impl(layout = method_name)]` - specify layout wrapper for page routing

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Type, parse2};

use super::impl_common::{
    DispatchContextType, EventHandlerMethod, HandlerContexts, HandlerInfo, KeybindScope,
    KeybindsMethod, LifecycleContext, LifecycleHooksDefined, LifecycleHookInfo, PageMethod,
    PartialImplBlock, RequestHandlerMethod, app_metadata_mod, extract_handler_info,
    extract_lifecycle_hook_info, generate_config_impl, generate_element_impl,
    generate_event_dispatch, generate_handler_wrappers, generate_keybinds_closures_impl,
    generate_lifecycle_hooks_impl, generate_request_dispatch, get_type_name,
    parse_event_handler_metadata, parse_request_handler_metadata, reconstruct_method,
    reconstruct_method_stripped, validate_lifecycle_hook_contexts,
};

/// Attributes for the #[app_impl] macro
struct AppImplAttrs {
    /// Layout method name for page routing (e.g., `layout = layout`)
    layout: Option<syn::Ident>,
}

impl AppImplAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut layout = None;

        if !attr.is_empty() {
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("layout") {
                    let _eq: syn::Token![=] = meta.input.parse()?;
                    let ident: syn::Ident = meta.input.parse()?;
                    layout = Some(ident);
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        Ok(Self { layout })
    }
}

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
    // Parse attributes
    let attrs = match AppImplAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

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
    let mut lifecycle_hooks = LifecycleHooksDefined::default();
    let mut has_element = false;
    let mut has_title = false;
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

        // Check for lifecycle hook attributes
        if method.has_attr("on_start") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, LifecycleContext::App, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_start.push(hook_info);
        }
        if method.has_attr("on_foreground") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, LifecycleContext::App, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_foreground.push(hook_info);
        }
        if method.has_attr("on_background") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, LifecycleContext::App, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_background.push(hook_info);
        }
        if method.has_attr("on_close") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, LifecycleContext::App, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_close.push(hook_info);
        }

        // Check special methods using is_named()
        if method.is_named("element") {
            has_element = true;
        }
        if method.is_named("title") {
            has_title = true;
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

    // Collect page methods with named variants for page routing
    let named_page_methods: Vec<_> = page_methods
        .iter()
        .filter(|p| p.page_name.is_some())
        .collect();

    // Check if page routing is enabled (based on having named page methods)
    let has_page_routing = !named_page_methods.is_empty();

    // Generate trait method implementations
    let keybinds_impl =
        generate_keybinds_closures_impl(&keybinds_methods, &handler_contexts, &type_name);

    // Generate element impl - use page routing if enabled
    let element_impl = if has_page_routing {
        generate_page_routing_element_impl(&named_page_methods, &attrs.layout, &self_ty)
    } else {
        generate_element_impl(has_element, &self_ty)
    };

    // Generate page routing helper methods if page routing is enabled
    let page_routing_helpers = if has_page_routing {
        generate_page_routing_helpers(&named_page_methods, &self_ty)
    } else {
        quote! {}
    };

    let config_impl = generate_config_impl(&type_name);

    let title_impl = if has_title {
        quote! {
            fn title(&self) -> String {
                #self_ty::title(self)
            }
        }
    } else {
        quote! {}
    };

    // Generate handlers() method
    let handlers_impl = quote! {
        fn handlers(&self) -> &rafter::HandlerRegistry {
            &self.__handler_registry
        }
    };

    // Generate lifecycle_hooks method
    let lifecycle_hooks_impl = generate_lifecycle_hooks_impl(
        &lifecycle_hooks,
        LifecycleContext::App,
        &self_ty,
    );

    // Generate current_page impl - use page routing generated version if enabled
    let current_page_impl = if has_page_routing {
        // Page routing generates its own current_page method, use it
        quote! {
            fn current_page(&self) -> Option<String> {
                #self_ty::current_page(self)
            }
        }
    } else if has_current_page {
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

            // Page routing helpers (if page routing is enabled)
            #page_routing_helpers
        }

        impl #impl_generics rafter::App for #self_ty {
            #config_impl
            #title_impl
            #keybinds_impl
            #handlers_impl
            #element_impl
            #current_page_impl
            #lifecycle_hooks_impl
            #dirty_impl
            #panic_impl
            #event_dispatch_impl
            #request_dispatch_impl
        }
    }
}

/// Generate element() implementation with page routing.
///
/// Generates code like:
/// ```ignore
/// fn element(&self) -> tuidom::Element {
///     let content = match self.__page.get() {
///         Page::Active => self.active_tab(),
///         Page::Environments => self.environments_tab(),
///     };
///     self.layout(content)  // if layout is specified
/// }
/// ```
fn generate_page_routing_element_impl(
    page_methods: &[&PageMethod],
    layout: &Option<syn::Ident>,
    self_ty: &Type,
) -> TokenStream {
    // Generate match arms for each page
    let match_arms: Vec<TokenStream> = page_methods
        .iter()
        .map(|page| {
            let method_name = &page.name;
            let variant_name = page.page_name.as_ref().expect("page_name should be Some for named pages");
            let variant_ident = syn::Ident::new(variant_name, proc_macro2::Span::call_site());
            quote! {
                Page::#variant_ident => #self_ty::#method_name(self),
            }
        })
        .collect();

    let content_expr = quote! {
        match self.__page.get() {
            #(#match_arms)*
        }
    };

    // Wrap with layout if specified
    let final_expr = if let Some(layout_method) = layout {
        quote! {
            let content = #content_expr;
            #self_ty::#layout_method(self, content)
        }
    } else {
        content_expr
    };

    quote! {
        fn element(&self) -> tuidom::Element {
            #final_expr
        }
    }
}

/// Generate page routing helper methods.
///
/// Generates:
/// - `page(&self) -> Page` - getter for current page
/// - `navigate(&self, page: Page)` - setter for navigation
/// - `current_page(&self) -> Option<String>` - for keybind scoping
fn generate_page_routing_helpers(
    page_methods: &[&PageMethod],
    _self_ty: &Type,
) -> TokenStream {
    // Just check that we have pages to validate
    if page_methods.is_empty() {
        return quote! {};
    }

    quote! {
        /// Get the current page.
        pub fn page(&self) -> Page {
            self.__page.get()
        }

        /// Navigate to a different page.
        pub fn navigate(&self, page: Page) {
            self.__page.set(page);
        }

        /// Get the current page name as a string (for keybind scoping).
        fn current_page(&self) -> Option<String> {
            Some(format!("{:?}", self.__page.get()))
        }
    }
}
