//! Common utilities shared between app_impl and modal_impl macros.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ImplItem, ImplItemFn, ItemImpl, Type};

use super::handler::HandlerParams;

/// Keybind scope parsed from #[keybinds(view = X)] attribute
#[derive(Clone, Debug, Default)]
pub enum KeybindScope {
    /// No scope specified - global keybinds
    #[default]
    Global,
    /// View-scoped keybinds
    View(String),
}

/// Information about a keybinds method
pub struct KeybindsMethod {
    /// Method name
    pub name: Ident,
    /// Scope for these keybinds
    pub scope: KeybindScope,
}

/// Information about a handler method
pub struct HandlerMethod {
    /// Method name
    pub name: Ident,
    /// Handler parameter requirements
    pub params: HandlerParams,
    /// Handler is async
    pub is_async: bool,
}

/// Check if a method has the #[keybinds] attribute
pub fn is_keybinds_method(method: &ImplItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("keybinds"))
}

/// Check if method has #[handler] attribute and extract metadata
pub fn parse_handler_metadata(method: &ImplItemFn) -> Option<HandlerMethod> {
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
pub fn detect_handler_params_from_impl_fn(method: &ImplItemFn) -> HandlerParams {
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
pub fn is_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "view"
}

/// Extract the type name from a Type
pub fn get_type_name(ty: &Type) -> Option<Ident> {
    if let Type::Path(path) = ty {
        path.path.get_ident().cloned()
    } else {
        None
    }
}

/// Generate the metadata module name for an app
pub fn app_metadata_mod(type_name: &Ident) -> Ident {
    format_ident!(
        "__rafter_app_metadata_{}",
        type_name.to_string().to_lowercase()
    )
}

/// Generate the metadata module name for a modal
pub fn modal_metadata_mod(type_name: &Ident) -> Ident {
    format_ident!(
        "__rafter_modal_metadata_{}",
        type_name.to_string().to_lowercase()
    )
}

/// Strip custom rafter attributes from methods in an impl block
pub fn strip_custom_attrs(impl_block: &mut ItemImpl) {
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
}

/// Generate keybinds trait method implementation
pub fn generate_keybinds_impl(keybinds_methods: &[KeybindsMethod]) -> TokenStream {
    if keybinds_methods.is_empty() {
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
                match &m.scope {
                    KeybindScope::Global => {
                        quote! { __keybinds.merge(Self::#name()); }
                    }
                    KeybindScope::View(view_name) => {
                        quote! {
                            __keybinds.merge(
                                Self::#name().with_scope(
                                    rafter::keybinds::KeybindScope::View(#view_name.to_string())
                                )
                            );
                        }
                    }
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
}

/// Generate view trait method implementation
pub fn generate_view_impl(has_view: bool, self_ty: &Type) -> TokenStream {
    if has_view {
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
    }
}

/// Generate name trait method implementation
pub fn generate_name_impl(type_name: &Ident) -> TokenStream {
    let type_name_str = type_name.to_string();
    quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    }
}
