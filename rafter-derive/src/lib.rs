mod macros;

use proc_macro::TokenStream;

#[proc_macro_derive(Event)]
pub fn derive_event(input: TokenStream) -> TokenStream {
    macros::event::expand(input.into()).into()
}

#[proc_macro_derive(Request, attributes(response))]
pub fn derive_request(input: TokenStream) -> TokenStream {
    macros::request::expand(input.into()).into()
}
