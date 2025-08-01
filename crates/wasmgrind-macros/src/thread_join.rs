use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, Result, Token, parse_macro_input};

struct ThreadJoinFnInput {
    tmgmt: Expr,
    tracing: Option<Expr>,
}

impl Parse for ThreadJoinFnInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut tmgmt = None;
        let mut tracing = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "tmgmt" => tmgmt = Some(input.parse()?),
                "tracing" => tracing = Some(input.parse()?),
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("Unexpected argument `{}`", other),
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(ThreadJoinFnInput {
            tmgmt: tmgmt.ok_or_else(|| syn::Error::new(input.span(), "Missing argument: tmgmt"))?,
            tracing,
        })
    }
}

pub fn thread_join_func_(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ThreadJoinFnInput);
    let ThreadJoinFnInput {
        tmgmt,
        tracing: maybe_tracing,
    } = input;

    let (closure_args, tracing_code) = if maybe_tracing.is_some() {
        (
            quote! { tid: u32, fidx: u32, iidx: u32 },
            quote! {
                tracing.add_event(
                    wasmgrind_core::tmgmt::thread_id()
                        .expect("Thread-ID should be accessible"),
                    race_detection::tracing::Op::Join { tid },
                    (fidx, iidx),
                )
                .expect("Error while adding event to tracing");
            },
        )
    } else {
        (quote! { tid: u32 }, quote! {})
    };

    let tracing_expr = maybe_tracing.unwrap_or_else(|| syn::parse_quote! { () }); // dummy if none, unused anyway
    let expanded = quote! {
        {
            let tmgmt = #tmgmt;
            let tracing = #tracing_expr;

            move |#closure_args| {
                let thread = match tmgmt.lock() {
                    Ok(mut tmgmt_guard) => {
                        let thread = tmgmt_guard.retrieve_thread(tid);
                        drop(tmgmt_guard);
                        thread
                    }
                    Err(_) => return wasmgrind_error::errno::RT_ERROR_TMGMT_LOCK_POISONED,
                };

                if let Some(cond_handle) = thread {
                    match cond_handle
                        .take_when_ready()
                        .expect("JoinHandle should be accessible!")
                        .join()
                    {
                        // TODO:
                        // At this point the thread-local-storage and thread-stack should be deallocated
                        // by calling the method `__wasmgrind_thread_destroy` injected by wasm-threadify
                        Ok(result) => match result {
                            Ok(()) => {
                                #tracing_code
                                //println!("Event: T{}|join(T{})|F{}-I{}", thread_id(), tid, fidx, iidx);

                                wasmgrind_error::errno::NO_ERROR
                            }
                            Err(e) => {
                                println!("Error in Thread: {e}");
                                wasmgrind_error::errno::RT_ERROR_THREAD_RUNTIME_FAILURE
                            }
                        },
                        Err(e) => {
                            println!("Error in Join: {e:?}");
                            wasmgrind_error::errno::RT_ERROR_THREAD_JOIN_FAILURE
                        }
                    }
                } else {
                    println!("Error in Management: Given thread id not found!");
                    wasmgrind_error::errno::RT_ERROR_THREAD_NOT_FOUND
                }
            }
        }
    };

    TokenStream::from(expanded)
}
