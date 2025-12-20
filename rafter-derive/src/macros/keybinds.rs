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
    keys: Vec<String>,
    /// Handler name
    handler: Ident,
}

impl Parse for KeybindEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut keys = Vec::new();

        // Parse first key
        let first: LitStr = input.parse()?;
        keys.push(first.value());

        // Parse alternatives: | "key2" | "key3"
        while input.peek(Token![|]) {
            input.parse::<Token![|]>()?;
            let alt: LitStr = input.parse()?;
            keys.push(alt.value());
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

/// Parse a key string like "ctrl+shift+a" or "gg" into KeyCombo(s)
fn parse_key_string(s: &str) -> TokenStream {
    // Check for modifier prefixes
    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;

    let parts: Vec<&str> = s.split('+').collect();
    let key_part = if parts.len() > 1 {
        // Has modifiers
        for part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                _ => {}
            }
        }
        parts[parts.len() - 1]
    } else {
        parts[0]
    };

    // Check if it's a special key name first
    let is_special_key = matches!(
        key_part.to_lowercase().as_str(),
        "enter"
            | "return"
            | "escape"
            | "esc"
            | "backspace"
            | "tab"
            | "space"
            | "up"
            | "down"
            | "left"
            | "right"
            | "home"
            | "end"
            | "pageup"
            | "pgup"
            | "pagedown"
            | "pgdn"
            | "insert"
            | "ins"
            | "delete"
            | "del"
            | "f1"
            | "f2"
            | "f3"
            | "f4"
            | "f5"
            | "f6"
            | "f7"
            | "f8"
            | "f9"
            | "f10"
            | "f11"
            | "f12"
    );

    // Check if it's a sequence (multiple chars without modifiers, like "gg")
    // but NOT a special key name
    let is_sequence = !ctrl
        && !shift
        && !alt
        && key_part.len() > 1
        && key_part.chars().all(|c| c.is_alphanumeric())
        && !is_special_key;

    if is_sequence {
        // Generate a sequence of KeyCombos
        let combos: Vec<_> = key_part
            .chars()
            .map(|c| {
                let key = parse_single_key(&c.to_string());
                quote! {
                    rafter::keybinds::KeyCombo::new(
                        #key,
                        rafter::events::Modifiers::NONE
                    )
                }
            })
            .collect();

        quote! { vec![#(#combos),*] }
    } else {
        // Single key with optional modifiers
        let key = parse_single_key(key_part);
        let modifiers = quote! {
            rafter::events::Modifiers {
                ctrl: #ctrl,
                shift: #shift,
                alt: #alt,
            }
        };

        quote! {
            vec![rafter::keybinds::KeyCombo::new(#key, #modifiers)]
        }
    }
}

/// Parse a single key name into a Key enum variant
fn parse_single_key(s: &str) -> TokenStream {
    match s.to_lowercase().as_str() {
        "enter" | "return" => quote! { rafter::keybinds::Key::Enter },
        "escape" | "esc" => quote! { rafter::keybinds::Key::Escape },
        "backspace" => quote! { rafter::keybinds::Key::Backspace },
        "tab" => quote! { rafter::keybinds::Key::Tab },
        "space" => quote! { rafter::keybinds::Key::Space },
        "up" => quote! { rafter::keybinds::Key::Up },
        "down" => quote! { rafter::keybinds::Key::Down },
        "left" => quote! { rafter::keybinds::Key::Left },
        "right" => quote! { rafter::keybinds::Key::Right },
        "home" => quote! { rafter::keybinds::Key::Home },
        "end" => quote! { rafter::keybinds::Key::End },
        "pageup" | "pgup" => quote! { rafter::keybinds::Key::PageUp },
        "pagedown" | "pgdn" => quote! { rafter::keybinds::Key::PageDown },
        "insert" | "ins" => quote! { rafter::keybinds::Key::Insert },
        "delete" | "del" => quote! { rafter::keybinds::Key::Delete },
        "f1" => quote! { rafter::keybinds::Key::F(1) },
        "f2" => quote! { rafter::keybinds::Key::F(2) },
        "f3" => quote! { rafter::keybinds::Key::F(3) },
        "f4" => quote! { rafter::keybinds::Key::F(4) },
        "f5" => quote! { rafter::keybinds::Key::F(5) },
        "f6" => quote! { rafter::keybinds::Key::F(6) },
        "f7" => quote! { rafter::keybinds::Key::F(7) },
        "f8" => quote! { rafter::keybinds::Key::F(8) },
        "f9" => quote! { rafter::keybinds::Key::F(9) },
        "f10" => quote! { rafter::keybinds::Key::F(10) },
        "f11" => quote! { rafter::keybinds::Key::F(11) },
        "f12" => quote! { rafter::keybinds::Key::F(12) },
        _ => {
            // Single character key
            if let Some(c) = s.chars().next() {
                quote! { rafter::keybinds::Key::Char(#c) }
            } else {
                quote! { rafter::keybinds::Key::Char(' ') }
            }
        }
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

        for key_str in &entry.keys {
            let keys = parse_key_string(key_str);

            add_calls.push(quote! {
                __keybinds.add(rafter::keybinds::Keybind {
                    keys: #keys,
                    handler: rafter::keybinds::HandlerId::new(#handler_name),
                    scope: rafter::keybinds::KeybindScope::Global,
                });
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
