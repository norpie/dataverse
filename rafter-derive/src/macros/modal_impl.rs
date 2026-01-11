//! The `#[modal_impl]` attribute macro for implementing the Modal trait.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, Type, parse2};

use super::impl_common::{
    HandlerContexts, HandlerInfo, KeybindScope, KeybindsMethod, PageMethod, PartialImplBlock,
    extract_handler_info, generate_async_lifecycle_impl, generate_element_impl,
    generate_handler_wrappers, generate_keybinds_closures_impl, generate_name_impl,
    get_type_name, modal_metadata_mod, reconstruct_method_stripped,
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
            // Parse: Result = Type (where Type can include generics like Option<String>)
            let parser = syn::meta::parser(|meta| {
                if meta.path.is_ident("Result") {
                    let _eq: syn::Token![=] = meta.input.parse()?;
                    let ty: Type = meta.input.parse()?;
                    result_type = Some(ty);
                }
                Ok(())
            });
            syn::parse::Parser::parse2(parser, attr)?;
        }

        Ok(Self { result_type })
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
    let mut has_element = false;
    let mut has_position = false;
    let mut has_size = false;
    let mut has_on_start = false;
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
            handler_contexts.insert(method.sig.ident.to_string(), handler_info.contexts.clone());
            handler_infos.push(handler_info);

            // Try to extract result type from ModalContext<R>
            if inferred_result_type.is_none() {
                inferred_result_type = extract_result_type_from_sig(&method.sig);
            }
        }

        // Check for page method
        if method.has_attr("page") {
            // For modals, we only support a single unnamed #[page] method
            let has_name = method.attrs.iter().any(|attr| {
                if attr.path().is_ident("page") {
                    matches!(&attr.meta, syn::Meta::List(_))
                } else {
                    false
                }
            });

            if has_name {
                return syn::Error::new_spanned(
                    &method.sig.ident,
                    "Modals only support #[page], not #[page(Name)]. Use #[page] instead.",
                )
                .to_compile_error();
            }

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
        if method.is_named("position") {
            has_position = true;
        }
        if method.is_named("size") {
            has_size = true;
        }
        if method.is_named("on_start") {
            has_on_start = true;
        }

        // Add to reconstructed methods (with custom attrs stripped)
        reconstructed_methods.push(reconstruct_method_stripped(method));
    }

    // Validate page methods for modals
    if page_methods.len() > 1 {
        return syn::Error::new_spanned(
            &partial_impl.self_ty,
            "Modals can only have one #[page] method",
        )
        .to_compile_error();
    }

    // TODO: Process page_methods for DSL parsing
    let _ = &page_methods;

    // Determine the result type
    let result_type = attrs
        .result_type
        .or(inferred_result_type)
        .unwrap_or_else(|| syn::parse_quote!(()));

    // Generate trait method implementations
    let keybinds_impl =
        generate_keybinds_closures_impl(&keybinds_methods, &handler_contexts, &type_name);
    let element_impl = generate_element_impl(has_element, &self_ty);
    let name_impl = generate_name_impl(&type_name);

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

    // Generate on_start method
    let on_start_impl = generate_async_lifecycle_impl("on_start", has_on_start, &self_ty);

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

    quote! {
        #(#impl_attrs)*
        impl #impl_generics #self_ty {
            #(#reconstructed_methods)*

            // Handler wrappers for page! macro integration
            #handler_wrappers
        }

        impl #impl_generics rafter::Modal for #self_ty {
            type Result = #result_type;

            #name_impl
            #position_impl
            #size_impl
            #on_start_impl
            #keybinds_impl
            #handlers_impl
            #element_impl
            #dirty_impl
        }
    }
}
