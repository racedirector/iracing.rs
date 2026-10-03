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
//! ```no_run
//! use iracing_sdk_derive::IRacingTelemetryFrame;
//!
//! fn session_dq_scoring_invalid(bits: iracing_sdk::BitField) -> bool {
//!     iracing_sdk::irsdk::SessionFlags::from(bits)
//!         .has_disqualification_scoring_invalid()
//! }
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
//!         has = "iracing_sdk::irsdk::SessionFlags::GREEN.bits()"
//!     )]
//!     is_green: bool,
//!
//!     #[bitfield_map(
//!         name = "SessionFlags",
//!         decoder = "session_dq_scoring_invalid"
//!     )]
//!     dq_scoring_invalid: bool,
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::fold::Fold;
use syn::parse::Parser;
use syn::{Attribute, DeriveInput, Expr, Field, Lit, Meta, Type, parse_macro_input};

/// Derive macro that implements `::iracing_sdk::adapters::FrameAdapter` for structs with named fields.
///
/// Generates an implementation that performs two phases:
/// 1. Connection-time layout validation producing an ordered extraction plan.
/// 2. Runtime adaptation that decodes packet bytes into the struct fields using the plan.
///
/// # Examples
///
/// ```no_run
/// use iracing_sdk_derive::IRacingTelemetryFrame;
///
/// #[derive(IRacingTelemetryFrame)]
/// struct SimpleFrame {
///     #[field_name = "Speed"]
///     speed: f32,
///     #[field_name = "Gear"]
///     gear: Option<i32>,
/// }
/// ```
///
/// The generated impl validates a provided `TelemetryLayout` and produces an `AdapterValidation`
/// which `adapt` uses to populate `SimpleFrame` from a `FramePacket`.
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

/// Generate the `FrameAdapter` implementation for the provided derive input.
///
/// Parses the given `DeriveInput` (must be a struct with named fields), computes per-field
/// extraction strategies, builds a telemetry lookup map for calculated expressions,
/// and emits the `impl ::iracing_sdk::adapters::FrameAdapter` token stream which contains
/// `validate_layout` and `adapt` implementations tailored to the struct's fields and attributes.
///
/// The function returns a `syn::Error` on unsupported inputs (non-struct or non-named fields)
/// or on invalid/malformed per-field attributes.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::DeriveInput;
///
/// let input: DeriveInput = syn::parse_quote! {
///     struct S {
///         #[field_name = "Speed"]
///         speed: f32,
///     }
/// };
/// let _tokens = generate_frame_adapter(&input)?;
/// ```
/* no outer attributes */
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
            FieldStrategy::BitfieldHas { .. } | FieldStrategy::BitfieldMap { .. } => {
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
            fn validate_layout(layout: &::std::sync::Arc<::iracing_sdk::TelemetryLayout>) -> ::iracing_sdk::Result<::iracing_sdk::adapters::AdapterValidation> {
                use ::iracing_sdk::adapters::FieldExtraction;

                #(#validation_checks)*

                let extraction_plan = vec![#(#extraction_plan_items),*];
                Ok(::iracing_sdk::adapters::AdapterValidation::new(::std::sync::Arc::clone(layout), extraction_plan))
            }

            fn adapt(packet: &::iracing_sdk::types::FramePacket, validation: &::iracing_sdk::adapters::AdapterValidation) -> Self {
                let validated_frame = validation.for_packet(packet).expect("adapter layout mismatch");

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
    /// Critical telemetry field that must exist in the layout.
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

/// Determine the extraction strategy for a single struct field from its attributes and type.
///
/// This inspects bitfield-specific attributes (`#[bitfield(...)]`, `#[bitfield_map(...)]`) first,
/// then regular attributes (`#[field_name = "..."]`, `#[missing = "..."]`, `#[fail_if_missing]`,
/// `#[calculated = "..."]`, `#[skip]`) and the Rust type to produce one of the `FieldStrategy`
/// variants:
/// - `Skipped` when `#[skip]` is present.
/// - `Calculated` when `#[calculated = "..."]` is present.
/// - Bitfield strategies (`BitfieldHas` / `BitfieldMap`) when corresponding `#[bitfield*]` attrs exist.
/// - `Critical` when `#[fail_if_missing]` is set for a non-`Option` telemetry field.
/// - `Optional` for `Option<T>` telemetry-backed fields.
/// - `WithDefault` when `#[missing = "..."]` is provided for a non-`Option` field.
/// - `TypeDefault` when the field is telemetry-backed but has no explicit missing/default handling.
///
/// The function returns a `syn::Error` when required attribute forms or literal values are missing
/// or malformed, or when bitfield target types are incompatible (for example, `#[bitfield(..., has = ...)]`
/// requires `bool` or `Option<bool>`).
///
/// # Examples
///
/// ```rust,ignore
/// use syn::{Field, Result};
/// // parse a field with a telemetry name into a syn::Field and derive its strategy
/// let field: Field = syn::parse_quote! {
///     #[field_name = "RPM"]
///     pub rpm: Option<u32>
/// };
/// let strat = parse_field_strategy(&field).unwrap();
/// assert!(matches!(strat, FieldStrategy::Optional { .. }));
/// ```
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
            if !is_bitfield_common_attribute(attr) {
                continue;
            }

            match parse_attribute(attr)? {
                AttributeValue::Missing(value) => default_value = Some(value),
                AttributeValue::FailIfMissing => fail_if_missing = true,
                _ => {}
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
        if !is_regular_field_attribute(attr) {
            continue;
        }

        match parse_attribute(attr)? {
            AttributeValue::FieldName(name) => field_name = Some(name),
            AttributeValue::Missing(value) => default_value = Some(value),
            AttributeValue::FailIfMissing => fail_if_missing = true,
            AttributeValue::Calculated(expr) => calculated = Some(expr),
            AttributeValue::Skip => skip = true,
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

    let option_inner_type = extract_option_type(&field_type);
    if fail_if_missing {
        if option_inner_type.is_some() {
            return Err(syn::Error::new_spanned(
                &field.ty,
                "#[fail_if_missing] cannot be used on Option<T> fields",
            ));
        }

        return Ok(FieldStrategy::Critical {
            field_name,
            field_ident,
            field_type,
        });
    }

    if let Some(inner_type) = option_inner_type {
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

/// Parses a field's attributes for a `#[bitfield(...)]` or `#[bitfield_map(...)]` directive.
///
/// Returns `Some(BitfieldAttr::Has { name, mask })` when a `#[bitfield(name = "...", has = "...")]`
/// attribute is found, `Some(BitfieldAttr::Map { name, decoder })` when a
/// `#[bitfield_map(name = "...", decoder = "...")]` attribute is found, and `None` when neither
/// attribute is present. Returns a `syn::Error` if the attribute is present but malformed
/// (missing required keys or non-string literal values).
///
/// # Examples
///
/// ```rust,ignore
/// use syn::Field;
///
/// // Parse a field with a `bitfield` attribute.
/// let field: Field = syn::parse_quote! {
///     #[bitfield(name = "Speed", has = "0x4")]
///     pub speed: bool
/// };
/// let attr = parse_bitfield_attr(&field).unwrap();
/// assert!(matches!(attr, Some(BitfieldAttr::Has { name, mask }) if name == "Speed" && mask == "0x4"));
///
/// // Parse a field with a `bitfield_map` attribute.
/// let field_map: Field = syn::parse_quote! {
///     #[bitfield_map(name = "Flags", decoder = "decode_flags")]
///     pub flags: u32
/// };
/// let attr_map = parse_bitfield_attr(&field_map).unwrap();
/// assert!(matches!(attr_map, Some(BitfieldAttr::Map { name, decoder }) if name == "Flags" && decoder == "decode_flags"));
/// ```
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

fn is_bitfield_common_attribute(attr: &Attribute) -> bool {
    let path = attr.path();
    path.is_ident("missing") || path.is_ident("fail_if_missing") || path.is_ident("default")
}

fn is_regular_field_attribute(attr: &Attribute) -> bool {
    let path = attr.path();
    path.is_ident("field_name")
        || path.is_ident("missing")
        || path.is_ident("fail_if_missing")
        || path.is_ident("default")
        || path.is_ident("calculated")
        || path.is_ident("skip")
}

/// Parse a single field attribute into an `AttributeValue`.
///
/// Accepts a `syn::Attribute` that uses one of the supported forms and returns
/// a structured `AttributeValue` or a `syn::Error` if the attribute is unknown
/// or has an invalid literal form. Supported attribute shapes:
/// - `#[field_name = "…"]`
/// - `#[missing = "…"]`
/// - `#[calculated = "…"]`
/// - `#[fail_if_missing]`
/// - `#[skip]`
/// - The `#[default = ...]` form is rejected with a specific error message.
///
/// # Examples
///
/// ```rust,ignore
/// use syn::Attribute;
/// // parse a name-value attribute into a syn::Attribute
/// let attr: Attribute = syn::parse_quote!(#[field_name = "Speed"]);
/// let parsed = parse_attribute(&attr).unwrap();
/// match parsed {
///     AttributeValue::FieldName(name) => assert_eq!(name, "Speed"),
///     _ => panic!("unexpected attribute value"),
/// }
/// ```
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

/// Extracts the inner `T` when the provided `ty` is syntactically `Option<T>`.
///
/// # Returns
///
/// `Some(T)` containing the inner type if `ty` is `Option<T>`, `None` otherwise.
///
/// # Examples
///
/// ```rust,ignore
/// use quote::ToTokens;
/// use syn::Type;
///
/// let t: Type = syn::parse_str("Option<u32>").unwrap();
/// let inner = extract_option_type(&t).expect("expected Option");
/// assert_eq!(format!("{}", inner.into_token_stream()), "u32");
///
/// let t2: Type = syn::parse_str("Vec<u32>").unwrap();
/// assert!(extract_option_type(&t2).is_none());
/// ```
fn extract_option_type(ty: &Type) -> Option<Type> {
    if let Type::Path(type_path) = ty {
        let last_segment = type_path.path.segments.last()?;
        if last_segment.ident == "Option"
            && let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments
            && let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
        {
            return Some(inner_type.clone());
        }
    }
    None
}

/// Build the code snippets for the layout validation phase and the extraction plan from field strategies.
///
/// This function converts an ordered slice of `FieldStrategy` values into two vectors of token streams:
/// - validation checks: statements that will be emitted into `validate_layout` to verify layout presence and telemetry type compatibility;
/// - extraction plan items: `FieldExtraction` entries that encode how `adapt` should read or compute each struct field at runtime.
///
/// # Returns
///
/// A tuple where the first element is a `Vec<proc_macro2::TokenStream>` containing validation-check code fragments and the second element is a `Vec<proc_macro2::TokenStream>` containing extraction-plan entries.
///
/// # Examples
///
/// ```rust,ignore
/// // Minimal example: no fields produces empty validation and extraction-plan vectors.
/// let (validation_checks, extraction_plan_items) = generate_validation_phase(&[]);
/// assert!(validation_checks.is_empty());
/// assert!(extraction_plan_items.is_empty());
/// ```
fn generate_validation_phase(
    strategies: &[FieldStrategy],
) -> (Vec<proc_macro2::TokenStream>, Vec<proc_macro2::TokenStream>) {
    let mut checks = Vec::new();
    let mut items = Vec::new();
    for (index, strategy) in strategies.iter().enumerate() {
        let id = format_ident!("field_id_{}", index);
        let (name, ty, required, kind) = match strategy {
            FieldStrategy::Critical {
                field_name,
                field_type,
                ..
            } => (field_name, field_type.clone(), true, 0),
            FieldStrategy::Optional {
                field_name,
                inner_type,
                ..
            } => (field_name, inner_type.clone(), false, 1),
            FieldStrategy::TypeDefault {
                field_name,
                field_type,
                ..
            }
            | FieldStrategy::WithDefault {
                field_name,
                field_type,
                ..
            } => (field_name, field_type.clone(), false, 2),
            FieldStrategy::BitfieldHas {
                field_name,
                target_is_option,
                fail_if_missing,
                ..
            }
            | FieldStrategy::BitfieldMap {
                field_name,
                target_is_option,
                fail_if_missing,
                ..
            } => (
                field_name,
                syn::parse_quote!(::iracing_sdk::BitField),
                *fail_if_missing,
                if *fail_if_missing {
                    0
                } else if *target_is_option {
                    1
                } else {
                    2
                },
            ),
            FieldStrategy::Calculated { expression_str, .. } => {
                let _ = expression_str;
                items.push(quote!(FieldExtraction::Calculated));
                continue;
            }
            FieldStrategy::Skipped { .. } => {
                items.push(quote!(FieldExtraction::Skipped));
                continue;
            }
        };
        checks.push(quote! { let #id = ::iracing_sdk::AdapterValidation::resolve::<#ty>(layout, #name, #required)?; });
        items.push(match kind {
            0 => quote!(FieldExtraction::Required(#id.expect("required field resolution"))),
            1 => quote!(FieldExtraction::Optional(#id)),
            _ => quote!(FieldExtraction::WithDefault(#id)),
        });
    }
    (checks, items)
}

/// Rewrites a calculated expression so bare telemetry identifiers are replaced with
/// calls that fetch or default their extracted values at runtime.
///
/// Identifiers in `expr` that match keys in `field_map` are replaced with
/// `validated_frame.fetch_or_default::<T>(slot)` expressions where `T` is
/// the associated `Type` from `field_map`.
///
/// # Examples
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use syn::{Expr, Type};
///
/// // Build a simple expression referencing telemetry fields `speed` and `rpm`.
/// let expr: Expr = syn::parse_str("speed * 2.0 + rpm as f32").unwrap();
///
/// // Map `speed` and `rpm` to dummy types to trigger rewriting.
/// let mut field_map: HashMap<String, (usize, Type)> = HashMap::new();
/// field_map.insert("speed".to_string(), (0, syn::parse_str::<Type>("f32").unwrap()));
/// field_map.insert("rpm".to_string(), (1, syn::parse_str::<Type>("i32").unwrap()));
///
/// let tokens = process_calculated_expression(&expr, &field_map).unwrap();
/// let s = tokens.to_string();
///
/// // The output should contain fetch_or_default-style calls for the mapped identifiers.
/// assert!(s.contains("fetch_or_default"));
/// ```
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
    /// Rewrites simple identifier paths that match known telemetry fields into
    /// `validated_frame.fetch_or_default::<Type>(slot)` calls; all other
    /// expressions are folded unchanged.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use syn::{parse_quote, Expr, LitStr};
    /// use std::collections::HashMap;
    ///
    /// // Minimal stand-in for the folder's field_map: name -> (index, type)
    /// let mut field_map: HashMap<String, (usize, syn::Type)> = HashMap::new();
    /// field_map.insert("speed".to_string(), (0, parse_quote!(i32)));
    ///
    /// // Expression referencing a telemetry field by bare identifier.
    /// let expr: Expr = parse_quote!(speed);
    ///
    /// // Manually perform the transformation the folder would do:
    /// let transformed: Expr = parse_quote! {
    ///     validated_frame.fetch_or_default::<i32>(0)
    /// };
    ///
    /// let rendered = quote::quote!(#transformed).to_string();
    /// assert!(rendered.contains("fetch_or_default"));
    /// assert!(rendered.contains("speed"));
    /// ```
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Path(expr_path)
                if expr_path.qself.is_none() && expr_path.path.segments.len() == 1 =>
            {
                if let Some(ident) = expr_path.path.get_ident() {
                    let ident_str = ident.to_string();
                    if let Some((index, ty)) = self.field_map.get(&ident_str) {
                        let index = *index;
                        let ty = ty.clone();
                        return syn::parse_quote! {
                            validated_frame.fetch_or_default::<#ty>(#index)
                        };
                    }
                }
                Expr::Path(expr_path)
            }
            other => syn::fold::fold_expr(self, other),
        }
    }
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_type_default_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let _ = field_name;
    quote!(#field_ident: validated_frame.fetch_or_default::<#field_type>(#index))
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_with_default_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    default_expr: &Expr,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let _ = field_name;
    quote!(#field_ident: validated_frame.decode::<#field_type>(#index).ok().flatten().unwrap_or_else(|| #default_expr))
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_optional_assignment(
    index: usize,
    field_ident: &syn::Ident,
    inner_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    let _ = field_name;
    quote!(#field_ident: validated_frame.decode::<#inner_type>(#index).ok().flatten())
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_critical_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_type: &syn::Type,
    field_name: &str,
) -> proc_macro2::TokenStream {
    quote!(#field_ident: validated_frame.decode::<#field_type>(#index).expect(concat!("Failed to decode critical field ", #field_name)).expect("missing required plan slot"))
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_bitfield_has_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_name: &str,
    target_is_option: bool,
    default_expr: &Option<Expr>,
    mask_expr: &Expr,
    fail_if_missing: bool,
) -> proc_macro2::TokenStream {
    let _ = field_name;
    let value = quote!(validated_frame.decode::<::iracing_sdk::BitField>(#index));
    if target_is_option {
        quote!(#field_ident: #value.ok().flatten().map(|bits| bits.has_flag(#mask_expr)))
    } else if fail_if_missing {
        quote!(#field_ident: #value.expect("critical bitfield decode").expect("critical bitfield slot").has_flag(#mask_expr))
    } else {
        let fallback = default_expr
            .as_ref()
            .map(|expr| quote!(#expr))
            .unwrap_or_else(|| quote!(false));
        quote!(#field_ident: #value.ok().flatten().map(|bits| bits.has_flag(#mask_expr)).unwrap_or_else(|| #fallback))
    }
}

/// Generates a positional field assignment against the retained validated layout.
fn generate_bitfield_map_assignment(
    index: usize,
    field_ident: &syn::Ident,
    field_name: &str,
    target_is_option: bool,
    default_expr: &Option<Expr>,
    decoder_expr: &Expr,
    fail_if_missing: bool,
) -> proc_macro2::TokenStream {
    let _ = field_name;
    let value = quote!(validated_frame.decode::<::iracing_sdk::BitField>(#index));
    if target_is_option {
        quote!(#field_ident: #value.ok().flatten().map(#decoder_expr))
    } else if fail_if_missing {
        quote!(#field_ident: (#decoder_expr)(#value.expect("critical bitfield decode").expect("critical bitfield slot")))
    } else {
        let fallback = default_expr
            .as_ref()
            .map(|expr| quote!(#expr))
            .unwrap_or_else(|| quote!(::core::default::Default::default()));
        quote!(#field_ident: #value.ok().flatten().map(#decoder_expr).unwrap_or_else(|| #fallback))
    }
}

/// Generates the runtime field-assignment token streams used by the generated `adapt` method.
///
/// Produces one token stream per struct field (in the same order as `strategies`) containing
/// the initializer for that field. Each assignment is built from the corresponding
/// `FieldStrategy`; `Calculated` strategies are rewritten using `telemetry_map`. Returns a
/// `syn::Error` if rewriting any calculated expression fails.
///
/// # Parameters
///
/// - `strategies`: ordered per-field extraction strategies describing how each struct field
///   should be populated at runtime.
/// - `telemetry_map`: mapping from telemetry variable name to `(field_index, field_type)` used
///   when rewriting calculated expressions.
///
/// # Returns
///
/// `Ok(Vec<proc_macro2::TokenStream>)` with one token stream per field initializer in struct order,
/// or `Err(syn::Error)` if generation fails (for example, while processing a calculated expression).
///
/// # Examples
///
/// ```rust,ignore
/// # use std::collections::HashMap;
/// # use syn::Type;
/// # use proc_macro2::TokenStream;
/// # fn _example() -> Result<(), syn::Error> {
/// let strategies: Vec<FieldStrategy> = Vec::new();
/// let telemetry_map: HashMap<String, (usize, Type)> = HashMap::new();
/// let assignments = generate_extraction_phase(&strategies, &telemetry_map)?;
/// assert!(assignments.is_empty());
/// # Ok(()) }
/// ```
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
                fail_if_missing,
                ..
            } => generate_bitfield_has_assignment(
                index,
                field_ident,
                field_name,
                *target_is_option,
                default_expr,
                mask_expr,
                *fail_if_missing,
            ),
            FieldStrategy::BitfieldMap {
                field_ident,
                field_name,
                target_is_option,
                default_expr,
                decoder_expr,
                fail_if_missing,
                ..
            } => generate_bitfield_map_assignment(
                index,
                field_ident,
                field_name,
                *target_is_option,
                default_expr,
                decoder_expr,
                *fail_if_missing,
            ),

            FieldStrategy::Calculated {
                field_ident,
                expression,
                ..
            } => {
                let rewritten = process_calculated_expression(expression, telemetry_map)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;
    use syn::parse_quote;

    fn parse_strategy_error(field: &Field) -> String {
        match parse_field_strategy(field) {
            Ok(_) => panic!("field strategy should fail"),
            Err(err) => err.to_string(),
        }
    }

    #[test]
    fn malformed_missing_attribute_is_reported_for_regular_fields() {
        let field: Field = parse_quote! {
            #[field_name = "Speed"]
            #[missing = 123]
            speed: f32
        };

        let error = parse_strategy_error(&field);

        assert!(error.contains("missing must be a string literal"));
    }

    #[test]
    fn malformed_missing_attribute_is_reported_for_bitfield_fields() {
        let field: Field = parse_quote! {
            #[bitfield(name = "SessionFlags", has = "0b1")]
            #[missing = 123]
            is_green: bool
        };

        let error = parse_strategy_error(&field);

        assert!(error.contains("missing must be a string literal"));
    }

    #[test]
    fn fail_if_missing_rejects_option_fields() {
        let field: Field = parse_quote! {
            #[field_name = "Speed"]
            #[fail_if_missing]
            speed: Option<f32>
        };

        let error = parse_strategy_error(&field);

        assert!(error.contains("#[fail_if_missing] cannot be used on Option<T> fields"));
    }

    #[test]
    fn parse_attribute_extracts_field_name() {
        let attr: Attribute = parse_quote!(#[field_name = "Speed"]);

        let parsed = parse_attribute(&attr).unwrap();

        assert!(matches!(parsed, AttributeValue::FieldName(name) if name == "Speed"));
    }

    #[test]
    fn parse_bitfield_attr_supports_has_and_map_forms() {
        let has_field: Field = parse_quote! {
            #[bitfield(name = "Flags", has = "0x4")]
            flag: bool
        };
        let map_field: Field = parse_quote! {
            #[bitfield_map(name = "Flags", decoder = "decode_flags")]
            decoded: u32
        };

        let has_attr = parse_bitfield_attr(&has_field).unwrap();
        let map_attr = parse_bitfield_attr(&map_field).unwrap();

        assert!(matches!(
            has_attr,
            Some(BitfieldAttr::Has { name, mask }) if name == "Flags" && mask == "0x4"
        ));
        assert!(matches!(
            map_attr,
            Some(BitfieldAttr::Map { name, decoder }) if name == "Flags" && decoder == "decode_flags"
        ));
    }

    #[test]
    fn extract_option_type_returns_inner_type() {
        let option_ty: Type = parse_quote!(Option<u32>);
        let vec_ty: Type = parse_quote!(Vec<u32>);

        let inner = extract_option_type(&option_ty).unwrap();

        assert_eq!(inner.into_token_stream().to_string(), "u32");
        assert!(extract_option_type(&vec_ty).is_none());
    }

    #[test]
    fn parse_field_strategy_returns_optional_variant() {
        let field: Field = parse_quote! {
            #[field_name = "RPM"]
            rpm: Option<u32>
        };

        let strategy = parse_field_strategy(&field).unwrap();

        assert!(matches!(
            strategy,
            FieldStrategy::Optional { field_name, .. } if field_name == "RPM"
        ));
    }

    #[test]
    fn generate_validation_phase_builds_expected_plan_kinds() {
        let strategies = vec![
            FieldStrategy::TypeDefault {
                field_name: "Speed".to_string(),
                field_ident: parse_quote!(speed),
                field_type: parse_quote!(f32),
            },
            FieldStrategy::Optional {
                field_name: "Gear".to_string(),
                field_ident: parse_quote!(gear),
                inner_type: parse_quote!(i32),
            },
            FieldStrategy::Calculated {
                field_ident: parse_quote!(derived),
                expression: parse_quote!(speed + 1.0),
                expression_str: "speed + 1.0".to_string(),
            },
        ];

        let (validation_checks, extraction_plan_items) = generate_validation_phase(&strategies);
        let checks = validation_checks
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let plan = extraction_plan_items
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(validation_checks.len(), 2);
        assert_eq!(extraction_plan_items.len(), 3);
        assert!(checks.contains("resolve"));
        assert!(checks.contains("\"Speed\""));
        assert!(checks.contains("\"Gear\""));
        assert!(plan.contains("FieldExtraction :: WithDefault"));
        assert!(plan.contains("FieldExtraction :: Optional"));
        assert!(plan.contains("FieldExtraction :: Calculated"));
    }

    #[test]
    fn process_calculated_expression_rewrites_known_identifiers() {
        let expr: Expr = parse_quote!(speed * 2.0 + rpm as f32);
        let mut field_map = HashMap::new();
        field_map.insert("speed".to_string(), (0, parse_quote!(f32)));
        field_map.insert("rpm".to_string(), (1, parse_quote!(i32)));

        let tokens = process_calculated_expression(&expr, &field_map).unwrap();
        let rendered = tokens.to_string();

        assert!(rendered.contains("fetch_or_default"));
        assert!(!rendered.contains("\"speed\""));
        assert!(!rendered.contains("\"rpm\""));
    }

    #[test]
    fn generate_optional_assignment_emits_optional_decode_path() {
        let tokens =
            generate_optional_assignment(0, &parse_quote!(speed), &parse_quote!(f32), "Speed");
        let rendered = tokens.to_string();

        assert!(rendered.contains("decode"));
        assert!(rendered.contains("flatten"));
        assert!(rendered.contains("decode"));
        assert!(!rendered.contains("\"Speed\""));
    }

    #[test]
    fn generate_bitfield_has_assignment_emits_has_flag_logic() {
        let default_expr = Some(parse_quote!(true));
        let mask_expr: Expr = parse_quote!(0x4u32);
        let tokens = generate_bitfield_has_assignment(
            0,
            &parse_quote!(is_green),
            "SessionFlags",
            false,
            &default_expr,
            &mask_expr,
            false,
        );
        let rendered = tokens.to_string();

        assert!(rendered.contains("has_flag"));
        assert!(rendered.contains("0x4u32"));
        assert!(!rendered.contains("\"SessionFlags\""));
    }

    #[test]
    fn generate_bitfield_map_assignment_emits_decoder_call() {
        let decoder_expr: Expr = parse_quote!(decode_flags);
        let tokens = generate_bitfield_map_assignment(
            0,
            &parse_quote!(flags),
            "SessionFlags",
            false,
            &None,
            &decoder_expr,
            false,
        );
        let rendered = tokens.to_string();

        assert!(rendered.contains("(decode_flags)"));
        assert!(rendered.contains("unwrap_or_else"));
        assert!(!rendered.contains("\"SessionFlags\""));
    }
}
