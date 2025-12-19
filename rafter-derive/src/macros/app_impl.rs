use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Ident, ImplItem, ImplItemFn, ItemImpl, Type, parse2};

/// Attributes for the #[app_impl] macro
struct AppImplAttrs {
    // Currently no attributes supported
}

impl AppImplAttrs {
    fn parse(_attr: TokenStream) -> syn::Result<Self> {
        Ok(Self {})
    }
}

/// Information about a keybinds method
struct KeybindsMethod {
    /// Method name
    name: Ident,
    /// Scope (None = app-level, Some(view) = view-scoped)
    #[allow(dead_code)]
    scope: Option<KeybindScope>,
}

#[derive(Clone)]
#[allow(dead_code)]
enum KeybindScope {
    View(Ident),
    Modal(Ident),
    Global,
}

/// Information about a handler method
#[allow(dead_code)]
struct HandlerMethod {
    /// Method name
    name: Ident,
    /// Is async handler
    is_async: bool,
    /// Supersedes previous calls
    supersedes: bool,
    /// Queues calls
    queues: bool,
    /// Debounce milliseconds
    debounce_ms: u64,
    /// Handler takes context parameter
    has_context: bool,
}

/// Check if a method has the #[keybinds] attribute
fn is_keybinds_method(method: &ImplItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("keybinds"))
}

/// Parse keybinds scope from attributes
fn parse_keybinds_scope(attrs: &[Attribute]) -> Option<KeybindScope> {
    for attr in attrs {
        if attr.path().is_ident("keybinds") {
            // Try to parse scope: #[keybinds(view = SomeView)] or #[keybinds(global)]
            {
                let meta: syn::Meta = attr.meta.clone();
                if let syn::Meta::List(list) = meta {
                    let mut scope = None;
                    let _ = list.parse_nested_meta(|meta| {
                        if meta.path.is_ident("view") {
                            let value: syn::Expr = meta.value()?.parse()?;
                            if let syn::Expr::Path(path) = value
                                && let Some(ident) = path.path.get_ident()
                            {
                                scope = Some(KeybindScope::View(ident.clone()));
                            }
                        } else if meta.path.is_ident("modal") {
                            let value: syn::Expr = meta.value()?.parse()?;
                            if let syn::Expr::Path(path) = value
                                && let Some(ident) = path.path.get_ident()
                            {
                                scope = Some(KeybindScope::Modal(ident.clone()));
                            }
                        } else if meta.path.is_ident("global") {
                            scope = Some(KeybindScope::Global);
                        }
                        Ok(())
                    });
                    return scope;
                }
            }
        }
    }
    None
}

/// Check if method has #[handler] attribute and extract metadata
fn parse_handler_metadata(method: &ImplItemFn) -> Option<HandlerMethod> {
    for attr in &method.attrs {
        if attr.path().is_ident("handler") {
            // Found a #[handler] attribute
            let is_async = method.sig.asyncness.is_some();

            // Check if method takes context parameter
            let has_context = method.sig.inputs.iter().any(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                    let ty = &pat_type.ty;
                    let ty_str = quote::quote!(#ty).to_string();
                    ty_str.contains("AppContext") || ty_str.contains("Context")
                } else {
                    false
                }
            });

            // Parse handler attributes (supersedes, queues, debounce)
            let mut supersedes = false;
            let mut queues = false;
            let mut debounce_ms = 0u64;

            if let syn::Meta::List(list) = &attr.meta {
                let _ = list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("supersedes") {
                        supersedes = true;
                    } else if meta.path.is_ident("queues") {
                        queues = true;
                    } else if meta.path.is_ident("debounce")
                        && let Ok(value) = meta.value()
                        && let Ok(lit) = value.parse::<syn::LitInt>()
                    {
                        debounce_ms = lit.base10_parse().unwrap_or(0);
                    }
                    Ok(())
                });
            }

            return Some(HandlerMethod {
                name: method.sig.ident.clone(),
                is_async,
                supersedes,
                queues,
                debounce_ms,
                has_context,
            });
        }
    }
    None
}

/// Check if method is named "view"
fn is_view_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "view"
}

/// Check if method is named "view_with_focus"
fn is_view_with_focus_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "view_with_focus"
}

/// Check if method is named "focusable_ids"
fn is_focusable_ids_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "focusable_ids"
}

/// Check if method is named "captures_input"
fn is_captures_input_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "captures_input"
}

/// Extract the type name from a Type
fn get_type_name(ty: &Type) -> Option<Ident> {
    if let Type::Path(path) = ty {
        path.path.get_ident().cloned()
    } else {
        None
    }
}

pub fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _attrs = match AppImplAttrs::parse(attr) {
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
        "__rafter_app_metadata_{}",
        type_name.to_string().to_lowercase()
    );

    // Collect method information
    let mut keybinds_methods = Vec::new();
    let mut handlers = Vec::new();
    let mut has_view = false;
    let mut has_view_with_focus = false;
    let mut has_focusable_ids = false;
    let mut has_captures_input = false;

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

            if is_view_with_focus_method(method) {
                has_view_with_focus = true;
            }

            if is_focusable_ids_method(method) {
                has_focusable_ids = true;
            }

            if is_captures_input_method(method) {
                has_captures_input = true;
            }
        }
    }

    // Strip our custom attributes from methods
    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            method
                .attrs
                .retain(|a| !a.path().is_ident("keybinds") && !a.path().is_ident("handler"));
            // Remove our metadata doc attributes (legacy, may not be needed anymore)
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

    // Generate view method (delegate to user's view if present)
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

    // Generate view_with_focus method (delegate to user's implementation if present)
    let view_with_focus_impl = if has_view_with_focus {
        quote! {
            fn view_with_focus(&self, focus: &rafter::focus::FocusState) -> rafter::node::Node {
                #self_ty::view_with_focus(self, focus)
            }
        }
    } else {
        quote! {}
    };

    // Generate focusable_ids method (delegate to user's implementation if present)
    let focusable_ids_impl = if has_focusable_ids {
        quote! {
            fn focusable_ids(&self) -> Vec<String> {
                #self_ty::focusable_ids(self)
            }
        }
    } else {
        quote! {}
    };

    // Generate captures_input method (delegate to user's implementation if present)
    let captures_input_impl = if has_captures_input {
        quote! {
            fn captures_input(&self, id: &str) -> bool {
                #self_ty::captures_input(self, id)
            }
        }
    } else {
        quote! {}
    };

    // Generate name method
    let type_name_str = type_name.to_string();
    let name_impl = quote! {
        fn name(&self) -> &'static str {
            #type_name_str
        }
    };

    // Generate is_dirty and clear_dirty methods
    let dirty_impl = quote! {
        fn is_dirty(&self) -> bool {
            #metadata_mod::is_dirty(self)
        }

        fn clear_dirty(&mut self) {
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
    let dispatch_arms: Vec<_> = handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let name_str = name.to_string();
            if h.is_async {
                // Async handlers need special treatment - for now, skip
                quote! {
                    #name_str => {
                        // TODO: async handler dispatch
                    }
                }
            } else if h.has_context {
                quote! {
                    #name_str => {
                        self.#name(cx);
                    }
                }
            } else {
                quote! {
                    #name_str => {
                        self.#name();
                    }
                }
            }
        })
        .collect();

    let dispatch_impl = if handlers.is_empty() {
        quote! {}
    } else {
        quote! {
            fn dispatch(&mut self, handler_id: &rafter::keybinds::HandlerId, cx: &mut rafter::context::AppContext) {
                log::debug!("Dispatching handler: {}", handler_id.0);
                match handler_id.0.as_str() {
                    #(#dispatch_arms)*
                    other => {
                        log::warn!("Unknown handler: {}", other);
                    }
                }
            }
        }
    };

    // Output the impl block plus App trait implementation
    let impl_generics = &impl_block.generics;

    // Determine which methods the user already implements
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
            #view_with_focus_impl
            #focusable_ids_impl
            #captures_input_impl
            #dirty_impl
            #panic_impl
            #dispatch_impl
        }
    }
}
