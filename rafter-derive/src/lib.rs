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

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::handler::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn event_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::event_handler::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn request_handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::request_handler::expand(attr.into(), item.into()).into()
}
