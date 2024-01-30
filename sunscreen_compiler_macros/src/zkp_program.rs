use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use sunscreen_compiler_common::macros::{
    create_program_node, emit_signature, extract_fn_arguments, ExtractFnArgumentsError,
};
use syn::{
    parse_macro_input, spanned::Spanned, Generics, ItemFn, Path, ReturnType, Type, TypeParamBound,
};

use crate::{
    error::{Error, Result},
    internals::attr::ZkpProgramAttrs,
};

enum ArgumentKind {
    Shared,
    Private,
    Public,
    Constant,
}

pub fn zkp_program_impl(
    metadata: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr_params = parse_macro_input!(metadata as ZkpProgramAttrs);
    let input_fn = parse_macro_input!(input as ItemFn);

    match parse_inner(attr_params, input_fn) {
        Ok(s) => s.into(),
        Err(Error::CompileError(s, msg)) => proc_macro::TokenStream::from(quote_spanned! {
            s => compile_error! { #msg }
        }),
    }
}

fn get_generic_arg(generics: &Generics) -> Result<(Ident, Path)> {
    if generics.type_params().count() != 1 {
        return Err(Error::compile_error(
            generics.span(),
            "ZKP programs must take 1 generic argument with bound sunscreen::BackendField",
        ));
    }

    if generics.lifetimes().count() > 0 {
        return Err(Error::compile_error(
            generics.span(),
            "ZKP programs don't support lifetimes.",
        ));
    }

    if generics.const_params().count() > 0 {
        return Err(Error::compile_error(
            generics.span(),
            "ZKP programs don't support const params.",
        ));
    }

    let generic = generics.type_params().next().unwrap();
    if generic.bounds.len() != 1 {
        return Err(Error::compile_error(generic.span(), "ZKP programs must take 1 generic argument with bound sunscreen::BackendField. This must be the only bound and cannot be specified in a `where` clause."));
    }

    let bound = generic.bounds[0].clone();

    let bound = match bound {
        TypeParamBound::Trait(x) => x,
        TypeParamBound::Lifetime(x) => {
            return Err(Error::compile_error(
                x.span(),
                "ZKP programs don't support lifetimes.",
            ))
        }
        TypeParamBound::Verbatim(x) => {
            return Err(Error::compile_error(
                x.span(),
                "ZKP programs don't support verbatim.",
            ))
        }
        _ => {
            return Err(Error::compile_error(
                bound.span(),
                "Sunscreen doesn't understand this.",
            ))
        }
    };

    Ok((generic.ident.clone(), bound.path))
}

fn parse_inner(_attr_params: ZkpProgramAttrs, input_fn: ItemFn) -> Result<TokenStream> {
    let zkp_program_name = &input_fn.sig.ident;
    let vis = &input_fn.vis;
    let body = &input_fn.block;
    let inputs = &input_fn.sig.inputs;
    let ret = &input_fn.sig.output;

    let (generic_ident, generic_bound) = get_generic_arg(&input_fn.sig.generics)?;

    match ret {
        ReturnType::Default => {}
        _ => {
            return Err(Error::compile_error(
                ret.span(),
                "ZKP programs may not return values.",
            ));
        }
    };

    let mut is_shared = false;
    let mut public_seen = false;
    let mut constant_seen = false;
    let mut private_seen = false;
    let unwrapped_inputs = match extract_fn_arguments(inputs) {
        Ok(args) => {
            args.iter().map(|a| {
                let mut arg_kind = ArgumentKind::Private;

                match &a.0[..] {
                    [] => {
                        if public_seen || constant_seen {
                            return Err(Error::compile_error(a.2.span(),
                                "private arguments must be specified before #[public] and #[constant] arguments"
                            ));
                        }
                        private_seen = true;
                    },
                    [attr] => {
                        let ident = attr.path().get_ident();

                        match ident.map(|x| x.to_string()).as_deref() {
                            Some("shared") => {
                                if private_seen || public_seen || constant_seen {
                                    return Err(Error::compile_error(attr.path().span(),
                                        "#[shared] arguments must be specified before #[private], #[public] and #[constant] arguments"
                                    ));
                                }
                                arg_kind = ArgumentKind::Shared;
                                is_shared = true;
                            },
                            Some("private") => {
                                if public_seen || constant_seen {
                                    return Err(Error::compile_error(attr.path().span(),
                                        "#[private] arguments must be specified before #[public] and #[constant] arguments"
                                    ));
                                }
                                private_seen = true;
                            },
                            Some("public") => {
                                if constant_seen {
                                    return Err(Error::compile_error(attr.path().span(),
                                        "#[public] arguments must be specified before #[constant] arguments "
                                    ));
                                }
                                arg_kind = ArgumentKind::Public;
                                public_seen = true;
                            },
                            Some("constant") => {
                                arg_kind = ArgumentKind::Constant;
                                constant_seen = true;
                            },
                            _ => {
                                return Err(Error::compile_error(attr.path().span(), &format!(
                                    "Expected #[shared], #[private], #[public] or #[constant], found {}",
                                    attr.path().to_token_stream()
                                )));
                            }
                        }
                    },
                    [_, attr, ..] => {
                        return Err(Error::compile_error(attr.span(), "ZKP program arguments may only have one attribute (#[shared], #[private], #[public] or #[constant])."));
                    }
                };

                Ok((arg_kind, a.1, a.2))
            }).collect::<Result<Vec<(ArgumentKind, &Type, &Ident)>>>()?
        },
        Err(ExtractFnArgumentsError::ContainsSelf(s)) => Err(Error::compile_error(s, "ZKP programs must not contain `self`"))?,
        Err(ExtractFnArgumentsError::ContainsMut(s)) => Err(Error::compile_error(s, "ZKP program arguments cannot be `mut`"))?,
        Err(ExtractFnArgumentsError::IllegalPat(s)) => Err(Error::compile_error(s, "Expected Identifier"))?,
        Err(ExtractFnArgumentsError::IllegalType(s)) => Err(Error::compile_error(s, "ZKP program arguments must be an array or named struct type"))?,
    };

    let argument_types = unwrapped_inputs
        .iter()
        .map(|(_, t, _)| (**t).clone())
        .collect::<Vec<Type>>();

    let signature = emit_signature(&argument_types, &[]);

    let build_arg = format_ident!("shared_input");

    let var_decl = unwrapped_inputs.iter().map(|t| {
        let (input_type, input_arg) = match t.0 {
            ArgumentKind::Shared => ("shared_input", Some(&build_arg)),
            ArgumentKind::Private => ("private_input", None),
            ArgumentKind::Public => ("public_input", None),
            ArgumentKind::Constant => ("constant_input", None),
        };

        create_program_node(&t.2.to_string(), t.1, input_type, input_arg)
    });

    let zkp_program_struct_name =
        Ident::new(&format!("{}_struct", zkp_program_name), Span::call_site());

    let zkp_program_name_literal = format!("{}", zkp_program_name);

    let (associated_share_type, into_shared_variant, ext_impl) = if is_shared {
        (quote! { sunscreen::zkp::Shared }, quote! {Ok}, None)
    } else {
        (
            quote! { sunscreen::zkp::NotShared },
            quote! {Err},
            Some(quote! {
                impl sunscreen::ZkpProgramFnExt for #zkp_program_struct_name {}
            }),
        )
    };

    Ok(quote! {
        #[allow(non_camel_case_types)]
        #[derive(Clone)]
        #vis struct #zkp_program_struct_name;

        impl AsRef<str> for #zkp_program_struct_name {
            fn as_ref(&self) -> &str {
                #zkp_program_name_literal
            }
        }

        impl <#generic_ident: #generic_bound> sunscreen::ZkpProgramFn<#generic_ident> for #zkp_program_struct_name {
            type Share = #associated_share_type;

            fn build(&self, shared_input: <Self::Share as sunscreen::zkp::Share>::Input) -> sunscreen::Result<sunscreen::zkp::ZkpFrontendCompilation> {
                use std::cell::RefCell;
                use std::mem::transmute;
                use sunscreen::{Error, INDEX_ARENA, Result, types::{zkp::{ProgramNode, CreateSharedZkpProgramInput, CreateZkpProgramInput, ConstrainEq, IntoProgramNode}, TypeName}, zkp::{CURRENT_ZKP_CTX, ZkpContext, ZkpData}};

                let mut context = ZkpContext::new(ZkpData::new());

                CURRENT_ZKP_CTX.with(|ctx| {
                    // Transmute away the lifetime to 'static. So long as we are careful with internal()
                    // panicing, this is safe because we set the context back to none before the function
                    // returns.
                    ctx.swap(&RefCell::new(Some(unsafe { transmute(&mut context) })));

                    #[allow(clippy::type_complexity)]
                    #[forbid(unused_variables)]
                    let panic_res = std::panic::catch_unwind(|| {
                        #(#var_decl)*
                        #body
                    });

                    // when panicing or not, we need to clear our indicies arena and
                    // unset the context reference.
                    match panic_res {
                        Ok(v) => { },
                        Err(err) => {
                            INDEX_ARENA.with(|allocator| {
                                allocator.borrow_mut().reset()
                            });
                            ctx.swap(&RefCell::new(None));
                            std::panic::resume_unwind(err)
                        }
                    };

                    INDEX_ARENA.with(|allocator| {
                        allocator.borrow_mut().reset()
                    });
                    ctx.swap(&RefCell::new(None));
                });

                Ok(context.graph)
            }

            fn name(&self) -> &str {
                #zkp_program_name_literal
            }

            fn signature(&self) -> sunscreen::CallSignature {
                #signature
            }

            #[allow(clippy::type_complexity)]
            fn into_shared(self) -> Result<std::boxed::Box<dyn sunscreen::ZkpProgramFn<F, Share = sunscreen::zkp::Shared>>, std::boxed::Box<dyn sunscreen::ZkpProgramFn<F, Share = sunscreen::zkp::NotShared>>> {
                #into_shared_variant(std::boxed::Box::new(self))
            }
        }

        #ext_impl

        #[allow(non_upper_case_globals)]
        #vis const #zkp_program_name: #zkp_program_struct_name = #zkp_program_struct_name;
    })
}
