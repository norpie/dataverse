//! The `keybinds!` macro for defining keybind mappings.
//!
//! Parses a DSL and generates `Keybinds::add_str` calls.
//! Parsing happens at runtime, errors are logged and skipped.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Ident, LitStr, Token,
    parse::{Parse, ParseStream},
    parse2,
};

/// A single keybind entry: "key" | "key2" => handler
struct KeybindEntry {
    /// Key strings (alternatives separated by |)
    keys: Vec<LitStr>,
    /// Handler name
    handler: Ident,
}

impl Parse for KeybindEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut keys = Vec::new();

        // Parse first key
        let first: LitStr = input.parse()?;
        keys.push(first);

        // Parse alternatives: | "key2" | "key3"
        while input.peek(Token![|]) {
            input.parse::<Token![|]>()?;
            let alt: LitStr = input.parse()?;
            keys.push(alt);
        }

        // Parse =>
        input.parse::<Token![=>]>()?;

        // Parse handler name
        let handler: Ident = input.parse()?;

        Ok(Self { keys, handler })
    }
}

/// All keybind entries
struct KeybindsInput {
    entries: Vec<KeybindEntry>,
}

impl Parse for KeybindsInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();

        while !input.is_empty() {
            entries.push(input.parse()?);

            // Optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { entries })
    }
}

pub fn expand(input: TokenStream) -> TokenStream {
    let keybinds: KeybindsInput = match parse2(input) {
        Ok(k) => k,
        Err(e) => return e.to_compile_error(),
    };

    let mut add_calls = Vec::new();

    for entry in keybinds.entries {
        let handler_name = entry.handler.to_string();

        for key_lit in &entry.keys {
            add_calls.push(quote! {
                __keybinds.add_str(#key_lit, #handler_name);
            });
        }
    }

    quote! {
        {
            let mut __keybinds = rafter::keybinds::Keybinds::new();
            #(#add_calls)*
            __keybinds
        }
    }
}
