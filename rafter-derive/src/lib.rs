mod macros;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn app(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::app::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn app_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::app_impl::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::handler::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::component::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn modal(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::modal::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn modal_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::modal_impl::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn theme(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::theme::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn theme_group(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::theme_group::expand(attr.into(), item.into()).into()
}

#[proc_macro_attribute]
pub fn system_overlay(attr: TokenStream, item: TokenStream) -> TokenStream {
    macros::system_overlay::expand(attr.into(), item.into()).into()
}

#[proc_macro]
pub fn view(input: TokenStream) -> TokenStream {
    macros::view::expand(input.into()).into()
}

#[proc_macro]
pub fn keybinds(input: TokenStream) -> TokenStream {
    macros::keybinds::expand(input.into()).into()
}
