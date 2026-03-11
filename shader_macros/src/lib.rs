use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};
#[proc_macro_attribute]
pub fn shader_lang(_attr: TokenStream, item: TokenStream) -> TokenStream {
    TokenStream::from(quote!{

    })
}
#[proc_macro_derive(DisplayInner)]
pub fn derive_display_inner(input: TokenStream) -> TokenStream {
    // 1. Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);
    
    // Get the name of the enum (e.g., `FuncArg`)
    let name = &input.ident;

    // 2. Ensure this macro is only applied to enums
    let data_enum = match &input.data {
        Data::Enum(data) => data,
        _ => panic!("DisplayInner can only be derived for enums"),
    };

    // 3. Generate the match arms for each variant
    let match_arms = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;

        match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                quote! {
                    #name::#variant_name(inner_val) => inner_val.to_string(),
                }
            }
            _ => panic!("ToStringInner currently only supports variants with exactly one unnamed field!"),
        }
    }).collect::<Vec<_>>(); // Collected to avoid type inference errors!

    // 4. Implement std::string::ToString
    let expanded = quote! {
        impl std::string::ToString for #name {
            fn to_string(&self) -> String {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };
    

    // 5. Return the generated code as a TokenStream
    TokenStream::from(expanded)
}