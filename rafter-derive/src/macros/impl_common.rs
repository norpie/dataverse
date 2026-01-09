//! Common utilities shared between app_impl, modal_impl, and system_impl macros.

use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Generics, Ident, ImplItem, ImplItemFn, ItemImpl, Path, Signature, Token, Type,
    Visibility,
    parse::{Parse, ParseStream},
    token::Brace,
};

// =============================================================================
// Partial Parsing Types
// =============================================================================

/// Partially parsed impl block - signatures parsed, bodies kept as raw TokenStream.
///
/// This allows us to parse method signatures (for context detection) while keeping
/// method bodies as raw tokens that can be parsed as DSL instead of Rust code.
pub struct PartialImplBlock {
    pub attrs: Vec<Attribute>,
    pub generics: Generics,
    pub trait_: Option<(Option<Token![!]>, Path, Token![for])>,
    pub self_ty: Box<Type>,
    pub methods: Vec<PartialMethod>,
}

impl Parse for PartialImplBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        input.parse::<Token![impl]>()?;

        // Parse generics (before trait/type)
        let mut generics: Generics = input.parse()?;

        // Parse optional trait: `!Trait for` or `Trait for`
        let trait_ = if input.peek(Token![!]) || {
            // Look ahead to see if there's a `for` after a path
            let fork = input.fork();
            fork.parse::<Path>().is_ok() && fork.peek(Token![for])
        } {
            let polarity: Option<Token![!]> = input.parse()?;
            let path: Path = input.parse()?;
            let for_token: Token![for] = input.parse()?;
            Some((polarity, path, for_token))
        } else {
            None
        };

        let self_ty: Type = input.parse()?;

        // Parse where clause if present
        generics.where_clause = input.parse()?;

        let content;
        syn::braced!(content in input);

        let mut methods = Vec::new();
        while !content.is_empty() {
            methods.push(content.parse()?);
        }

        Ok(Self {
            attrs,
            generics,
            trait_,
            self_ty: Box::new(self_ty),
            methods,
        })
    }
}

/// Partially parsed method - signature fully parsed, body kept as raw TokenStream.
pub struct PartialMethod {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub body: TokenStream,
    pub _brace: Brace,
}

impl Parse for PartialMethod {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis: Visibility = input.parse()?;
        let sig: Signature = input.parse()?;

        // Collect body tokens WITHOUT parsing as Rust statements
        let content;
        let brace = syn::braced!(content in input);
        let body: TokenStream = content.parse()?;

        Ok(Self {
            attrs,
            vis,
            sig,
            body,
            _brace: brace,
        })
    }
}

impl PartialMethod {
    /// Check if this method has a specific attribute
    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|a| a.path().is_ident(name))
    }

    /// Check if this method is named a specific name
    pub fn is_named(&self, name: &str) -> bool {
        self.sig.ident == name
    }
}

// =============================================================================
// Method Reconstruction
// =============================================================================

/// Reconstruct a method from PartialMethod back to valid TokenStream
pub fn reconstruct_method(method: &PartialMethod) -> TokenStream {
    let attrs = &method.attrs;
    let vis = &method.vis;
    let sig = &method.sig;
    let body = &method.body;

    quote! {
        #(#attrs)*
        #vis #sig {
            #body
        }
    }
}

/// Custom attributes to strip from methods
const CUSTOM_ATTRS: &[&str] = &[
    "keybinds",
    "handler",
    "event_handler",
    "request_handler",
    "page",
];

/// Strip custom attributes from a PartialMethod and return as TokenStream
pub fn reconstruct_method_stripped(method: &PartialMethod) -> TokenStream {
    let attrs: Vec<_> = method
        .attrs
        .iter()
        .filter(|a| !CUSTOM_ATTRS.iter().any(|name| a.path().is_ident(name)))
        .collect();

    let vis = &method.vis;
    let sig = &method.sig;
    let body = &method.body;

    quote! {
        #(#attrs)*
        #vis #sig {
            #body
        }
    }
}

/// Detect which contexts a handler method needs from its signature.
/// Works with PartialMethod's Signature.
///
/// Note: All handlers are assumed to be async. The asyncness of the signature
/// is not checked here - handlers that aren't async will fail at compile time.
pub fn detect_handler_contexts_from_sig(sig: &Signature) -> HandlerContexts {
    let mut contexts = HandlerContexts::default();

    for arg in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            if ty_str.contains("AppContext") {
                contexts.app_context = true;
                contexts.param_order.push(ContextParam::App);
            } else if ty_str.contains("GlobalContext") {
                contexts.global_context = true;
                contexts.param_order.push(ContextParam::Global);
            } else if ty_str.contains("ModalContext") {
                contexts.modal_context = true;
                contexts.param_order.push(ContextParam::Modal);
            }
        }
    }

    contexts
}

/// Extract full handler information from a signature.
///
/// Returns HandlerInfo containing:
/// - Context requirements (which contexts the handler needs)
/// - Non-context arguments (for wrapper signature generation)
pub fn extract_handler_info(name: &Ident, sig: &Signature) -> HandlerInfo {
    let mut contexts = HandlerContexts::default();
    let mut args = Vec::new();

    for arg in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            // Check if this is a context parameter
            if ty_str.contains("AppContext") {
                contexts.app_context = true;
                contexts.param_order.push(ContextParam::App);
            } else if ty_str.contains("GlobalContext") {
                contexts.global_context = true;
                contexts.param_order.push(ContextParam::Global);
            } else if ty_str.contains("ModalContext") {
                contexts.modal_context = true;
                contexts.param_order.push(ContextParam::Modal);
            } else if ty_str.contains("EventData") {
                contexts.event_data = true;
                contexts.param_order.push(ContextParam::Event);
            } else {
                // Non-context argument - capture for wrapper generation
                let pat = &pat_type.pat;
                args.push(HandlerArg {
                    pattern: quote!(#pat),
                    ty: quote!(#ty),
                });
            }
        }
    }

    HandlerInfo {
        name: name.clone(),
        contexts,
        args,
    }
}

// =============================================================================
// Keybinds DSL Parser
// =============================================================================

/// A parsed keybind entry from the DSL.
///
/// Supports two formats (both valid Rust syntax):
/// - `bind("key1", "key2", handler)` - multiple keys
/// - `bind("key", handler)` - single key
/// - `bind("key", handler(arg1, arg2))` - with arguments
#[derive(Clone, Debug)]
pub struct KeybindDslEntry {
    /// Key strings
    pub keys: Vec<syn::LitStr>,
    /// Handler name
    pub handler: Ident,
    /// Handler arguments (expressions captured as TokenStream for later processing)
    pub args: Vec<TokenStream>,
}

impl Parse for KeybindDslEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Expect: bind("key1", "key2", handler) or bind("key", handler(args))
        let bind_ident: Ident = input.parse()?;
        if bind_ident != "bind" {
            return Err(syn::Error::new(
                bind_ident.span(),
                "expected `bind`",
            ));
        }

        let content;
        syn::parenthesized!(content in input);

        // Parse keys (string literals) until we hit an identifier (handler name)
        let mut keys = Vec::new();
        loop {
            if content.peek(syn::LitStr) {
                let key: syn::LitStr = content.parse()?;
                keys.push(key);
                // Consume comma if present
                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
            } else {
                break;
            }
        }

        if keys.is_empty() {
            return Err(syn::Error::new(
                content.span(),
                "expected at least one key string",
            ));
        }

        // Parse handler name
        let handler: Ident = content.parse()?;

        // Parse optional arguments: handler(arg1, arg2)
        let args = if content.peek(syn::token::Paren) {
            let args_content;
            syn::parenthesized!(args_content in content);
            let args_punctuated =
                args_content.parse_terminated(parse_arg_expr, Token![,])?;
            args_punctuated.into_iter().collect()
        } else {
            Vec::new()
        };

        // Consume trailing comma if present (for the last arg in bind())
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }

        Ok(Self {
            keys,
            handler,
            args,
        })
    }
}

/// Parse a single argument expression as TokenStream.
/// We don't parse as syn::Expr because we want to preserve the tokens exactly
/// (they may reference self.foo which needs to be captured in closures).
fn parse_arg_expr(input: ParseStream) -> syn::Result<TokenStream> {
    let mut tokens = TokenStream::new();
    let mut depth = 0;

    while !input.is_empty() {
        // Stop at comma (unless nested in parens/brackets/braces)
        if depth == 0 && input.peek(Token![,]) {
            break;
        }

        if input.peek(syn::token::Paren) {
            let content;
            let paren = syn::parenthesized!(content in input);
            let inner: TokenStream = content.parse()?;
            tokens.extend(quote::quote_spanned!(paren.span.join()=> (#inner)));
            continue;
        }
        if input.peek(syn::token::Bracket) {
            let content;
            let bracket = syn::bracketed!(content in input);
            let inner: TokenStream = content.parse()?;
            tokens.extend(quote::quote_spanned!(bracket.span.join()=> [#inner]));
            continue;
        }
        if input.peek(syn::token::Brace) {
            let content;
            let brace = syn::braced!(content in input);
            let inner: TokenStream = content.parse()?;
            tokens.extend(quote::quote_spanned!(brace.span.join()=> {#inner}));
            continue;
        }

        // Parse other tokens
        let tt: proc_macro2::TokenTree = input.parse()?;
        match &tt {
            proc_macro2::TokenTree::Group(g) => {
                match g.delimiter() {
                    proc_macro2::Delimiter::Parenthesis
                    | proc_macro2::Delimiter::Bracket
                    | proc_macro2::Delimiter::Brace => {
                        depth += 1;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        tokens.extend(std::iter::once(tt));
    }

    Ok(tokens)
}

/// Parsed keybinds DSL - all entries in a keybinds method body
#[derive(Clone, Debug)]
pub struct KeybindsDsl {
    pub entries: Vec<KeybindDslEntry>,
}

impl Parse for KeybindsDsl {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();

        while !input.is_empty() {
            entries.push(input.parse()?);
            // Consume trailing semicolon if present
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
        }

        Ok(Self { entries })
    }
}

/// A context parameter type in a handler signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextParam {
    App,
    Global,
    Modal,
    Event,
}

/// Handler parameter requirements for the new context architecture.
///
/// Handlers declare what contexts they need via their signature (varargs pattern).
/// This tracks both which contexts are needed AND their order in the signature.
///
/// Note: All handlers are assumed to be async. Sync handlers are not supported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HandlerContexts {
    pub app_context: bool,
    pub global_context: bool,
    pub modal_context: bool,
    pub event_data: bool,
    /// Order of context parameters in the handler signature
    pub param_order: Vec<ContextParam>,
}

impl HandlerContexts {
    pub fn needs_app_context(&self) -> bool {
        self.app_context
    }

    pub fn needs_global_context(&self) -> bool {
        self.global_context
    }
}

/// Keybind scope parsed from #[keybinds(page = X)] attribute
#[derive(Clone, Debug, Default)]
pub enum KeybindScope {
    /// No scope specified - global keybinds
    #[default]
    Global,
    /// Page-scoped keybinds
    Page(String),
}

/// Information about a keybinds method
pub struct KeybindsMethod {
    /// Method name
    pub name: Ident,
    /// Scope for these keybinds
    pub scope: KeybindScope,
}

/// Information about a handler method (legacy - unused)
pub struct HandlerMethod {
    /// Method name
    pub name: Ident,
    /// Handler context requirements
    pub contexts: HandlerContexts,
    /// Handler is async
    pub is_async: bool,
}

/// A non-context argument in a handler signature.
#[derive(Clone, Debug)]
pub struct HandlerArg {
    /// The argument pattern (e.g., `id` or `_`)
    pub pattern: TokenStream,
    /// The argument type (e.g., `u64`)
    pub ty: TokenStream,
}

/// Full information about a handler method needed for wrapper generation.
#[derive(Clone, Debug)]
pub struct HandlerInfo {
    /// Handler name
    pub name: Ident,
    /// Context requirements
    pub contexts: HandlerContexts,
    /// Non-context arguments (excluding self and context params)
    pub args: Vec<HandlerArg>,
}

/// Information about an event handler method
pub struct EventHandlerMethod {
    /// Method name
    pub name: Ident,
    /// Event type as a string (for codegen)
    pub event_type: String,
    /// Handler context requirements
    pub contexts: HandlerContexts,
}

/// Information about a request handler method
pub struct RequestHandlerMethod {
    /// Method name
    pub name: Ident,
    /// Request type as a string (for codegen)
    pub request_type: String,
    /// Handler context requirements
    pub contexts: HandlerContexts,
}

/// Information about a page method marked with #[page] or #[page(Name)]
///
/// Note: Fields are collected but not currently used. Reserved for future
/// multi-page routing support.
#[allow(dead_code)]
pub struct PageMethod {
    /// Method name
    pub name: Ident,
    /// Optional page name (None for #[page], Some("Name") for #[page(Name)])
    pub page_name: Option<String>,
    /// The method body tokens (to be parsed as DSL)
    pub body: TokenStream,
}

/// Check if a method has the #[keybinds] attribute
pub fn is_keybinds_method(method: &ImplItemFn) -> bool {
    method.attrs.iter().any(|a| a.path().is_ident("keybinds"))
}

/// Check if method has #[handler] attribute and extract metadata
pub fn parse_handler_metadata(method: &ImplItemFn) -> Option<HandlerMethod> {
    for attr in &method.attrs {
        if attr.path().is_ident("handler") {
            let contexts = detect_handler_contexts(method);
            let is_async = method.sig.asyncness.is_some();

            return Some(HandlerMethod {
                name: method.sig.ident.clone(),
                contexts,
                is_async,
            });
        }
    }
    None
}

/// Check if method has #[event_handler] attribute and extract metadata.
/// Extracts the event type from the method signature (first non-self, non-context parameter).
pub fn parse_event_handler_metadata(method: &ImplItemFn) -> Option<EventHandlerMethod> {
    let has_attr = method
        .attrs
        .iter()
        .any(|a| a.path().is_ident("event_handler"));

    if !has_attr {
        return None;
    }

    let event_type = extract_message_type(method)?;
    let contexts = detect_handler_contexts(method);

    Some(EventHandlerMethod {
        name: method.sig.ident.clone(),
        event_type,
        contexts,
    })
}

/// Check if method has #[request_handler] attribute and extract metadata.
/// Extracts the request type from the method signature (first non-self, non-context parameter).
pub fn parse_request_handler_metadata(method: &ImplItemFn) -> Option<RequestHandlerMethod> {
    let has_attr = method
        .attrs
        .iter()
        .any(|a| a.path().is_ident("request_handler"));

    if !has_attr {
        return None;
    }

    let request_type = extract_message_type(method)?;
    let contexts = detect_handler_contexts(method);

    Some(RequestHandlerMethod {
        name: method.sig.ident.clone(),
        request_type,
        contexts,
    })
}

/// Check if method has #[page] or #[page(Name)] attribute and extract metadata.
/// Returns the page method info including the body tokens for DSL parsing.
pub fn parse_page_metadata(method: &ImplItemFn) -> Option<PageMethod> {
    for attr in &method.attrs {
        if attr.path().is_ident("page") {
            // Extract optional page name from #[page(Name)]
            let page_name = match &attr.meta {
                syn::Meta::Path(_) => None, // #[page]
                syn::Meta::List(list) => {
                    // #[page(Name)] - parse the name
                    let tokens = &list.tokens;
                    let name: Option<Ident> = syn::parse2(tokens.clone()).ok();
                    name.map(|n| n.to_string())
                }
                syn::Meta::NameValue(_) => None, // Not supported
            };

            // Extract the method body tokens for DSL parsing
            // The block contains the DSL, not regular Rust code
            let block = &method.block;
            let body: TokenStream = quote!(#block);

            return Some(PageMethod {
                name: method.sig.ident.clone(),
                page_name,
                body,
            });
        }
    }
    None
}

/// Extract the message type (event/request) from a method's parameters.
/// Returns the first non-self, non-context parameter type as a string.
fn extract_message_type(method: &ImplItemFn) -> Option<String> {
    for arg in &method.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");

            // Skip context parameters
            if ty_str.contains("AppContext")
                || ty_str.contains("GlobalContext")
                || ty_str.contains("ModalContext")
            {
                continue;
            }

            // Skip self
            if let syn::Pat::Ident(pat) = pat_type.pat.as_ref() {
                if pat.ident == "self" {
                    continue;
                }
            }

            return Some(ty_str);
        }
    }
    None
}

/// Detect which contexts a handler method needs from its signature.
/// Also tracks the ORDER of context parameters.
pub fn detect_handler_contexts(method: &ImplItemFn) -> HandlerContexts {
    let mut contexts = HandlerContexts::default();

    for arg in &method.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            let ty = &pat_type.ty;
            let ty_str = quote!(#ty).to_string();

            if ty_str.contains("AppContext") {
                contexts.app_context = true;
                contexts.param_order.push(ContextParam::App);
            } else if ty_str.contains("GlobalContext") {
                contexts.global_context = true;
                contexts.param_order.push(ContextParam::Global);
            } else if ty_str.contains("ModalContext") {
                contexts.modal_context = true;
                contexts.param_order.push(ContextParam::Modal);
            }
        }
    }

    contexts
}

/// Check if method is named "element"
pub fn is_element_method(method: &ImplItemFn) -> bool {
    method.sig.ident == "element"
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

/// Generate the metadata module name for a system
pub fn system_metadata_mod(type_name: &Ident) -> Ident {
    format_ident!(
        "__rafter_system_metadata_{}",
        type_name.to_string().to_lowercase()
    )
}

/// Strip custom rafter attributes from methods in an impl block
pub fn strip_custom_attrs(impl_block: &mut ItemImpl) {
    for item in &mut impl_block.items {
        if let ImplItem::Fn(method) = item {
            method.attrs.retain(|a| {
                !a.path().is_ident("keybinds")
                    && !a.path().is_ident("handler")
                    && !a.path().is_ident("event_handler")
                    && !a.path().is_ident("request_handler")
                    && !a.path().is_ident("page")
            });
            // Remove metadata doc attributes
            method.attrs.retain(|a| {
                if a.path().is_ident("doc")
                    && let syn::Meta::NameValue(nv) = &a.meta
                    && let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &nv.value
                {
                    let val = s.value();
                    return !val.starts_with("__rafter_handler:")
                        && !val.starts_with("__rafter_event_handler:")
                        && !val.starts_with("__rafter_request_handler:");
                }
                true
            });
        }
    }
}

/// Convert a PascalCase or camelCase identifier to snake_case
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

/// Generate keybinds trait method implementation
pub fn generate_keybinds_impl(
    keybinds_methods: &[KeybindsMethod],
    type_name: &Ident,
) -> TokenStream {
    let name_snake = to_snake_case(&type_name.to_string());

    if keybinds_methods.is_empty() {
        quote! {
            fn keybinds(&self) -> rafter::Keybinds {
                rafter::Keybinds::new()
            }
        }
    } else {
        let merge_calls: Vec<_> = keybinds_methods
            .iter()
            .map(|m| {
                let name = &m.name;
                match &m.scope {
                    KeybindScope::Global => {
                        let id_prefix = &name_snake;
                        quote! {
                            __keybinds.merge(
                                Self::#name().with_id_prefix(#id_prefix)
                            );
                        }
                    }
                    KeybindScope::Page(view_name) => {
                        let view_name_snake = to_snake_case(view_name);
                        let id_prefix = format!("{}.{}", name_snake, view_name_snake);
                        quote! {
                            __keybinds.merge(
                                Self::#name()
                                    .with_id_prefix(#id_prefix)
                                    .with_scope(rafter::KeybindScope::Page(#view_name.to_string()))
                            );
                        }
                    }
                }
            })
            .collect();

        quote! {
            fn keybinds(&self) -> rafter::Keybinds {
                let mut __keybinds = rafter::Keybinds::new();
                #(#merge_calls)*
                __keybinds
            }
        }
    }
}

/// Generate element trait method implementation
pub fn generate_element_impl(has_element: bool, self_ty: &Type) -> TokenStream {
    if has_element {
        quote! {
            fn element(&self) -> tuidom::Element {
                #self_ty::element(self)
            }
        }
    } else {
        quote! {
            fn element(&self) -> tuidom::Element {
                tuidom::Element::empty()
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

/// Generate config trait method implementation using metadata module
pub fn generate_config_impl(type_name: &Ident) -> TokenStream {
    let metadata_mod = app_metadata_mod(type_name);
    quote! {
        fn config() -> rafter::AppConfig {
            #metadata_mod::config()
        }
    }
}

// =============================================================================
// Closure Generation for Keybinds
// =============================================================================

/// Generate a closure registration for a single keybind DSL entry.
///
/// Produces code like:
/// ```ignore
/// {
///     let __self = self.clone();
///     let __arg0 = (arg0_expr).clone();
///     __keybinds.add("key", "type.handler", std::sync::Arc::new(move |__hx: &rafter::HandlerContext| {
///         __self.handler(__arg0.clone(), __hx.cx(), __hx.gx());
///     }));
/// }
/// ```
///
/// # Arguments
/// * `entry` - The DSL entry containing keys, handler name, and args
/// * `handler_contexts` - Map of handler names to their context requirements
/// * `type_name` - Type name for ID prefix
/// * `scope` - Optional page scope for the keybind
pub fn generate_closure_for_keybind(
    entry: &KeybindDslEntry,
    handler_contexts: &HashMap<String, HandlerContexts>,
    type_name: &Ident,
) -> TokenStream {
    let handler_name = &entry.handler;
    let handler_name_str = handler_name.to_string();
    let type_name_snake = to_snake_case(&type_name.to_string());
    let keybind_id = format!("{}.{}", type_name_snake, handler_name_str);

    // Look up handler's context requirements
    let contexts = handler_contexts
        .get(&handler_name_str)
        .cloned()
        .unwrap_or_default();

    // Generate argument captures (evaluated when keybinds() is called)
    let arg_captures: Vec<TokenStream> = entry
        .args
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let arg_name = format_ident!("__arg{}", i);
            quote! {
                let #arg_name = (#arg).clone();
            }
        })
        .collect();

    // Argument names for closure capture
    let arg_names: Vec<Ident> = (0..entry.args.len())
        .map(|i| format_ident!("__arg{}", i))
        .collect();

    let key_adds: Vec<TokenStream> = entry
        .keys
        .iter()
        .map(|key| {
            let cloned_args: Vec<TokenStream> = arg_names
                .iter()
                .map(|name| quote! { let #name = #name.clone(); })
                .collect();

            // Clone contexts for the async spawn
            let context_clones: Vec<TokenStream> = contexts
                .param_order
                .iter()
                .enumerate()
                .map(|(i, param)| {
                    let ctx_name = format_ident!("__ctx{}", i);
                    match param {
                        ContextParam::App => quote! { let #ctx_name = __hx.cx().clone(); },
                        ContextParam::Global => quote! { let #ctx_name = __hx.gx().clone(); },
                        ContextParam::Modal => quote! { let #ctx_name = __hx.mx().clone(); },
                        ContextParam::Event => quote! { let #ctx_name = __hx.event().clone(); },
                    }
                })
                .collect();

            let context_refs: Vec<TokenStream> = (0..contexts.param_order.len())
                .map(|i| {
                    let ctx_name = format_ident!("__ctx{}", i);
                    quote! { &#ctx_name }
                })
                .collect();

            let call_params: Vec<TokenStream> = arg_names
                .iter()
                .map(|name| quote! { #name.clone() })
                .chain(context_refs)
                .collect();

            // All handlers are async - spawn them
            let handler_call = quote! {
                #(#context_clones)*
                let __self = __self.clone();
                #(#cloned_args)*
                tokio::spawn(async move {
                    __self.#handler_name(#(#call_params),*).await;
                });
            };

            quote! {
                __keybinds.add(#key, #keybind_id, std::sync::Arc::new({
                    let __self = __self.clone();
                    #(#cloned_args)*
                    move |__hx: &rafter::HandlerContext| {
                        #handler_call
                    }
                }));
            }
        })
        .collect();

    quote! {
        {
            let __self = self.clone();
            #(#arg_captures)*
            #(#key_adds)*
        }
    }
}

/// Generate the full keybinds() method body using closures.
///
/// Parses each keybinds method body as DSL and generates closure registrations.
pub fn generate_keybinds_closures_impl(
    keybinds_methods: &[(KeybindsMethod, TokenStream)], // (method info, body tokens)
    handler_contexts: &HashMap<String, HandlerContexts>,
    type_name: &Ident,
) -> TokenStream {
    if keybinds_methods.is_empty() {
        return quote! {
            fn keybinds(&self) -> rafter::KeybindClosures {
                rafter::KeybindClosures::new()
            }
        };
    }

    let mut all_closure_code = Vec::new();

    for (method, body) in keybinds_methods {
        // Parse the body as DSL
        let dsl: KeybindsDsl = match syn::parse2(body.clone()) {
            Ok(d) => d,
            Err(e) => {
                return e.to_compile_error();
            }
        };

        // Generate closure code for each entry
        for entry in &dsl.entries {
            let closure_code =
                generate_closure_for_keybind(entry, handler_contexts, type_name);

            // Apply scope if page-scoped
            let scoped_code = match &method.scope {
                KeybindScope::Global => closure_code,
                KeybindScope::Page(_page_name) => {
                    // For page-scoped keybinds, we need to set scope on the keybind
                    // This is handled by the KeybindClosures::add_scoped method if we have it
                    // For now, we'll need to track this differently
                    // TODO: Handle page scope in KeybindClosures
                    closure_code
                }
            };

            all_closure_code.push(scoped_code);
        }
    }

    quote! {
        fn keybinds(&self) -> rafter::KeybindClosures {
            let mut __keybinds = rafter::KeybindClosures::new();
            #(#all_closure_code)*
            __keybinds
        }
    }
}

// =============================================================================
// Event/Request Dispatch Generation
// =============================================================================

/// Context type for dispatch generation - determines which contexts are available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchContextType {
    /// App dispatch: has AppContext + GlobalContext
    App,
    /// System dispatch: only GlobalContext
    System,
}

/// Generate event dispatch methods.
///
/// For App: `dispatch_event(&self, event_type, event, cx, gx)`
/// For System: `dispatch_event(&self, event_type, event, gx)`
pub fn generate_event_dispatch(
    event_handlers: &[EventHandlerMethod],
    context_type: DispatchContextType,
) -> TokenStream {
    if event_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();
            let (call, clones) = generate_event_handler_call_and_clones(name, &h.contexts, context_type);

            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => {
                    if let Some(event) = event.downcast_ref::<#event_type>() {
                        let event = event.clone();
                        #clones
                        tokio::spawn(async move {
                            #call
                        });
                        return true;
                    }
                    false
                }
            }
        })
        .collect();

    let has_handler_arms: Vec<_> = event_handlers
        .iter()
        .map(|h| {
            let event_type: syn::Type = syn::parse_str(&h.event_type).unwrap();
            quote! {
                t if t == std::any::TypeId::of::<#event_type>() => true,
            }
        })
        .collect();

    let sig = match context_type {
        DispatchContextType::App => quote! {
            fn dispatch_event(
                &self,
                event_type: std::any::TypeId,
                event: &(dyn std::any::Any + Send + Sync),
                cx: &rafter::AppContext,
                gx: &rafter::GlobalContext,
            ) -> bool
        },
        DispatchContextType::System => quote! {
            fn dispatch_event(
                &self,
                event_type: std::any::TypeId,
                event: &(dyn std::any::Any + Send + Sync),
                gx: &rafter::GlobalContext,
            ) -> bool
        },
    };

    quote! {
        #sig {
            match event_type {
                #(#dispatch_arms)*
                _ => false,
            }
        }

        fn has_event_handler(&self, event_type: std::any::TypeId) -> bool {
            match event_type {
                #(#has_handler_arms)*
                _ => false,
            }
        }
    }
}

/// Generate request dispatch methods.
///
/// For App: `dispatch_request(&self, request_type, request, cx, gx)`
/// For System: `dispatch_request(&self, request_type, request, gx)`
pub fn generate_request_dispatch(
    request_handlers: &[RequestHandlerMethod],
    context_type: DispatchContextType,
) -> TokenStream {
    if request_handlers.is_empty() {
        return quote! {};
    }

    let dispatch_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let name = &h.name;
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();
            let (call, clones) = generate_request_handler_call_and_clones(name, &h.contexts, context_type);

            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => {
                    if let Ok(request) = request.downcast::<#request_type>() {
                        #clones
                        return Some(Box::pin(async move {
                            let response = #call;
                            Box::new(response) as Box<dyn std::any::Any + Send + Sync>
                        }));
                    }
                    None
                }
            }
        })
        .collect();

    let has_handler_arms: Vec<_> = request_handlers
        .iter()
        .map(|h| {
            let request_type: syn::Type = syn::parse_str(&h.request_type).unwrap();
            quote! {
                t if t == std::any::TypeId::of::<#request_type>() => true,
            }
        })
        .collect();

    let sig = match context_type {
        DispatchContextType::App => quote! {
            fn dispatch_request(
                &self,
                request_type: std::any::TypeId,
                request: Box<dyn std::any::Any + Send + Sync>,
                cx: &rafter::AppContext,
                gx: &rafter::GlobalContext,
            ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>>>
        },
        DispatchContextType::System => quote! {
            fn dispatch_request(
                &self,
                request_type: std::any::TypeId,
                request: Box<dyn std::any::Any + Send + Sync>,
                gx: &rafter::GlobalContext,
            ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn std::any::Any + Send + Sync>> + Send>>>
        },
    };

    quote! {
        #sig {
            match request_type {
                #(#dispatch_arms)*
                _ => None,
            }
        }

        fn has_request_handler(&self, request_type: std::any::TypeId) -> bool {
            match request_type {
                #(#has_handler_arms)*
                _ => false,
            }
        }
    }
}

/// Generate event handler call expression and clone statements.
fn generate_event_handler_call_and_clones(
    name: &Ident,
    contexts: &HandlerContexts,
    context_type: DispatchContextType,
) -> (TokenStream, TokenStream) {
    match context_type {
        DispatchContextType::App => {
            let call = match (contexts.app_context, contexts.global_context) {
                (false, false) => quote! { this.#name(event).await; },
                (true, false) => quote! { this.#name(event, &cx).await; },
                (false, true) => quote! { this.#name(event, &gx).await; },
                (true, true) => quote! { this.#name(event, &cx, &gx).await; },
            };
            let clones = match (contexts.app_context, contexts.global_context) {
                (false, false) => quote! { let this = self.clone(); },
                (true, false) => quote! { let this = self.clone(); let cx = cx.clone(); },
                (false, true) => quote! { let this = self.clone(); let gx = gx.clone(); },
                (true, true) => quote! { let this = self.clone(); let cx = cx.clone(); let gx = gx.clone(); },
            };
            (call, clones)
        }
        DispatchContextType::System => {
            // Systems only have GlobalContext
            let call = if contexts.global_context {
                quote! { this.#name(event, &gx).await; }
            } else {
                quote! { this.#name(event).await; }
            };
            let clones = if contexts.global_context {
                quote! { let this = self.clone(); let gx = gx.clone(); }
            } else {
                quote! { let this = self.clone(); }
            };
            (call, clones)
        }
    }
}

/// Generate request handler call expression and clone statements.
fn generate_request_handler_call_and_clones(
    name: &Ident,
    contexts: &HandlerContexts,
    context_type: DispatchContextType,
) -> (TokenStream, TokenStream) {
    match context_type {
        DispatchContextType::App => {
            let call = match (contexts.app_context, contexts.global_context) {
                (false, false) => quote! { this.#name(*request).await },
                (true, false) => quote! { this.#name(*request, &cx).await },
                (false, true) => quote! { this.#name(*request, &gx).await },
                (true, true) => quote! { this.#name(*request, &cx, &gx).await },
            };
            let clones = match (contexts.app_context, contexts.global_context) {
                (false, false) => quote! { let this = self.clone(); },
                (true, false) => quote! { let this = self.clone(); let cx = cx.clone(); },
                (false, true) => quote! { let this = self.clone(); let gx = gx.clone(); },
                (true, true) => quote! { let this = self.clone(); let cx = cx.clone(); let gx = gx.clone(); },
            };
            (call, clones)
        }
        DispatchContextType::System => {
            // Systems only have GlobalContext
            let call = if contexts.global_context {
                quote! { this.#name(*request, &gx).await }
            } else {
                quote! { this.#name(*request).await }
            };
            let clones = if contexts.global_context {
                quote! { let this = self.clone(); let gx = gx.clone(); }
            } else {
                quote! { let this = self.clone(); }
            };
            (call, clones)
        }
    }
}

// =============================================================================
// Handler Wrapper Generation
// =============================================================================

/// Generate dispatch wrapper methods for all handlers.
///
/// For each handler, generates a `__wrap_{name}` method that:
/// - Takes the handler's non-context arguments + `&HandlerContext`
/// - Clones everything needed for the async spawn
/// - Spawns the async handler with proper context injection
///
/// Example:
/// ```ignore
/// // User writes:
/// #[handler]
/// async fn delete(&self, id: u64, cx: &AppContext) { ... }
///
/// // Macro generates:
/// fn __wrap_delete(&self, id: u64, __hx: &rafter::HandlerContext) {
///     let __self = self.clone();
///     let __arg0 = id.clone();
///     let __ctx0 = __hx.cx().clone();
///     tokio::spawn(async move {
///         __self.delete(__arg0, &__ctx0).await;
///     });
/// }
/// ```
pub fn generate_handler_wrappers(handlers: &[HandlerInfo]) -> TokenStream {
    let wrappers: Vec<TokenStream> = handlers
        .iter()
        .map(|handler| generate_single_wrapper(handler))
        .collect();

    quote! {
        #(#wrappers)*
    }
}

/// Generate a single dispatch wrapper method for a handler.
fn generate_single_wrapper(handler: &HandlerInfo) -> TokenStream {
    let handler_name = &handler.name;
    let wrapper_name = format_ident!("__wrap_{}", handler_name);

    // Generate wrapper parameters: non-context args + HandlerContext
    let wrapper_params: Vec<TokenStream> = handler
        .args
        .iter()
        .map(|arg| {
            let pat = &arg.pattern;
            let ty = &arg.ty;
            quote! { #pat: #ty }
        })
        .collect();

    // Generate argument clones
    let arg_clones: Vec<TokenStream> = handler
        .args
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let arg_name = format_ident!("__arg{}", i);
            let pat = &arg.pattern;
            quote! { let #arg_name = #pat.clone(); }
        })
        .collect();

    // Generate context clones based on handler's context requirements
    let context_clones: Vec<TokenStream> = handler
        .contexts
        .param_order
        .iter()
        .enumerate()
        .map(|(i, param)| {
            let ctx_name = format_ident!("__ctx{}", i);
            match param {
                ContextParam::App => quote! { let #ctx_name = __hx.cx().clone(); },
                ContextParam::Global => quote! { let #ctx_name = __hx.gx().clone(); },
                ContextParam::Modal => quote! { let #ctx_name = __hx.mx().clone(); },
                ContextParam::Event => quote! { let #ctx_name = __hx.event().clone(); },
            }
        })
        .collect();

    // Generate context references for the handler call
    let context_refs: Vec<TokenStream> = (0..handler.contexts.param_order.len())
        .map(|i| {
            let ctx_name = format_ident!("__ctx{}", i);
            quote! { &#ctx_name }
        })
        .collect();

    // Generate argument usages for the handler call
    let arg_usages: Vec<TokenStream> = (0..handler.args.len())
        .map(|i| {
            let arg_name = format_ident!("__arg{}", i);
            quote! { #arg_name }
        })
        .collect();

    // Combine args and contexts for the handler call
    let call_params: Vec<TokenStream> = arg_usages
        .into_iter()
        .chain(context_refs)
        .collect();

    quote! {
        #[doc(hidden)]
        fn #wrapper_name(&self, #(#wrapper_params,)* __hx: &rafter::HandlerContext) {
            let __self = self.clone();
            #(#arg_clones)*
            #(#context_clones)*
            tokio::spawn(async move {
                __self.#handler_name(#(#call_params),*).await;
            });
        }
    }
}
