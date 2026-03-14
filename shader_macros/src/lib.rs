use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DataStruct, DeriveInput, Fields, parse_macro_input};
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

#[proc_macro_derive(ShaderStruct)]
pub fn derive_shader_struct(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(DataStruct { fields: Fields::Named(fields), .. }) => &fields.named,
        _ => panic!("ShaderStruct can only be derived for structs with named fields!"),
    };

    let accessors: Vec<_> = fields.iter().map(|f| {
        let field_name = f.ident.as_ref().unwrap();
        let field_ty = &f.ty;
        let field_name_str = field_name.to_string();

        quote! {
            pub fn #field_name(self) -> TypedAccessExpr<Var<#struct_name>, #field_ty> {
                TypedAccessExpr {
                    v: VarAccessExpr::StructFieldAccess(
                        Box::new(VarAccessExpr::Var(self)), 
                        #field_name_str
                    ),
                    _marker: std::marker::PhantomData,
                }
            }
        }
    }).collect();

    // 2. WGSL Fields (Collected to Vec)
    let wgsl_fields: Vec<_> = fields.iter().map(|f| {
        let field_name_str = f.ident.as_ref().unwrap().to_string();
        let field_ty = &f.ty;
        quote! {
            format!("    {}: {},\n", #field_name_str, <#field_ty as WgslType>::wgsl_name())
        }
    }).collect();

    // 3. Generics & Arguments (Collected to Vec)
    let generics: Vec<_> = (0..fields.len())
        .map(|i| syn::Ident::new(&format!("E{}", i), proc_macro2::Span::call_site()))
        .collect();

    let new_args: Vec<_> = fields.iter().zip(&generics).map(|(f, g)| {
        let field_name = &f.ident;
        quote! { #field_name: #g }
    }).collect();

    // 4. Bound names & Core Types (THESE WERE MISSING!)
    let bound_names: Vec<_> = fields.iter().map(|f| {
        let ident = f.ident.as_ref().unwrap();
        syn::Ident::new(&format!("_res_{}", ident), proc_macro2::Span::call_site())
    }).collect();

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    // 5. Bounds, Evals, and Call Args (Collected to Vec)
    let bounds: Vec<_> = fields.iter().zip(&generics).map(|(f, g)| {
        let field_ty = &f.ty;
        quote! { #g: Into<ShaderDSL<'a, Var<#field_ty>>> + Clone + 'a}
    }).collect();

    let evals: Vec<_> = fields.iter().zip(&bound_names).map(|(f, b)| {
        let field_name = f.ident.as_ref().unwrap();
        let field_ty = &f.ty; 
        quote! { 
            #b <- Into::<ShaderDSL<'a, Var<#field_ty>>>::into(#field_name.clone()); 
        }
    }).collect();

    let call_args: Vec<_> = bound_names.iter().map(|b| {
        quote! { Into::<FuncArg>::into(#b) }
    }).collect();

    let struct_name_str = struct_name.to_string();

    // 6. The Constructor (Now type-safe and fully inferred!)
    let struct_constructor = quote! {
        impl #struct_name {
            pub fn new<'a, #(#generics),*>(
                #(#new_args),*
            ) -> ShaderDSL<'a, Var<#struct_name>>
            where
                #(#bounds,)* #(Var<#field_types>: Into<FuncArg>),* {
                
                _mdo_move! {
                    [ #(#field_names),* ] 
                    
                    #(#evals)*
                    
                    ident <- _new_ident();
                    
                    _call_func_rt(
                        FuncName::NormalFunctionInvoke(#struct_name_str), 
                        vec![ #(#call_args),* ], 
                        ident
                    )
                }
            }
        }
    }; 

    let expanded = quote! {
        impl WgslType for #struct_name {
            fn wgsl_name() -> &'static str {
                stringify!(#struct_name)
            }
        }

        impl Var<#struct_name> {
            #(#accessors)*
        }

        impl ShaderStruct for #struct_name {
            fn wgsl_struct_decl() -> String {
                let mut decl = format!("struct {} {{\n", stringify!(#struct_name));
                #(
                    decl.push_str(&#wgsl_fields);
                )*
                decl.push_str("}");
                decl
            }
        }

        #struct_constructor

        impl From<Var<#struct_name>> for FuncArg {
            fn from(v: Var<#struct_name>) -> Self {
                // IMPORTANT: Change `FuncArg::Expr` to match whatever enum variant 
                // you actually use to hold strings/AST nodes in your FuncArg definition!
                FuncArg::StructName(v.to_string()) 
            }
        }
    };

    TokenStream::from(expanded)
}