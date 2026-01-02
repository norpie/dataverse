//! The `#[event_handler]` attribute macro for event subscriber methods.

use proc_macro2::TokenStream;

use super::handler_common::expand_message_handler;

pub fn expand(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_message_handler(item, false)
}
