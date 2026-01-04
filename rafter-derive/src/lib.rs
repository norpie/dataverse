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

#[proc_macro]
pub fn keybinds(input: TokenStream) -> TokenStream {
    macros::keybinds::expand(input.into()).into()
}

#[proc_macro]
pub fn page(input: TokenStream) -> TokenStream {
    macros::page::expand(input.into()).into()
}

#[proc_macro_attribute]
pub fn theme(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::theme::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn app(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::app::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn modal(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::modal::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::system::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn app_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::app_impl::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn modal_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::modal_impl::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn system_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::system_impl::expand(attr.into(), item.into()).into()
}
