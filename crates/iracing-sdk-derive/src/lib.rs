//! Derive macros for automatic frame adapter generation.
//!
//! This crate provides the `IRacingTelemetryFrame` derive macro for automatically generating
//! `FrameAdapter` implementations.
//!
//! ## Supported field macros
//!
//! The derive macro recognizes these field-level attributes:
//!
//! - `#[field_name = "..."]` - bind a struct field to a telemetry variable
//! - `#[missing = "..."]` - provide an expression used when the telemetry value is absent
//! - `#[fail_if_missing]` - require the telemetry variable to exist at connection time
//! - `#[calculated = "..."]` - compute the field from a Rust expression at runtime
//! - `#[skip]` - exclude the field from telemetry extraction
//! - `#[bitfield(name = "...", has = "...")]` - extract a `bool` / `Option<bool>` from a bitfield mask
//! - `#[bitfield_map(name = "...", decoder = "...")]` - extract any `T` / `Option<T>` from a bitfield decoder
//!
//! ## Strategy summary
//!
//! - **Required fields**: `#[field_name = "Speed"]` - connection fails if missing
//! - **Optional fields**: `Option<T>` type with `#[field_name = "Gear"]`
//! - **Default values**: `#[field_name = "Fuel"] #[missing = "50.0"]`
//! - **Critical fields**: `#[field_name = "Temp"] #[fail_if_missing]`
//! - **Calculated fields**: `#[calculated = "42"]` - computed at runtime
//! - **Skipped fields**: `#[skip]` - application-managed, not from telemetry
//! - **Bitfield flag checks**: `#[bitfield(name = "...", has = "...")]`
//! - **Bitfield decoders**: `#[bitfield_map(name = "...", decoder = "...")]`
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use iracing_sdk_derive::IRacingTelemetryFrame;
//!
//! #[derive(IRacingTelemetryFrame, Debug)]
//! struct CarData {
//!     #[field_name = "Speed"]
//!     speed: f32,
//!
//!     #[field_name = "Gear"]
//!     gear: Option<i32>,
//!
//!     #[field_name = "FuelLevel"]
//!     #[missing = "100.0"]
//!     fuel: f32,
//!
//!     #[calculated = "std::time::Instant::now()"]
//!     timestamp: std::time::Instant,
//!
//!     #[skip]
//!     last_lap_time: f32,
//!
//!     #[bitfield(
//!         name = "SessionFlags",
//!         has = "iracing_sdk::SessionFlags::GREEN.bits()"
//!     )]
//!     is_green: bool,
//!
//!     #[bitfield_map(
//!         name = "SessionFlags",
//!         decoder = "iracing_sdk::session_dq_scoring_invalid"
//!     )]
//!     dq_scoring_invalid: bool,
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::fold::Fold;
use syn::parse::Parser;
use syn::{
    Attribute, DeriveInput, Expr, Field, Lit, LitInt, LitStr, Meta, Type, parse_macro_input,
};

/// Derive macro for automatic frame adapter generation.
///
/// Generates a `FrameAdapter` implementation with dual-phase validation:
/// 1. Connection-time schema validation with helpful error messages
/// 2. Runtime field extraction with zero HashMap lookups
#[proc_macro_derive(
    IRacingTelemetryFrame,
    attributes(
        field_name,
        missing,
        fail_if_missing,
        calculated,
        skip,
        bitfield,
        bitfield_map
    )
)]
pub fn derive_from_raw_frame(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_frame_adapter(&input) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error().into(),
    }
}

/// Generate the FrameAdapter implementation
fn generate_frame_adapter(input: &DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Extract fields from struct
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "Only named fields are supported",
                ));
            }
        },
        _ => return Err(syn::Error::new_spanned(input, "Only structs are supported")),
    };

    // Parse each field into strategies
    let mut field_strategies = Vec::new();
    for field in fields.iter() {
        let strategy = parse_field_strategy(field)?;
        field_strategies.push(strategy);
    }

    // Build lookup map for calculated expressions
    let mut telemetry_map: HashMap<String, (usize, syn::Type)> = HashMap::new();
    for (index, strategy) in field_strategies.iter().enumerate() {
        match strategy {
            FieldStrategy::TypeDefault {
                field_name,
                field_type,
                ..
            }
            | FieldStrategy::WithDefault {
                field_name,
                field_type,
                ..
            }
            | FieldStrategy::Critical {
                field_name,
                field_type,
                ..
            } => {
                telemetry_map.insert(field_name.clone(), (index, field_type.clone()));
            }
            FieldStrategy::Optional {
                field_name,
                inner_type,
                ..
            } => {
                telemetry_map.insert(field_name.clone(), (index, inner_type.clone()));
            }
            FieldStrategy::BitfieldHas { field_name: _, .. }
            | FieldStrategy::BitfieldMap { field_name: _, .. } => {
                // Bitfield variables have u32 underlying type (BitField). Calculated expressions rarely reference them directly; skip mapping.
            }
            FieldStrategy::Calculated { .. } | FieldStrategy::Skipped { .. } => {}
        }
    }

    // Generate validation phase code
    let (validation_checks, extraction_plan_items) = generate_validation_phase(&field_strategies);

    // Generate extraction phase code
    let extraction_assignments = generate_extraction_phase(&field_strategies, &telemetry_map)?;

    // Generate the complete implementation
    let expanded = quote! {
        impl #impl_generics ::iracing_sdk::adapters::FrameAdapter for #struct_name #ty_generics #where_clause {
            fn validate_schema(schema: &::iracing_sdk::VariableSchema) -> ::iracing_sdk::Result<::iracing_sdk::adapters::AdapterValidation> {
                use ::iracing_sdk::adapters::FieldExtraction;

                #(#validation_checks)*

                let extraction_plan = vec![#(#extraction_plan_items),*];
                Ok(::iracing_sdk::adapters::AdapterValidation::new(extraction_plan))
            }

            fn adapt(packet: &::iracing_sdk::types::FramePacket, validation: &::iracing_sdk::adapters::AdapterValidation) -> Self {
                use ::iracing_sdk::adapters::FieldExtraction;
                use ::iracing_sdk::VarData;
                let data = packet.data.as_ref();

                Self {
                    #(#extraction_assignments),*
                }
            }
        }
    };

    Ok(expanded.into())
}

/// Field strategy determined from attributes and type analysis.
enum FieldStrategy {
    /// Critical telemetry field that must exist in the schema.
    Critical {
        field_name: String,
        field_ident: syn::Ident,
        field_type: syn::Type,
    },
    /// Optional telemetry field represented as `Option<T>`.
    Optional {
        field_name: String,
        field_ident: syn::Ident,
        inner_type: syn::Type,
    },
    /// Telemetry field with an explicit `#[missing = "..."]` expression.
    WithDefault {
        field_name: String,
        field_ident: syn::Ident,
        field_type: syn::Type,
        default_expr: Expr,
    },
    /// Telemetry field that falls back to `<T as Default>::default()` when absent.
    TypeDefault {
        field_name: String,
        field_ident: syn::Ident,
        field_type: syn::Type,
    },
    /// Calculated field produced from a runtime expression.
    Calculated {
        field_ident: syn::Ident,
        expression: Expr,
        expression_str: String,
    },
    /// Bitfield single-bit extraction to bool/Option<bool> using mask
    BitfieldHas {
        field_name: String,
        field_ident: syn::Ident,
        target_is_option: bool,
        default_expr: Option<Expr>,
        fail_if_missing: bool,
        mask_expr: Expr,
    },
    /// Bitfield decode using a user-provided decoder: fn(BitField) -> T
    BitfieldMap {
        field_name: String,
        field_ident: syn::Ident,
        target_is_option: bool,
        default_expr: Option<Expr>,
        fail_if_missing: bool,
        decoder_expr: Expr,
    },
    /// Field managed entirely by application code.
    Skipped {
        field_ident: syn::Ident,
        field_type: syn::Type,
    },
}

fn generate_type_validation_check(
    probe_expr: proc_macro2::TokenStream,
    clone_expr: proc_macro2::TokenStream,
    field_name_lit: &str,
    target_type: &Type,
    treat_mismatch_as_missing: bool,
) -> proc_macro2::TokenStream {
    let mismatch_handler = if treat_mismatch_as_missing {
        quote! {
            None
        }
    } else {
        quote! {
            return Err(::iracing_sdk::IRacingSDKError::Parse {
                context: "Frame adapter validation".to_string(),
                details: format!(
                    "Field '{}' has incompatible telemetry type: {}",
                    #field_name_lit,
                    details
                ),
            });
        }
    };

    quote! {
        match ::iracing_sdk::adapters::telemetry_type_mismatch_details::<#target_type>(#probe_expr)? {
            Some(details) => { #mismatch_handler }
            None => Some(#clone_expr),
        }
    }
}

/// Parse a single field into its strategy
fn parse_field_strategy(field: &Field) -> syn::Result<FieldStrategy> {
    let field_ident = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new_spanned(field, "Field must have a name"))?;
    let field_type = field.ty.clone();

    // Check for bitfield-style attributes first
    if let Some(bit_attr) = parse_bitfield_attr(field)? {
        // Common toggles also supported on bitfield fields
        let mut default_value: Option<String> = None;
        let mut fail_if_missing = false;
        for attr in &field.attrs {
            if let Ok(attr_value) = parse_attribute(attr) {
                match attr_value {
                    AttributeValue::Missing(value) => default_value = Some(value),
                    AttributeValue::FailIfMissing => fail_if_missing = true,
                    _ => {}
                }
            }
        }

        let (target_is_option, _inner_ty) = if let Some(inner) = extract_option_type(&field_type) {
            (true, inner)
        } else {
            (false, field_type.clone())
        };

        match bit_attr {
            BitfieldAttr::Has { name, mask } => {
                // Validate target type: bool or Option<bool>
                let is_bool = if target_is_option {
                    extract_option_type(&field_type).map(|t| quote::quote!(#t).to_string())
                        == Some("bool".to_string())
                } else {
                    quote::quote!(#field_type).to_string() == "bool"
                };
                if !is_bool {
                    return Err(syn::Error::new_spanned(
                        &field.ty,
                        "#[bitfield(..., has = ...)] requires field type bool or Option<bool>",
                    ));
                }
                let mask_expr: Expr = syn::parse_str(&mask)?;
                let default_expr = if let Some(s) = default_value {
                    Some(syn::parse_str(&s)?)
                } else {
                    None
                };
                return Ok(FieldStrategy::BitfieldHas {
                    field_name: name,
                    field_ident,
                    target_is_option,
                    default_expr,
                    fail_if_missing,
                    mask_expr,
                });
            }
            BitfieldAttr::Map { name, decoder } => {
                // Any target type T / Option<T>
                let decoder_expr: Expr = syn::parse_str(&decoder)?;
                let default_expr = if let Some(s) = default_value {
                    Some(syn::parse_str(&s)?)
                } else {
                    None
                };
                return Ok(FieldStrategy::BitfieldMap {
                    field_name: name,
                    field_ident,
                    target_is_option,
                    default_expr,
                    fail_if_missing,
                    decoder_expr,
                });
            }
        }
    }

    // Parse non-bitfield attributes
    let mut field_name: Option<String> = None;
    let mut default_value: Option<String> = None;
    let mut fail_if_missing = false;
    let mut calculated: Option<String> = None;
    let mut skip = false;

    for attr in &field.attrs {
        if let Ok(attr_value) = parse_attribute(attr) {
            match attr_value {
                AttributeValue::FieldName(name) => field_name = Some(name),
                AttributeValue::Missing(value) => default_value = Some(value),
                AttributeValue::FailIfMissing => fail_if_missing = true,
                AttributeValue::Calculated(expr) => calculated = Some(expr),
                AttributeValue::Skip => skip = true,
            }
        }
    }

    if skip {
        return Ok(FieldStrategy::Skipped {
            field_ident,
            field_type,
        });
    }

    if let Some(expr_str) = calculated {
        let expression: Expr = syn::parse_str(&expr_str)?;
        return Ok(FieldStrategy::Calculated {
            field_ident,
            expression,
            expression_str: expr_str,
        });
    }

    let field_name = field_name.ok_or_else(|| {
        syn::Error::new_spanned(
            field,
            "Missing #[field_name = \"...\"] attribute. Use #[skip] for non-telemetry fields.",
        )
    })?;

    if fail_if_missing {
        return Ok(FieldStrategy::Critical {
            field_name,
            field_ident,
            field_type,
        });
    }

    if let Some(inner_type) = extract_option_type(&field_type) {
        return Ok(FieldStrategy::Optional {
            field_name,
            field_ident,
            inner_type,
        });
    }

    if let Some(default_str) = default_value {
        let default_expr: Expr = syn::parse_str(&default_str)?;
        return Ok(FieldStrategy::WithDefault {
            field_name,
            field_ident,
            field_type,
            default_expr,
        });
    }

    Ok(FieldStrategy::TypeDefault {
        field_name,
        field_ident,
        field_type,
    })
}

/// Parsed attribute values
#[derive(Debug)]
enum AttributeValue {
    FieldName(String),
    Missing(String),
    FailIfMissing,
    Calculated(String),
    Skip,
}

/// Parsed bitfield attributes
#[derive(Debug)]
enum BitfieldAttr {
    Has { name: String, mask: String },
    Map { name: String, decoder: String },
}

fn parse_bitfield_attr(field: &Field) -> syn::Result<Option<BitfieldAttr>> {
    use syn::punctuated::Punctuated;
    use syn::{Meta, MetaNameValue, Token};

    for attr in &field.attrs {
        if let Meta::List(list) = &attr.meta {
            if list.path.is_ident("bitfield") {
                let mut name: Option<String> = None;
                let mut mask: Option<String> = None;
                let pairs: Punctuated<MetaNameValue, Token![,]> =
                    Punctuated::parse_terminated.parse2(list.tokens.clone())?;
                for nv in pairs {
                    if nv.path.is_ident("name") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = nv.value
                        {
                            name = Some(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "bitfield name must be a string literal",
                            ));
                        }
                    } else if nv.path.is_ident("has") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = nv.value
                        {
                            mask = Some(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "bitfield has must be a string literal expression path",
                            ));
                        }
                    }
                }
                let name = name.ok_or_else(|| {
                    syn::Error::new_spanned(attr, "bitfield requires name = \"...\"")
                })?;
                let mask = mask.ok_or_else(|| {
                    syn::Error::new_spanned(attr, "bitfield requires has = \"...\"")
                })?;
                return Ok(Some(BitfieldAttr::Has { name, mask }));
            } else if list.path.is_ident("bitfield_map") {
                let mut name: Option<String> = None;
                let mut decoder: Option<String> = None;
                let pairs: Punctuated<MetaNameValue, Token![,]> =
                    Punctuated::parse_terminated.parse2(list.tokens.clone())?;
                for nv in pairs {
                    if nv.path.is_ident("name") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = nv.value
                        {
                            name = Some(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "bitfield_map name must be a string literal",
                            ));
                        }
                    } else if nv.path.is_ident("decoder") {
                        if let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Str(s),
                            ..
                        }) = nv.value
                        {
                            decoder = Some(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                &nv.value,
                                "bitfield_map decoder must be a string literal path",
                            ));
                        }
                    }
                }
                let name = name.ok_or_else(|| {
                    syn::Error::new_spanned(attr, "bitfield_map requires name = \"...\"")
                })?;
                let decoder = decoder.ok_or_else(|| {
                    syn::Error::new_spanned(attr, "bitfield_map requires decoder = \"path\"")
                })?;
                return Ok(Some(BitfieldAttr::Map { name, decoder }));
            }
        }
    }
    Ok(None)
}

/// Parse a single attribute
fn parse_attribute(attr: &Attribute) -> syn::Result<AttributeValue> {
    match &attr.meta {
        Meta::NameValue(name_value) if name_value.path.is_ident("field_name") => {
            if let Expr::Lit(expr_lit) = &name_value.value {
                if let Lit::Str(lit_str) = &expr_lit.lit {
                    Ok(AttributeValue::FieldName(lit_str.value()))
                } else {
                    Err(syn::Error::new_spanned(
                        &name_value.value,
                        "field_name must be a string literal",
                    ))
                }
            } else {
                Err(syn::Error::new_spanned(
                    &name_value.value,
                    "field_name must be a string literal",
                ))
            }
        }
        Meta::NameValue(name_value) if name_value.path.is_ident("missing") => {
            if let Expr::Lit(expr_lit) = &name_value.value {
                if let Lit::Str(lit_str) = &expr_lit.lit {
                    Ok(AttributeValue::Missing(lit_str.value()))
                } else {
                    Err(syn::Error::new_spanned(
                        &name_value.value,
                        "missing must be a string literal",
                    ))
                }
            } else {
                Err(syn::Error::new_spanned(
                    &name_value.value,
                    "missing must be a string literal",
                ))
            }
        }
        Meta::NameValue(name_value) if name_value.path.is_ident("default") => {
            Err(syn::Error::new_spanned(
                &name_value.path,
                "`#[default = ...]` is reserved by Rust when deriving Default. Use `#[missing = ...]` instead.",
            ))
        }
        Meta::NameValue(name_value) if name_value.path.is_ident("calculated") => {
            if let Expr::Lit(expr_lit) = &name_value.value {
                if let Lit::Str(lit_str) = &expr_lit.lit {
                    Ok(AttributeValue::Calculated(lit_str.value()))
                } else {
                    Err(syn::Error::new_spanned(
                        &name_value.value,
                        "calculated must be a string literal",
                    ))
                }
            } else {
                Err(syn::Error::new_spanned(
                    &name_value.value,
                    "calculated must be a string literal",
                ))
            }
        }
        Meta::Path(path) if path.is_ident("fail_if_missing") => Ok(AttributeValue::FailIfMissing),
        Meta::Path(path) if path.is_ident("skip") => Ok(AttributeValue::Skip),
        _ => Err(syn::Error::new_spanned(attr, "Unknown attribute")),
    }
}

/// Extract inner type from Option<T>
fn extract_option_type(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        let last_segment = type_path.path.segments.last()?;
        if last_segment.ident == "Option" {
            if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                    return Some(inner_type.clone());
                }
            }
        }
    }
    None
}

/// Generate validation phase code
fn generate_validation_phase(
    strategies: &[FieldStrategy],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut validation_checks = Vec::new();
    let mut extraction_plan_items = Vec::new();

    for (index, strategy) in strategies.iter().enumerate() {
        match strategy {
            FieldStrategy::TypeDefault {
                field_name,
                field_type,
                ..
            } => {
                let field_name_lit = field_name;
                let var_name = format_ident!("var_info_{}", index);
                let validated_name = format_ident!("validated_var_info_{}", index);
                let type_check = generate_type_validation_check(
                    quote!(var_info),
                    quote!((*var_info).clone()),
                    field_name_lit,
                    field_type,
                    true,
                );

                validation_checks.push(quote! {
                    let #var_name = schema.get_variable(#field_name_lit);
                    let #validated_name = match #var_name {
                        Some(var_info) => { #type_check }
                        None => None,
                    };
                });

                extraction_plan_items.push(quote! {
                    FieldExtraction::WithDefault {
                        name: #field_name_lit.to_string(),
                        var_info: #validated_name,
                        default_value: ::iracing_sdk::adapters::DefaultValue::TypeDefault,
                    }
                });
            }
            FieldStrategy::WithDefault {
                field_name,
                default_expr,
                field_type,
                ..
            } => {
                let field_name_lit = field_name;
                let var_name = format_ident!("var_info_{}", index);
                let validated_name = format_ident!("validated_var_info_{}", index);
                let default_repr = quote!(#default_expr).to_string();
                let default_repr_lit = LitStr::new(&default_repr, proc_macro2::Span::call_site());
                let type_check = generate_type_validation_check(
                    quote!(var_info),
                    quote!((*var_info).clone()),
                    field_name_lit,
                    field_type,
                    true,
                );

                validation_checks.push(quote! {
                    let #var_name = schema.get_variable(#field_name_lit);
                    let #validated_name = match #var_name {
                        Some(var_info) => { #type_check }
                        None => None,
                    };
                });

                extraction_plan_items.push(quote! {
                    FieldExtraction::WithDefault {
                        name: #field_name_lit.to_string(),
                        var_info: #validated_name,
                        default_value: ::iracing_sdk::adapters::DefaultValue::ExplicitExpression(#default_repr_lit.to_string()),
                    }
                });
            }
            FieldStrategy::Optional {
                field_name,
                inner_type,
                ..
            } => {
                let field_name_lit = field_name;
                let var_name = format_ident!("var_info_{}", index);
                let validated_name = format_ident!("validated_var_info_{}", index);
                let type_check = generate_type_validation_check(
                    quote!(var_info),
                    quote!((*var_info).clone()),
                    field_name_lit,
                    inner_type,
                    true,
                );

                validation_checks.push(quote! {
                    let #var_name = schema.get_variable(#field_name_lit);
                    let #validated_name = match #var_name {
                        Some(var_info) => { #type_check }
                        None => None,
                    };
                });

                extraction_plan_items.push(quote! {
                    FieldExtraction::Optional {
                        name: #field_name_lit.to_string(),
                        var_info: #validated_name,
                    }
                });
            }
            FieldStrategy::Critical {
                field_name,
                field_type,
                ..
            } => {
                let field_name_lit = field_name;
                validation_checks.push(quote! {
                    if !schema.variables.contains_key(#field_name_lit) {
                        let available_fields: Vec<String> = schema.variables.keys().cloned().collect();
                        return Err(::iracing_sdk::IRacingSDKError::Parse {
                            context: "Frame adapter validation".to_string(),
                            details: format!("Critical field '{}' is missing from schema. Connection aborted. Available fields: {}",
                                #field_name_lit, available_fields.join(", ")),
                        });
                    }
                });

                let var_name = format_ident!("var_info_{}", index);
                let validated_name = format_ident!("validated_var_info_{}", index);
                let type_check = generate_type_validation_check(
                    quote!(&#var_name),
                    quote!(#var_name.clone()),
                    field_name_lit,
                    field_type,
                    false,
                );
                validation_checks.push(quote! {
                    let #var_name = schema.get_variable(#field_name_lit).unwrap().clone();
                    let #validated_name = {
                        #type_check
                    }.unwrap();
                });

                extraction_plan_items.push(quote! {
                    FieldExtraction::Required {
                        name: #field_name_lit.to_string(),
                        var_info: #validated_name,
                    }
                });
            }
            FieldStrategy::BitfieldHas {
                field_name,
                target_is_option,
                default_expr,
                fail_if_missing,
                ..
            }
            | FieldStrategy::BitfieldMap {
                field_name,
                target_is_option,
                default_expr,
                fail_if_missing,
                ..
            } => {
                let field_name_lit = field_name;
                let var_name = format_ident!("var_info_{}", index);
                let validated_name = format_ident!("validated_var_info_{}", index);
                let type_check = generate_type_validation_check(
                    quote!(var_info),
                    quote!((*var_info).clone()),
                    field_name_lit,
                    &syn::parse_quote!(::iracing_sdk::BitField),
                    !*fail_if_missing,
                );
                if *fail_if_missing {
                    // Override to Required path (strongest semantics)
                    validation_checks.push(quote! {
                        if !schema.variables.contains_key(#field_name_lit) {
                            let available_fields: Vec<String> = schema.variables.keys().cloned().collect();
                            return Err(::iracing_sdk::IRacingSDKError::Parse {
                                context: "Frame adapter validation".to_string(),
                                details: format!("Critical field '{}' is missing from schema. Connection aborted. Available fields: {}",
                                    #field_name_lit, available_fields.join(", ")),
                            });
                        }
                    });
                    validation_checks.push(quote! {
                        let #var_name = schema.get_variable(#field_name_lit).unwrap().clone();
                        let #validated_name = match ::iracing_sdk::adapters::telemetry_type_mismatch_details::<::iracing_sdk::BitField>(&#var_name)? {
                            Some(details) => {
                                return Err(::iracing_sdk::IRacingSDKError::Parse {
                                    context: "Frame adapter validation".to_string(),
                                    details: format!(
                                        "Field '{}' has incompatible telemetry type: {}",
                                        #field_name_lit,
                                        details
                                    ),
                                });
                            }
                            None => #var_name.clone(),
                        };
                    });
                    extraction_plan_items.push(quote! {
                        FieldExtraction::Required { name: #field_name_lit.to_string(), var_info: #validated_name }
                    });
                } else {
                    validation_checks.push(quote! {
                        let #var_name = schema.get_variable(#field_name_lit);
                    });

                    if *target_is_option {
                        validation_checks.push(quote! {
                            let #validated_name = match #var_name {
                                Some(var_info) => { #type_check }
                                None => None,
                            };
                        });
                        extraction_plan_items.push(quote! {
                            FieldExtraction::Optional { name: #field_name_lit.to_string(), var_info: #validated_name }
                        });
                    } else if default_expr.is_some() {
                        let default_repr = quote!(#default_expr).to_string();
                        let default_repr_lit =
                            LitStr::new(&default_repr, proc_macro2::Span::call_site());
                        validation_checks.push(quote! {
                            let #validated_name = match #var_name {
                                Some(var_info) => { #type_check }
                                None => None,
                            };
                        });
                        extraction_plan_items.push(quote! {
                            FieldExtraction::WithDefault {
                                name: #field_name_lit.to_string(),
                                var_info: #validated_name,
                                default_value: ::iracing_sdk::adapters::DefaultValue::ExplicitExpression(#default_repr_lit.to_string()),
                            }
                        });
                    } else {
                        validation_checks.push(quote! {
                            let #validated_name = match #var_name {
                                Some(var_info) => { #type_check }
                                None => None,
                            };
                        });
                        extraction_plan_items.push(quote! {
                            FieldExtraction::WithDefault {
                                name: #field_name_lit.to_string(),
                                var_info: #validated_name,
                                default_value: ::iracing_sdk::adapters::DefaultValue::TypeDefault,
                            }
                        });
                    }
                }
            }
            FieldStrategy::Calculated { expression_str, .. } => {
                extraction_plan_items.push(quote! {
                    FieldExtraction::Calculated {
                        expression: #expression_str.to_string(),
                    }
                });
            }
            FieldStrategy::Skipped { .. } => {
                extraction_plan_items.push(quote! {
                    FieldExtraction::Skipped
                });
            }
        }
    }

    (validation_checks, extraction_plan_items)
}

/// Process calculated field expressions to replace telemetry field names with extraction calls
fn process_calculated_expression(
    expr: &Expr,
    field_map: &HashMap<String, (usize, Type)>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mut folder = CalculatedExprFolder { field_map };
    let rewritten = folder.fold_expr(expr.clone());
    Ok(quote! { #rewritten })
}

/// Rewrites calculated expressions at compile time so they reuse the runtime extraction plan.
struct CalculatedExprFolder<'a> {
    field_map: &'a HashMap<String, (usize, Type)>,
}

impl<'a> Fold for CalculatedExprFolder<'a> {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Path(expr_path)
                if expr_path.qself.is_none() && expr_path.path.segments.len() == 1 =>
            {
                if let Some(ident) = expr_path.path.get_ident() {
                    let ident_str = ident.to_string();
                    if let Some((_, ty)) = self.field_map.get(&ident_str) {
                        let name_lit = LitStr::new(&ident_str, ident.span());
                        let ty = ty.clone();
                        return syn::parse_quote! {
                            validation.fetch_or_default::<#ty>(packet, #name_lit)
                        };
                    }
                }
                Expr::Path(expr_path)
            }
            other => syn::fold::fold_expr(self, other),
        }
    }
}

/// Generate field assignment for TypeDefault strategy
fn generate_type_default_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;
    quote! {
        #field_ident: {
            match validation.extraction_plan.get(#index_lit) {
                Some(::iracing_sdk::adapters::FieldExtraction::WithDefault { var_info, .. }) => {
                    if let Some(var_info) = var_info {
                        match <#field_type as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                            Ok(value) => value,
                            Err(_e) => {
                                static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                WARNED.call_once(|| {
                                    ::tracing::warn!(
                                        field = #field_name_lit,
                                        expected_type = ::std::any::type_name::<#field_type>(),
                                        actual_type = ?var_info.data_type,
                                        error = ?_e,
                                        "Type mismatch: failed to convert field, using default value (warning shown once)"
                                    );
                                });
                                <#field_type as ::core::default::Default>::default()
                            }
                        }
                    } else {
                        <#field_type as ::core::default::Default>::default()
                    }
                }
                _ => <#field_type as ::core::default::Default>::default(),
            }
        }
    }
}

/// Generate field assignment for WithDefault strategy
fn generate_with_default_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    default_expr: &Expr,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;
    quote! {
        #field_ident: {
            let fallback = || -> #field_type { #default_expr };
            match validation.extraction_plan.get(#index_lit) {
                Some(::iracing_sdk::adapters::FieldExtraction::WithDefault { var_info, .. }) => {
                    if let Some(var_info) = var_info {
                        match <#field_type as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                            Ok(value) => value,
                            Err(_e) => {
                                static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                WARNED.call_once(|| {
                                    ::tracing::warn!(
                                        field = #field_name_lit,
                                        expected_type = ::std::any::type_name::<#field_type>(),
                                        actual_type = ?var_info.data_type,
                                        error = ?_e,
                                        "Type mismatch: failed to convert field, using default value (warning shown once)"
                                    );
                                });
                                fallback()
                            }
                        }
                    } else {
                        fallback()
                    }
                }
                _ => fallback(),
            }
        }
    }
}

/// Generate field assignment for Optional strategy
fn generate_optional_assignment(
    index: usize,
    field_ident: &syn::Ident,
    inner_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;
    quote! {
        #field_ident: {
            match validation.extraction_plan.get(#index_lit) {
                Some(::iracing_sdk::adapters::FieldExtraction::Optional { var_info, .. }) => {
                    if let Some(var_info) = var_info {
                        match <#inner_type as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                            Ok(value) => Some(value),
                            Err(_e) => {
                                static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                WARNED.call_once(|| {
                                    ::tracing::warn!(
                                        field = #field_name_lit,
                                        expected_type = ::std::any::type_name::<#inner_type>(),
                                        actual_type = ?var_info.data_type,
                                        error = ?_e,
                                        "Type mismatch: failed to convert optional field, using None (warning shown once)"
                                    );
                                });
                                None
                            }
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
    }
}

/// Generate field assignment for Critical strategy
fn generate_critical_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;
    quote! {
        #field_ident: {
            match validation.extraction_plan.get(#index_lit) {
                Some(::iracing_sdk::adapters::FieldExtraction::Required { name, var_info }) => {
                    match <#field_type as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                        Ok(value) => value,
                        Err(err) => panic!("Failed to decode critical field '{}' during adapt: {err:?}", name),
                    }
                }
                Some(other) => panic!("Validation plan entry for '{}' is {:?}, expected Required", #field_name_lit, other),
                None => panic!("Validation plan missing required field '{}'", #field_name_lit),
            }
        }
    }
}

/// Generate field assignment for BitfieldHas strategy
fn generate_bitfield_has_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_name: &str,
    target_is_option: bool,
    default_expr: &Option<Expr>,
    mask_expr: &Expr,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;

    if target_is_option {
        quote! {
            #field_ident: {
                match validation.extraction_plan.get(#index_lit) {
                    Some(::iracing_sdk::adapters::FieldExtraction::Optional { var_info, .. }) => {
                        if let Some(var_info) = var_info {
                            match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                                Ok(bits) => Some(bits.has_flag(#mask_expr)),
                                Err(_e) => {
                                    static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                    WARNED.call_once(|| {
                                        ::tracing::warn!(
                                            field = #field_name_lit,
                                            expected_type = "BitField",
                                            actual_type = ?var_info.data_type,
                                            error = ?_e,
                                            "Type mismatch: failed to convert bitfield, using None (warning shown once)"
                                        );
                                    });
                                    None
                                }
                            }
                        } else { None }
                    }
                    _ => None,
                }
            }
        }
    } else {
        let fallback_bool = if let Some(expr) = default_expr {
            quote! { #expr }
        } else {
            quote! { false }
        };
        quote! {
            #field_ident: {
                match validation.extraction_plan.get(#index_lit) {
                    Some(::iracing_sdk::adapters::FieldExtraction::Required { var_info, .. }) => {
                        match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                            Ok(bits) => bits.has_flag(#mask_expr),
                            Err(err) => panic!("Failed to decode critical bitfield during adapt: {err:?}"),
                        }
                    }
                    Some(::iracing_sdk::adapters::FieldExtraction::WithDefault { var_info, .. }) => {
                        if let Some(var_info) = var_info {
                            match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                                Ok(bits) => bits.has_flag(#mask_expr),
                                Err(_e) => {
                                    static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                    WARNED.call_once(|| {
                                        ::tracing::warn!(
                                            field = #field_name_lit,
                                            expected_type = "BitField",
                                            actual_type = ?var_info.data_type,
                                            error = ?_e,
                                            "Type mismatch: failed to convert bitfield, using default value (warning shown once)"
                                        );
                                    });
                                    #fallback_bool
                                }
                            }
                        } else { #fallback_bool }
                    }
                    _ => { #fallback_bool },
                }
            }
        }
    }
}

/// Generate field assignment for BitfieldMap strategy
fn generate_bitfield_map_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_name: &str,
    target_is_option: bool,
    default_expr: &Option<Expr>,
    decoder_expr: &Expr,
) -> proc_macro2::TokenStream {
    let index_lit = LitInt::new(&index.to_string(), proc_macro2::Span::call_site());
    let field_name_lit = field_name;

    if target_is_option {
        quote! {
            #field_ident: {
                match validation.extraction_plan.get(#index_lit) {
                    Some(::iracing_sdk::adapters::FieldExtraction::Optional { var_info, .. }) => {
                        if let Some(var_info) = var_info {
                            match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                                Ok(bits) => Some((#decoder_expr)(bits)),
                                Err(_e) => {
                                    static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                    WARNED.call_once(|| {
                                        ::tracing::warn!(
                                            field = #field_name_lit,
                                            expected_type = "BitField",
                                            actual_type = ?var_info.data_type,
                                            error = ?_e,
                                            "Type mismatch: failed to convert bitfield, using None (warning shown once)"
                                        );
                                    });
                                    None
                                }
                            }
                        } else { None }
                    }
                    _ => None,
                }
            }
        }
    } else {
        let fallback_expr = if let Some(expr) = default_expr {
            quote! { #expr }
        } else {
            quote! { ::core::default::Default::default() }
        };
        quote! {
            #field_ident: {
                match validation.extraction_plan.get(#index_lit) {
                    Some(::iracing_sdk::adapters::FieldExtraction::Required { var_info, .. }) => {
                        match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                            Ok(bits) => (#decoder_expr)(bits),
                            Err(err) => panic!("Failed to decode critical bitfield during adapt: {err:?}"),
                        }
                    }
                    Some(::iracing_sdk::adapters::FieldExtraction::WithDefault { var_info, .. }) => {
                        if let Some(var_info) = var_info {
                            match <::iracing_sdk::BitField as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
                                Ok(bits) => (#decoder_expr)(bits),
                                Err(_e) => {
                                    static WARNED: ::std::sync::Once = ::std::sync::Once::new();
                                    WARNED.call_once(|| {
                                        ::tracing::warn!(
                                            field = #field_name_lit,
                                            expected_type = "BitField",
                                            actual_type = ?var_info.data_type,
                                            error = ?_e,
                                            "Type mismatch: failed to convert bitfield, using default value (warning shown once)"
                                        );
                                    });
                                    #fallback_expr
                                }
                            }
                        } else { #fallback_expr }
                    }
                    _ => { #fallback_expr },
                }
            }
        }
    }
}

/// Generate extraction phase code
fn generate_extraction_phase(
    strategies: &[FieldStrategy],
    telemetry_map: &HashMap<String, (usize, syn::Type)>,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut assignments = Vec::new();

    for (index, strategy) in strategies.iter().enumerate() {
        let assignment = match strategy {
            FieldStrategy::TypeDefault {
                field_ident,
                field_type,
                field_name,
            } => generate_type_default_assignment(index, field_ident, field_type, field_name),
            FieldStrategy::WithDefault {
                field_ident,
                field_type,
                default_expr,
                field_name,
            } => generate_with_default_assignment(
                index,
                field_ident,
                field_type,
                default_expr,
                field_name,
            ),
            FieldStrategy::Optional {
                field_ident,
                inner_type,
                field_name,
            } => generate_optional_assignment(index, field_ident, inner_type, field_name),
            FieldStrategy::Critical {
                field_ident,
                field_type,
                field_name,
            } => generate_critical_assignment(index, field_ident, field_type, field_name),
            FieldStrategy::BitfieldHas {
                field_ident,
                field_name,
                target_is_option,
                default_expr,
                mask_expr,
                ..
            } => generate_bitfield_has_assignment(
                index,
                field_ident,
                field_name,
                *target_is_option,
                default_expr,
                mask_expr,
            ),
            FieldStrategy::BitfieldMap {
                field_ident,
                field_name,
                target_is_option,
                default_expr,
                decoder_expr,
                ..
            } => generate_bitfield_map_assignment(
                index,
                field_ident,
                field_name,
                *target_is_option,
                default_expr,
                decoder_expr,
            ),

            FieldStrategy::Calculated {
                field_ident,
                expression,
                ..
            } => {
                let rewritten =
                    process_calculated_expression(expression, telemetry_map)?;
                quote! {
                    #field_ident: { #rewritten }
                }
            }
            FieldStrategy::Skipped {
                field_ident,
                field_type,
            } => {
                quote! {
                    #field_ident: <#field_type as ::core::default::Default>::default()
                }
            }
        };

        assignments.push(assignment);
    }

    Ok(assignments)
}
