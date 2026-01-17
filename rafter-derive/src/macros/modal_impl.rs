//! The `#[modal_impl]` attribute macro for implementing the Modal trait.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, Type, parse2};

use super::impl_common::{
    HandlerContexts, HandlerInfo, KeybindScope, KeybindsMethod, LifecycleContext,
    LifecycleHooksDefined, LifecycleHookInfo, PageMethod, PartialImplBlock, extract_handler_info,
    extract_lifecycle_hook_info, generate_element_impl, generate_handler_wrappers,
    generate_keybinds_closures_impl, generate_lifecycle_hooks_impl, generate_name_impl,
    get_type_name, modal_metadata_mod, reconstruct_method_stripped, validate_lifecycle_hook_contexts,
};

/// Modal kind for compile-time context checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ModalKindAttr {
    /// App-scoped modal (default) - has access to AppContext.
    #[default]
    App,
    /// System modal - only has access to GlobalContext.
    System,
}

/// Attributes for the #[modal_impl] macro
struct ModalImplAttrs {
    /// The result type for this modal
    result_type: Option<Type>,
    /// The modal kind (App or System)
    kind: ModalKindAttr,
    /// Layout method name for page routing (e.g., `layout = layout`)
    layout: Option<syn::Ident>,
}

impl ModalImplAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result_type = None;
        let mut kind = ModalKindAttr::default();
        let mut layout = None;

        if !attr.is_empty() {
            // Parse: Result = Type, kind = System/App, layout = method_name
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("Result") {
                    let _eq: syn::Token![=] = meta.input.parse()?;
                    let ty: Type = meta.input.parse()?;
                    result_type = Some(ty);
                } else if meta.path.is_ident("kind") {
                    let _eq: syn::Token![=] = meta.input.parse()?;
                    let ident: syn::Ident = meta.input.parse()?;
                    match ident.to_string().as_str() {
                        "System" => kind = ModalKindAttr::System,
                        "App" => kind = ModalKindAttr::App,
                        _ => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                "Expected 'App' or 'System'",
                            ))
                        }
                    }
                } else if meta.path.is_ident("layout") {
                    let _eq: syn::Token![=] = meta.input.parse()?;
                    let ident: syn::Ident = meta.input.parse()?;
                    layout = Some(ident);
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        Ok(Self { result_type, kind, layout })
    }
}

/// Try to extract the Result type from ModalContext<R> in method signature
fn extract_result_type_from_sig(sig: &syn::Signature) -> Option<Type> {
    for arg in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg
            && let Type::Reference(type_ref) = &*pat_type.ty
            && let Type::Path(type_path) = &*type_ref.elem
            && let Some(segment) = type_path.path.segments.last()
            && segment.ident == "ModalContext"
            && let PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) =
                &segment.arguments
            && let Some(GenericArgument::Type(result_ty)) = args.first()
        {
            return Some(result_ty.clone());
        }
    }
    None
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match ModalImplAttrs::parse(attr) {
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

    let metadata_mod = modal_metadata_mod(&type_name);

    // Collect method information
    let mut keybinds_methods: Vec<(KeybindsMethod, TokenStream)> = Vec::new();
    let mut handler_contexts: HashMap<String, HandlerContexts> = HashMap::new();
    let mut handler_infos: Vec<HandlerInfo> = Vec::new();
    let mut page_methods: Vec<PageMethod> = Vec::new();
    let mut lifecycle_hooks = LifecycleHooksDefined::default();
    let mut has_element = false;
    let mut has_position = false;
    let mut has_size = false;
    let mut inferred_result_type: Option<Type> = None;

    // Reconstructed methods for the impl block
    let mut reconstructed_methods: Vec<TokenStream> = Vec::new();

    for method in &partial_impl.methods {
        // Check for keybinds method
        if method.has_attr("keybinds") {
            keybinds_methods.push((
                KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope: KeybindScope::Global, // Modals don't support page scope
                },
                method.body.clone(),
            ));
            // Don't add keybinds methods to reconstructed output - they're consumed
            continue;
        }

        // Check for handler method
        if method.has_attr("handler") {
            let handler_info = extract_handler_info(&method.sig.ident, &method.sig);

            // For system modals, validate that handlers don't use AppContext
            if attrs.kind == ModalKindAttr::System && handler_info.contexts.app_context {
                return syn::Error::new_spanned(
                    &method.sig,
                    "System modal handlers cannot use AppContext. System modals only have access to GlobalContext and ModalContext.",
                )
                .to_compile_error();
            }

            handler_contexts.insert(method.sig.ident.to_string(), handler_info.contexts.clone());
            handler_infos.push(handler_info);

            // Try to extract result type from ModalContext<R>
            if inferred_result_type.is_none() {
                inferred_result_type = extract_result_type_from_sig(&method.sig);
            }
        }

        // Check for page method
        if method.has_attr("page") {
            // Extract page name from #[page(Variant)] attribute
            let page_name = method.attrs.iter().find_map(|attr| {
                if attr.path().is_ident("page") {
                    match &attr.meta {
                        syn::Meta::Path(_) => None, // #[page] without name
                        syn::Meta::List(list) => {
                            // #[page(Variant)] - parse the variant name
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
        let lifecycle_context = match attrs.kind {
            ModalKindAttr::App => LifecycleContext::AppModal,
            ModalKindAttr::System => LifecycleContext::SystemModal,
        };
        if method.has_attr("on_start") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, lifecycle_context, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_start.push(hook_info);
        }
        if method.has_attr("on_foreground") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, lifecycle_context, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_foreground.push(hook_info);
        }
        if method.has_attr("on_background") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, lifecycle_context, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_background.push(hook_info);
        }
        if method.has_attr("on_close") {
            let hook_info = extract_lifecycle_hook_info(&method.sig);
            if let Err(e) = validate_lifecycle_hook_contexts(&hook_info, lifecycle_context, &method.sig) {
                return e.to_compile_error();
            }
            lifecycle_hooks.on_close.push(hook_info);
        }

        // Check special methods using is_named()
        if method.is_named("element") {
            has_element = true;
        }
        if method.is_named("position") {
            has_position = true;
        }
        if method.is_named("size") {
            has_size = true;
        }

        // Add to reconstructed methods (with custom attrs stripped)
        reconstructed_methods.push(reconstruct_method_stripped(method));
    }

    // Collect page methods with named variants for page routing
    let named_page_methods: Vec<_> = page_methods
        .iter()
        .filter(|p| p.page_name.is_some())
        .collect();

    // Check if page routing is enabled (based on metadata)
    let has_page_routing = !named_page_methods.is_empty();

    // Determine the result type
    let result_type = attrs
        .result_type
        .or(inferred_result_type)
        .unwrap_or_else(|| syn::parse_quote!(()));

    // Generate trait method implementations
    let keybinds_impl =
        generate_keybinds_closures_impl(&keybinds_methods, &handler_contexts, &type_name);

    // Generate element impl - use page routing if enabled
    let element_impl = if has_page_routing {
        generate_page_routing_element_impl(&named_page_methods, &attrs.layout, &self_ty)
    } else {
        generate_element_impl(has_element, &self_ty)
    };

    let name_impl = generate_name_impl(&type_name);

    // Generate page routing helper methods if page routing is enabled
    let page_routing_helpers = if has_page_routing {
        generate_page_routing_helpers(&named_page_methods, &self_ty)
    } else {
        quote! {}
    };

    // Generate handlers() method
    let handlers_impl = quote! {
        fn handlers(&self) -> &rafter::HandlerRegistry {
            &self.__handler_registry
        }
    };

    // Generate position method
    // Priority: user-defined method > attribute from #[modal] > trait default
    let position_impl = if has_position {
        // User defined position() method
        quote! {
            fn position(&self) -> rafter::ModalPosition {
                #self_ty::position(self)
            }
        }
    } else {
        // Use attribute from #[modal] if available, otherwise trait default
        // We generate code that checks at compile time via the HAS_POSITION const
        quote! {
            fn position(&self) -> rafter::ModalPosition {
                #metadata_mod::position()
            }
        }
    };

    // Generate size method
    // Priority: user-defined method > attribute from #[modal] > trait default
    let size_impl = if has_size {
        // User defined size() method
        quote! {
            fn size(&self) -> rafter::ModalSize {
                #self_ty::size(self)
            }
        }
    } else {
        // Use attribute from #[modal] if available, otherwise trait default
        quote! {
            fn size(&self) -> rafter::ModalSize {
                #metadata_mod::size()
            }
        }
    };

    // Generate kind method
    let kind_impl = match attrs.kind {
        ModalKindAttr::App => quote! {
            fn kind(&self) -> rafter::ModalKind {
                rafter::ModalKind::App
            }
        },
        ModalKindAttr::System => quote! {
            fn kind(&self) -> rafter::ModalKind {
                rafter::ModalKind::System
            }
        },
    };

    // Generate lifecycle_hooks method
    let lifecycle_hooks_impl = generate_lifecycle_hooks_impl(
        &lifecycle_hooks,
        match attrs.kind {
            ModalKindAttr::App => LifecycleContext::AppModal,
            ModalKindAttr::System => LifecycleContext::SystemModal,
        },
        &self_ty,
    );

    // Generate dirty methods
    let dirty_impl = quote! {
        fn is_dirty(&self) -> bool {
            #metadata_mod::is_dirty(self)
        }

        fn clear_dirty(&self) {
            #metadata_mod::clear_dirty(self)
        }
    };

    // Generate handler wrapper methods
    let handler_wrappers = generate_handler_wrappers(&handler_infos);

    // Output the impl block plus Modal trait implementation
    let impl_generics = &partial_impl.generics;
    let impl_attrs = &partial_impl.attrs;

    // Generate SystemModal impl for system modals
    let system_modal_impl = if attrs.kind == ModalKindAttr::System {
        quote! {
            impl #impl_generics rafter::SystemModal for #self_ty {}
        }
    } else {
        quote! {}
    };

    quote! {
        #(#impl_attrs)*
        impl #impl_generics #self_ty {
            #(#reconstructed_methods)*

            // Handler wrappers for page! macro integration
            #handler_wrappers

            // Page routing helpers (if page routing is enabled)
            #page_routing_helpers
        }

        impl #impl_generics rafter::Modal for #self_ty {
            type Result = #result_type;

            #name_impl
            #kind_impl
            #position_impl
            #size_impl
            #lifecycle_hooks_impl
            #keybinds_impl
            #handlers_impl
            #element_impl
            #dirty_impl
        }

        #system_modal_impl
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
