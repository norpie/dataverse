//! The `#[modal_impl]` attribute macro for implementing the Modal trait.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    AngleBracketedGenericArguments, GenericArgument, ImplItem, ImplItemFn, ItemImpl, PathArguments,
    Type, parse2,
};

use super::impl_common::{
    ContextParam, HandlerMethod, KeybindScope, KeybindsMethod, PageMethod,
    generate_element_impl, generate_keybinds_impl, generate_name_impl, get_type_name,
    is_element_method, is_keybinds_method, modal_metadata_mod, parse_handler_metadata,
    parse_page_metadata, strip_custom_attrs,
};

/// Attributes for the #[modal_impl] macro
struct ModalImplAttrs {
    /// The result type for this modal
    result_type: Option<Type>,
}

impl ModalImplAttrs {
    fn parse(attr: TokenStream) -> syn::Result<Self> {
        let mut result_type = None;

        if !attr.is_empty() {
            // Parse: Result = SomeType
            let meta: syn::Meta = parse2(attr)?;
            if let syn::Meta::NameValue(nv) = meta
                && nv.path.is_ident("Result")
                && let syn::Expr::Path(expr_path) = &nv.value
            {
                result_type = Some(Type::Path(syn::TypePath {
                    qself: None,
                    path: expr_path.path.clone(),
                }));
            }
        }

        Ok(Self { result_type })
    }
}

/// Check if method is named "position"
fn is_position_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "position"
}

/// Check if method is named "size"
fn is_size_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "size"
}

/// Try to extract the Result type from ModalContext<R> in method params
fn extract_result_type_from_handlers(methods: &[&ImplItemFn]) -> Option<Type> {
    for method in methods {
        for attr in &method.attrs {
            if attr.path().is_ident("handler") {
                // Look for ModalContext<T> in parameters
                for arg in &method.sig.inputs {
                    if let syn::FnArg::Typed(pat_type) = arg
                        && let Type::Reference(type_ref) = &*pat_type.ty
                        && let Type::Path(type_path) = &*type_ref.elem
                        && let Some(segment) = type_path.path.segments.last()
                        && segment.ident == "ModalContext"
                        && let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            args,
                            ..
                        }) = &segment.arguments
                        && let Some(GenericArgument::Type(result_ty)) = args.first()
                    {
                        return Some(result_ty.clone());
                    }
                }
            }
        }
    }
    None
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attrs = match ModalImplAttrs::parse(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };

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

    let metadata_mod = modal_metadata_mod(&type_name);

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut page_methods: Vec<PageMethod> = Vec::new();
    let mut has_element = false;
    let mut has_position = false;
    let mut has_size = false;

    // Collect methods for result type extraction
    let handler_methods: Vec<_> = impl_block
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(method) = item {
                Some(method)
            } else {
                None
            }
        })
        .collect();

    // Try to extract result type from handler signatures
    let inferred_result_type = extract_result_type_from_handlers(&handler_methods);

    // Determine the result type
    let result_type = attrs
        .result_type
        .or(inferred_result_type)
        .unwrap_or_else(|| syn::parse_quote!(()));

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if is_keybinds_method(method) {
                // Modals don't support page-scoped keybinds, all are global
                keybinds_methods.push(KeybindsMethod {
                    name: method.sig.ident.clone(),
                    scope: KeybindScope::Global,
                });
            }

            if let Some(handler) = parse_handler_metadata(method) {
                handlers.push(handler);
            }

            if let Some(page) = parse_page_metadata(method) {
                // For modals, we only support a single unnamed #[page] method
                if page.page_name.is_some() {
                    return syn::Error::new_spanned(
                        &method.sig.ident,
                        "Modals only support #[page], not #[page(Name)]. Use #[page] instead.",
                    )
                    .to_compile_error();
                }
                page_methods.push(page);
            }

            if is_element_method(method) {
                has_element = true;
            }

            if is_position_method(method) {
                has_position = true;
            }

            if is_size_method(method) {
                has_size = true;
            }
        }
    }

    // Validate page methods for modals
    if page_methods.len() > 1 {
        return syn::Error::new_spanned(
            &impl_block.self_ty,
            "Modals can only have one #[page] method",
        )
        .to_compile_error();
    }

    // TODO: In later chunks, we'll process page_methods to:
    // 1. Parse the DSL in the method body
    // 2. Generate handler closures with proper context extraction
    // 3. Replace the method body with generated code
    let _ = &page_methods; // Suppress unused warning for now

    // Strip our custom attributes from methods
    strip_custom_attrs(&mut impl_block);

    // Generate trait method implementations
    let keybinds_impl = generate_keybinds_impl(&keybinds_methods, &type_name);
    let element_impl = generate_element_impl(has_element, &self_ty);
    let name_impl = generate_name_impl(&type_name);

    // Generate position method
    let position_impl = if has_position {
        quote! {
            fn position(&self) -> rafter::ModalPosition {
                #self_ty::position(self)
            }
        }
    } else {
        quote! {}
    };

    // Generate size method
    let size_impl = if has_size {
        quote! {
            fn size(&self) -> rafter::ModalSize {
                #self_ty::size(self)
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

    // Generate dispatch method
    let dispatch_impl = generate_modal_dispatch(&handlers);

    // Output the impl block plus Modal trait implementation
    let impl_generics = &impl_block.generics;

    quote! {
        #impl_block

        impl #impl_generics rafter::Modal for #self_ty {
            type Result = #result_type;

            #name_impl
            #position_impl
            #size_impl
            #keybinds_impl
            #element_impl
            #dirty_impl
            #dispatch_impl
        }
    }
}

/// Generate dispatch method for modal handlers.
/// Modals receive ModalContext, AppContext, and GlobalContext.
/// Handlers can use any combination via varargs pattern.
fn generate_modal_dispatch(handlers: &[HandlerMethod]) -> TokenStream {
    if handlers.is_empty() {
        return quote! {
            fn dispatch(
                &self,
                _handler_id: &rafter::HandlerId,
                _mx: &rafter::ModalContext<Self::Result>,
                _cx: &rafter::AppContext,
                _gx: &rafter::GlobalContext,
            ) {
            }
        };
    }

    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();
            let call = generate_modal_handler_call(name, h, h.is_async);

            // Determine what to clone based on what the handler needs
            let mut clones = vec![quote! { let this = self.clone(); }];
            if h.contexts.modal_context {
                clones.push(quote! { let mx = mx.clone(); });
            }
            if h.contexts.app_context {
                clones.push(quote! { let cx = cx.clone(); });
            }
            if h.contexts.global_context {
                clones.push(quote! { let gx = gx.clone(); });
            }

            quote! {
                #name_str => {
                    #(#clones)*
                    tokio::spawn(async move {
                        #call
                    });
                }
            }
        })
        .collect();

    quote! {
        fn dispatch(
            &self,
            handler_id: &rafter::HandlerId,
            mx: &rafter::ModalContext<Self::Result>,
            cx: &rafter::AppContext,
            gx: &rafter::GlobalContext,
        ) {
            log::debug!("Dispatching modal handler: {}", handler_id.0);
            match handler_id.0.as_str() {
                #(#dispatch_arms)*
                other => {
                    log::warn!("Unknown modal handler: {}", other);
                }
            }
        }
    }
}

/// Generate the modal handler call with appropriate context parameters (varargs pattern).
/// For modals: can use ModalContext, AppContext, and/or GlobalContext in any combination.
/// Arguments are generated in the same order as they appear in the handler signature.
fn generate_modal_handler_call(
    name: &syn::Ident,
    handler: &HandlerMethod,
    is_async: bool,
) -> TokenStream {
    // Build args in the same order as they appear in the handler signature
    let args: Vec<TokenStream> = handler
        .contexts
        .param_order
        .iter()
        .map(|param| match param {
            ContextParam::App => quote! { &cx },
            ContextParam::Global => quote! { &gx },
            ContextParam::Modal => quote! { &mx },
        })
        .collect();

    let call = if args.is_empty() {
        quote! { this.#name() }
    } else {
        quote! { this.#name(#(#args),*) }
    };

    if is_async {
        quote! { #call.await; }
    } else {
        quote! { #call; }
    }
}
