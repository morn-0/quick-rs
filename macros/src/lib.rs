use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn class(attr: TokenStream, item: TokenStream) -> TokenStream {
    println!("{attr:?}");
    println!("{item:?}");
    item
}
