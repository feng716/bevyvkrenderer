use proc_macro::TokenStream;
use quote::quote;
#[proc_macro_attribute]
pub fn shader_lang(_attr: TokenStream, item: TokenStream) -> TokenStream {
    TokenStream::from(quote!{

    })
}