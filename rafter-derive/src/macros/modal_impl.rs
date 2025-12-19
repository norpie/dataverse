//! The `#[modal_impl]` attribute macro for implementing the Modal trait.
//!
//! This macro processes an impl block and generates:
//! - The `Modal` trait implementation
//! - Handler dispatch code
//! - Keybind collection

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    AngleBracketedGenericArguments, GenericArgument, Ident, ImplItem, ImplItemFn, ItemImpl,
    PathArguments, Type, parse2,
};

use super::handler::HandlerParams;

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

/// Information about a keybinds method
struct KeybindsMethod {
    /// Method name
    name: Ident,
}

/// Information about a handler method
struct HandlerMethod {
    /// Method name
    name: Ident,
    /// Handler parameter requirements
    params: HandlerParams,
    /// Handler is async
    is_async: bool,
}

/// Check if a method has the #[keybinds] attribute
fn is_keybinds_method(method: &ImplItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("keybinds"))
}

/// Check if method has #[handler] attribute and extract metadata
fn parse_handler_metadata(method: &ImplItemFn) -> Option<HandlerMethod> {
    for attr in &method.attrs {
        if attr.path().is_ident("handler") {
            let params = detect_handler_params_from_impl_fn(method);
            let is_async = method.sig.asyncness.is_some();

            return Some(HandlerMethod {
                name: method.sig.ident.clone(),
                params,
                is_async,
            });
        }
    }
    None
}

/// Detect handler params from an impl method
fn detect_handler_params_from_impl_fn(method: &ImplItemFn) -> HandlerParams {
    let mut has_app_context = false;
    let mut has_modal_context = false;

    for arg in &method.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            if ty_str.contains("AppContext") {
                has_app_context = true;
            }
            if ty_str.contains("ModalContext") {
                has_modal_context = true;
            }
        }
    }

    match (has_app_context, has_modal_context) {
        (false, false) => HandlerParams::None,
        (true, false) => HandlerParams::AppContext,
        (false, true) => HandlerParams::ModalContext,
        (true, true) => HandlerParams::Both,
    }
}

/// Check if method is named "view"
fn is_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "view"
}

/// Check if method is named "position"
fn is_position_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "position"
}

/// Check if method is named "size"
fn is_size_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "size"
}

/// Extract the type name from a Type
fn get_type_name(ty: &Type) -> Option<Ident> {
    if let Type::Path(path) = ty {
        path.path.get_ident().cloned()
    } else {
        None
    }
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
    let self_ty = &impl_block.self_ty;
    let type_name = match get_type_name(self_ty) {
        Some(n) => n,
        None => {
            return syn::Error::new_spanned(self_ty, "Expected a simple type name")
                .to_compile_error();
        }
    };

    let metadata_mod = format_ident!(
        "__rafter_modal_metadata_{}",
        type_name.to_string().to_lowercase()
    );

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut has_view = false;
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
        .unwrap_or_else(|| {
            // Default to () if no result type specified or found
            syn::parse_quote!(())
        });

    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if is_keybinds_method(method) {
                keybinds_methods.push(KeybindsMethod {
                    name: method.sig.ident.clone(),
                });
            }

            if let Some(handler) = parse_handler_metadata(method) {
                handlers.push(handler);
            }

            if is_view_method(method) {
                has_view = true;
            }

            if is_position_method(method) {
                has_position = true;
            }

            if is_size_method(method) {
                has_size = true;
            }
        }
    }

    // Strip our custom attributes from methods
    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            method
                .attrs
                .retain(|a| !a.path().is_ident("keybinds") && !a.path().is_ident("handler"));
            // Remove metadata doc attributes
            method.attrs.retain(|a| {
                if a.path().is_ident("doc")
                    && let syn::Meta::NameValue(nv) = &a.meta
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                {
                    return !s.value().starts_with("__rafter_handler:");
                }
                true
            });
        }
    }

    // Generate keybinds method
    let keybinds_impl = if keybinds_methods.is_empty() {
        quote! {
            fn keybinds(&self) -> rafter::keybinds::Keybinds {
                rafter::keybinds::Keybinds::new()
            }
        }
    } else {
        let merge_calls: Vec<_> = keybinds_methods
            .iter()
            .map(|m| {
                let name = &m.name;
                quote! { __keybinds.merge(Self::#name()); }
            })
            .collect();

        quote! {
            fn keybinds(&self) -> rafter::keybinds::Keybinds {
                let mut __keybinds = rafter::keybinds::Keybinds::new();
                #(#merge_calls)*
                __keybinds
            }
        }
    };

    // Generate view method
    let view_impl = if has_view {
        quote! {
            fn view(&self) -> rafter::node::Node {
                #self_ty::view(self)
            }
        }
    } else {
        quote! {
            fn view(&self) -> rafter::node::Node {
                rafter::node::Node::empty()
            }
        }
    };

    // Generate position method
    let position_impl = if has_position {
        quote! {
            fn position(&self) -> rafter::modal::ModalPosition {
                #self_ty::position(self)
            }
        }
    } else {
        quote! {
            fn position(&self) -> rafter::modal::ModalPosition {
                rafter::modal::ModalPosition::default()
            }
        }
    };

    // Generate size method
    let size_impl = if has_size {
        quote! {
            fn size(&self) -> rafter::modal::ModalSize {
                #self_ty::size(self)
            }
        }
    } else {
        quote! {
            fn size(&self) -> rafter::modal::ModalSize {
                rafter::modal::ModalSize::default()
            }
        }
    };

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Generate clear_dirty method
    let dirty_impl = quote! {
        fn clear_dirty(&self) {
            #metadata_mod::clear_dirty(self)
        }
    };

    // Generate dispatch method - spawns async tasks for handlers
    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();

            // Generate the call based on what parameters the handler needs
            let call = match h.params {
                HandlerParams::None => {
                    if h.is_async {
                        quote! { this.#name().await; }
                    } else {
                        quote! { this.#name(); }
                    }
                }
                HandlerParams::AppContext => {
                    if h.is_async {
                        quote! { this.#name(&cx).await; }
                    } else {
                        quote! { this.#name(&cx); }
                    }
                }
                HandlerParams::ModalContext => {
                    if h.is_async {
                        quote! { this.#name(&mx).await; }
                    } else {
                        quote! { this.#name(&mx); }
                    }
                }
                HandlerParams::Both => {
                    if h.is_async {
                        quote! { this.#name(&cx, &mx).await; }
                    } else {
                        quote! { this.#name(&cx, &mx); }
                    }
                }
            };

            // Determine what we need to clone
            let needs_cx = h.params.needs_app_context();
            let needs_mx = h.params.needs_modal_context();

            match (needs_cx, needs_mx) {
                (false, false) => {
                    quote! {
                        #name_str => {
                            let this = self.clone();
                            tokio::spawn(async move {
                                #call
                            });
                        }
                    }
                }
                (true, false) => {
                    quote! {
                        #name_str => {
                            let this = self.clone();
                            let cx = cx.clone();
                            tokio::spawn(async move {
                                #call
                            });
                        }
                    }
                }
                (false, true) => {
                    quote! {
                        #name_str => {
                            let this = self.clone();
                            let mx = mx.clone();
                            tokio::spawn(async move {
                                #call
                            });
                        }
                    }
                }
                (true, true) => {
                    quote! {
                        #name_str => {
                            let this = self.clone();
                            let cx = cx.clone();
                            let mx = mx.clone();
                            tokio::spawn(async move {
                                #call
                            });
                        }
                    }
                }
            }
        })
        .collect();

    let dispatch_impl = if handlers.is_empty() {
        quote! {
            fn dispatch(&self, _handler_id: &rafter::keybinds::HandlerId, _cx: &rafter::context::AppContext, _mx: &rafter::modal::ModalContext<Self::Result>) {
            }
        }
    } else {
        quote! {
            fn dispatch(&self, handler_id: &rafter::keybinds::HandlerId, cx: &rafter::context::AppContext, mx: &rafter::modal::ModalContext<Self::Result>) {
                log::debug!("Dispatching modal handler: {}", handler_id.0);
                match handler_id.0.as_str() {
                    #(#dispatch_arms)*
                    other => {
                        log::warn!("Unknown modal handler: {}", other);
                    }
                }
            }
        }
    };

    // Output the impl block plus Modal trait implementation
    let impl_generics = &impl_block.generics;

    quote! {
        #impl_block

        impl #impl_generics rafter::modal::Modal for #self_ty {
            type Result = #result_type;

            #name_impl
            #position_impl
            #size_impl
            #keybinds_impl
            #view_impl
            #dirty_impl
            #dispatch_impl
        }
    }
}
