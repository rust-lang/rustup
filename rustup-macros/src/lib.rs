//! Procedural macros for `rustup`.

use ::quote::quote;
use proc_macro2::TokenStream;
use syn::{parse_macro_input, parse_quote, Block, Expr, ItemFn, LitStr};

/// Custom wrapper macro around `#[test]` and `#[tokio::test]`.
///
/// Calls `rustup::test::before_test()` before the test body, and
/// `rustup::test::after_test()` after, even in the event of an unwinding panic.
/// For async functions calls the async variants of these functions.
#[proc_macro_attribute]
pub fn integration_test(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut path: Option<LitStr> = None;
    if !args.is_empty() {
        let test_parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("mod_path") {
                path = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported test property"))
            }
        });

        parse_macro_input!(args with test_parser);
    }
    let input = parse_macro_input!(input);
    test_inner(
        path.map(|s| s.value()).unwrap_or("::rustup::test".into()),
        Clone::clone(&input),
    )
    .unwrap_or_else(|err| {
        let err = err.to_compile_error();
        quote! { #err #input }
    })
    .into()
}

/// Custom wrapper macro around `#[test]` and `#[tokio::test]` for unit tests.
///
/// Calls `rustup::test::before_test()` before the test body, and
/// `rustup::test::after_test()` after, even in the event of an unwinding panic.
/// For async functions calls the async variants of these functions.
#[proc_macro_attribute]
pub fn unit_test(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut path: Option<LitStr> = None;

    if !args.is_empty() {
        let test_parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("mod_path") {
                path = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported test property"))
            }
        });

        parse_macro_input!(args with test_parser);
    }

    let input = parse_macro_input!(input);

    test_inner(
        path.map(|s| s.value()).unwrap_or("crate::test".into()),
        Clone::clone(&input),
    )
    .unwrap_or_else(|err| {
        let err = err.to_compile_error();
        quote! { #err #input }
    })
    .into()
}

fn test_inner(mod_path: String, mut input: ItemFn) -> syn::Result<TokenStream> {
    if input.sig.asyncness.is_some() {
        let before_ident = format!("{}::before_test_async", mod_path);
        let before_ident = syn::parse_str::<Expr>(&before_ident)?;
        let after_ident = format!("{}::after_test_async", mod_path);
        let after_ident = syn::parse_str::<Expr>(&after_ident)?;

        let inner = input.block;
        let name = input.sig.ident.clone();
        let new_block: Block = parse_quote! {
            {
                #before_ident().await;
                // Define a function with same name we can instrument inside the
                // tracing enablement logic.
                #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
                async fn #name() { #inner }
                // Thunk through a new thread to permit catching the panic
                // without grabbing the entire state machine defined by the
                // outer test function.
                let result = ::std::panic::catch_unwind(||{
                    let handle = tokio::runtime::Handle::current().clone();
                    ::std::thread::spawn(move || handle.block_on(#name())).join().unwrap()
                });
                #after_ident().await;
                match result {
                    Ok(result) => result,
                    Err(err) => ::std::panic::resume_unwind(err)
                }
            }
        };

        input.block = Box::new(new_block);

        Ok(quote! {
            #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
            #[::tokio::test(flavor = "multi_thread", worker_threads = 1)]
            #input
        })
    } else {
        let before_ident = format!("{}::before_test", mod_path);
        let before_ident = syn::parse_str::<Expr>(&before_ident)?;
        let after_ident = format!("{}::after_test", mod_path);
        let after_ident = syn::parse_str::<Expr>(&after_ident)?;

        let inner = input.block;
        let name = input.sig.ident.clone();
        let new_block: Block = parse_quote! {
            {
                #before_ident();
                // Define a function with same name we can instrument inside the
                // tracing enablement logic.
                #[cfg_attr(feature = "otel", tracing::instrument(skip_all))]
                fn #name() { #inner }
                let result = ::std::panic::catch_unwind(#name);
                #after_ident();
                match result {
                    Ok(result) => result,
                    Err(err) => ::std::panic::resume_unwind(err)
                }
            }
        };

        input.block = Box::new(new_block);
        Ok(quote! {
            #[::std::prelude::v1::test]
            #input
        })
    }
}
