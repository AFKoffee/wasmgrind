use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, Result, Token, parse_macro_input};

struct ThreadCreateFnInput {
    engine: Expr,
    module: Expr,
    memory: Expr,
    linker: Expr,
    tmgmt: Expr,
    tracing: Option<Expr>,
}

impl Parse for ThreadCreateFnInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut engine = None;
        let mut module = None;
        let mut memory = None;
        let mut linker = None;
        let mut tmgmt = None;
        let mut tracing = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match key.to_string().as_str() {
                "engine" => engine = Some(input.parse()?),
                "module" => module = Some(input.parse()?),
                "memory" => memory = Some(input.parse()?),
                "linker" => linker = Some(input.parse()?),
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

        Ok(ThreadCreateFnInput {
            engine: engine
                .ok_or_else(|| syn::Error::new(input.span(), "Missing argument: engine"))?,
            module: module
                .ok_or_else(|| syn::Error::new(input.span(), "Missing argument: module"))?,
            memory: memory
                .ok_or_else(|| syn::Error::new(input.span(), "Missing argument: memory"))?,
            linker: linker
                .ok_or_else(|| syn::Error::new(input.span(), "Missing argument: linker"))?,
            tmgmt: tmgmt.ok_or_else(|| syn::Error::new(input.span(), "Missing argument: tmgmt"))?,
            tracing,
        })
    }
}

pub fn thread_create_func_(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ThreadCreateFnInput);

    let ThreadCreateFnInput {
        engine,
        module,
        memory,
        linker,
        tmgmt,
        tracing: maybe_tracing,
    } = input;

    let (closure_args, tracing_code) = if maybe_tracing.is_some() {
        (
            quote! { thread_id_ptr: u32, start_routine: u32, fidx: u32, iidx: u32 },
            quote! {
                tracing.add_event(
                    wasmgrind_core::tmgmt::thread_id().expect("Thread-ID should be accessible"),
                    race_detection::tracing::Op::Fork { tid },
                    (fidx, iidx),
                ).expect("Error while adding event to tracing");
            },
        )
    } else {
        (quote! { thread_id_ptr: u32, start_routine: u32 }, quote! {})
    };

    let tracing_expr = maybe_tracing.unwrap_or_else(|| syn::parse_quote! { () }); // dummy if none, unused anyway
    let expanded = quote! {
        {
            let engine = #engine;
            let module = #module;
            let memory = #memory;
            let linker = #linker;
            let tmgmt = #tmgmt;
            let tracing = #tracing_expr;

            move |#closure_args| {
                let engine = engine.clone();
                let module = module.clone();
                let linker = linker.clone();

                let tid = match tmgmt.lock() {
                    Ok(mut tmgmt_guard) => {
                        let tid = tmgmt_guard.register_thread();
                        drop(tmgmt_guard);
                        tid
                    },
                    Err(_) => return wasmgrind_error::errno::RT_ERROR_TMGMT_LOCK_POISONED,
                };

                #tracing_code

                let handle = std::thread::spawn(move || {
                    wasmgrind_core::tmgmt::set_thread_id(tid)?;
                    let mut store = wasmtime::Store::new(&engine, ());
                    let instance = match linker.read() {
                        Ok(linker_guard) => linker_guard.instantiate(&mut store, &module)?,
                        Err(_) => anyhow::bail!("Linker Mutex was poisoned!"),
                    };
                    let thread_start = instance.get_typed_func::<u32, ()>(&mut store, "thread_start")?;
                    thread_start.call(&mut store, start_routine)?;
                    Ok(())
                });

                match tmgmt.lock() {
                    Ok(tmgmt_guard) => {
                        tmgmt_guard.set_join_handle(tid, handle)
                            .expect("JoinHandle should be setable");
                    },
                    Err(_) => return wasmgrind_error::errno::RT_ERROR_TMGMT_LOCK_POISONED,
                }

                match usize::try_from(thread_id_ptr) {
                    Ok(tid_ptr) => {
                        crate::runtime::base::ThreadlinkRuntime::write_data_to_memory(&memory, tid_ptr, &tid.to_le_bytes())
                    },
                    Err(_) => wasmgrind_error::errno::RT_ERROR_TID_POINTER_CONVERSION_FAILED,
                }
            }
        }
    };

    TokenStream::from(expanded)
}
