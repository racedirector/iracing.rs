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

/// Derive macro that implements `::iracing_sdk::adapters::FrameAdapter` for structs with named fields.
///
/// Generates an implementation that performs two phases:
/// 1. Connection-time schema validation producing an ordered extraction plan.
/// 2. Runtime adaptation that decodes packet bytes into the struct fields using the plan.
///
/// # Examples
///
/// ```
/// use iracing_sdk_derive::IRacingTelemetryFrame;
///
/// #[derive(IRacingTelemetryFrame)]
/// struct SimpleFrame {
///     #[field_name = "Speed"]
///     speed: f32,
///     #[field_name = "DriverName"]
///     name: Option<String>,
/// }
/// ```
///
/// The generated impl validates a provided `VariableSchema` and produces an `AdapterValidation`
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
/// `validate_schema` and `adapt` implementations tailored to the struct's fields and attributes.
///
/// The function returns a `syn::Error` on unsupported inputs (non-struct or non-named fields)
/// or on invalid/malformed per-field attributes.
///
/// # Examples
///
/// ```
/// # use syn::{parse_str, DeriveInput};
/// # fn _example() -> syn::Result<()> {
/// let input: DeriveInput = parse_str("struct S { a: i32 }")?;
/// // `generate_frame_adapter` produces the token stream implementing FrameAdapter for `S`.
/// let _tokens = iracing_sdk_derive::generate_frame_adapter(&input)?;
/// # Ok(()) }
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

/// Generates a tokenized validation expression that checks a telemetry variable's runtime type against `target_type` and yields an optional cloned value for the field.
///
/// The produced code calls `::iracing_sdk::adapters::telemetry_type_mismatch_details::<T>(probe_expr)?` and:
/// - returns `Some(clone_expr)` when the telemetry type matches `target_type`;
/// - yields `None` when a type mismatch is detected and `treat_mismatch_as_missing` is `true`;
/// - returns a `IRacingSDKError::Parse` error when a type mismatch is detected and `treat_mismatch_as_missing` is `false` (the error message includes `field_name_lit` and the mismatch `details`).
///
/// # Parameters
///
/// - `probe_expr`: token stream yielding an expression used to probe the telemetry variable (e.g., a lookup into the schema).
/// - `clone_expr`: token stream yielding an expression that produces the value to return inside `Some(...)` when the types match.
/// - `field_name_lit`: string literal of the telemetry field name used in error messages.
/// - `target_type`: the Rust type to validate against the telemetry variable.
/// - `treat_mismatch_as_missing`: when `true`, a type mismatch is treated as missing (returns `None`); when `false`, a type mismatch produces a `Parse` error.
///
/// # Examples
///
/// ```no_run
/// # use quote::quote;
/// # use syn::parse_str;
/// // produce code that probes a schema entry and clones it when compatible
/// let probe = quote! { schema.get_variable("Speed") };
/// let clone = quote! { var_info.clone() };
/// let target: syn::Type = parse_str("f32").unwrap();
/// let tokens = generate_type_validation_check(probe, clone, "Speed", &target, true);
/// // `tokens` now contains generated code that yields `Some(var_info.clone())` or `None` depending on type compatibility
/// ```
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
/// ```
/// use syn::{Field, Result};
/// use std::str::FromStr;
/// // parse a field with a telemetry name into a syn::Field and derive its strategy
/// let field: Field = syn::parse_str("#[field_name = \"RPM\"] pub rpm: Option<u32>").unwrap();
/// let strat = crate::parse_field_strategy(&field).unwrap();
/// assert!(matches!(strat, crate::FieldStrategy::Optional { .. }));
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
/// ```
/// use syn::Field;
///
/// // Parse a field with a `bitfield` attribute.
/// let field: Field = syn::parse_str(
///     "#[bitfield(name = \"Speed\", has = \"0x4\")] pub speed: bool"
/// ).unwrap();
/// let attr = crate::parse_bitfield_attr(&field).unwrap();
/// assert!(matches!(attr, Some(crate::BitfieldAttr::Has { name, mask }) if name == "Speed" && mask == "0x4"));
///
/// // Parse a field with a `bitfield_map` attribute.
/// let field_map: Field = syn::parse_str(
///     "#[bitfield_map(name = \"Flags\", decoder = \"decode_flags\")] pub flags: u32"
/// ).unwrap();
/// let attr_map = crate::parse_bitfield_attr(&field_map).unwrap();
/// assert!(matches!(attr_map, Some(crate::BitfieldAttr::Map { name, decoder }) if name == "Flags" && decoder == "decode_flags"));
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
/// ```
/// use syn::Attribute;
/// // parse a name-value attribute into a syn::Attribute
/// let attr: Attribute = syn::parse_str(r#"#[field_name = "Speed"]"#).unwrap();
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
/// ```
/// use syn::Type;
///
/// let t: Type = syn::parse_str("Option<u32>").unwrap();
/// let inner = crate::extract_option_type(&t).expect("expected Option");
/// assert_eq!(format!("{}", inner.into_token_stream()), "u32");
///
/// let t2: Type = syn::parse_str("Vec<u32>").unwrap();
/// assert!(crate::extract_option_type(&t2).is_none());
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

/// Build the code snippets for the schema validation phase and the extraction plan from field strategies.
///
/// This function converts an ordered slice of `FieldStrategy` values into two vectors of token streams:
/// - validation checks: statements that will be emitted into `validate_schema` to verify schema presence and telemetry type compatibility;
/// - extraction plan items: `FieldExtraction` entries that encode how `adapt` should read or compute each struct field at runtime.
///
/// # Returns
///
/// A tuple where the first element is a `Vec<proc_macro2::TokenStream>` containing validation-check code fragments and the second element is a `Vec<proc_macro2::TokenStream>` containing extraction-plan entries.
///
/// # Examples
///
/// ```
/// // Minimal example: no fields produces empty validation and extraction-plan vectors.
/// let (validation_checks, extraction_plan_items) = generate_validation_phase(&[]);
/// assert!(validation_checks.is_empty());
/// assert!(extraction_plan_items.is_empty());
/// ```
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

/// Rewrites a calculated expression so bare telemetry identifiers are replaced with
/// calls that fetch or default their extracted values at runtime.
///
/// Identifiers in `expr` that match keys in `field_map` are replaced with
/// `validation.fetch_or_default::<T>(packet, "Name")`-style expressions where `T` is
/// the associated `Type` from `field_map`.
///
/// # Examples
///
/// ```
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
    /// `validation.fetch_or_default::<Type>(packet, "FieldName")` calls; all other
    /// expressions are folded unchanged.
    ///
    /// # Examples
    ///
    /// ```
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
    ///     validation.fetch_or_default::<i32>(packet, LitStr::new("speed", proc_macro2::Span::call_site()))
    /// };
    ///
    /// assert_eq!(quote::quote!(#transformed).to_string(), "validation . fetch_or_default :: < i32 > ( packet , \"speed\" )");
    /// ```
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

/// Generate the struct-field assignment token stream for a telemetry-backed field that falls back to the Rust type's `Default` when the variable is absent or decoding fails.
///
/// The produced tokens initialize the named field by indexing the adapter validation's extraction plan at `index` and:
/// - If the plan contains a `FieldExtraction::WithDefault` with `Some(var_info)`, attempts to decode the bytes using `<FieldType as VarData>::from_bytes`. On successful decode the decoded value is used; on decode error a one-time `tracing::warn!` is emitted and `<FieldType as Default>::default()` is used.
/// - If `var_info` is `None` or the plan entry is missing/other variant, uses `<FieldType as Default>::default()`.
///
/// # Parameters
///
/// - `index`: zero-based position of the field's extraction plan entry within `validation.extraction_plan`.
/// - `field_ident`: identifier of the struct field to assign.
/// - `field_type`: Rust type of the field (used both for decoding and to obtain `Default`).
/// - `field_name`: telemetry variable name used for diagnostic messages in warnings.
///
/// # Examples
///
/// ```
/// use proc_macro2::Span;
/// use syn::Ident;
/// // Construct a token stream for a field named `speed: i32` mapped to telemetry variable "Speed"
/// let ts = generate_type_default_assignment(
///     0,
///     &Ident::new("speed", Span::call_site()),
///     &syn::parse_str::<syn::Type>("i32").unwrap(),
///     "Speed",
/// );
/// // `ts` can be interpolated into an impl body generated by the derive macro.
/// ```
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
                                    ::iracing_sdk::__private::tracing::warn!(
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

/// Generates the TokenStream for a struct field initializer that implements the "WithDefault"
/// extraction strategy.
///
/// The generated code will:
/// - Look up the field's extraction plan entry at `index`.
/// - If a `var_info` is present, attempt to decode the bytes into `field_type` via
///   `<field_type as ::iracing_sdk::VarData>::from_bytes(&data, var_info)`.
/// - On successful decode, return the decoded value.
/// - On decode error, emit a one-time `tracing::warn!` annotated with `field_name` and the
///   observed telemetry type, then evaluate and return `default_expr`.
/// - If `var_info` is absent or the plan entry is missing/unexpected, evaluate and return
///   `default_expr`.
///
/// Parameters:
/// - `index`: index of this field's extraction plan entry in `validation.extraction_plan`.
/// - `field_ident`: identifier of the struct field to initialize.
/// - `field_type`: Rust type of the field; used both in the generated type casts and in the
///   emitted warning message.
/// - `default_expr`: expression to evaluate when the telemetry value is missing or decoding
///   fails; inserted verbatim into the generated code.
/// - `field_name`: telemetry variable name used in warning metadata.
///
/// # Examples
///
/// ```no_run
/// use syn::{Expr, Ident, Type};
/// use proc_macro2::TokenStream;
///
/// // Example usage (construction of syn values omitted for brevity)
/// let idx = 0usize;
/// let ident: Ident = syn::parse_str("speed").unwrap();
/// let ty: Type = syn::parse_str("f32").unwrap();
/// let default_expr: Expr = syn::parse_str("0.0f32").unwrap();
/// let field_name = "Speed";
///
/// let ts: TokenStream = generate_with_default_assignment(idx, &ident, &ty, &default_expr, field_name);
/// // `ts` now contains the tokens for the field initializer implementing the WithDefault strategy.
/// ```
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
                                    ::iracing_sdk::__private::tracing::warn!(
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

/// Generates the struct-field initializer TokenStream for a field whose strategy is `Optional`.
///
/// The produced code reads the adapter's extraction plan at `index`, expects a
/// `FieldExtraction::Optional { var_info, .. }` entry, and:
/// - if `var_info` is `Some`, decodes bytes with `<inner_type as VarData>::from_bytes(&data, var_info)` and returns `Some(value)` on success;
/// - on decode error, emits a one-time `tracing::warn!` (including the field name, expected Rust type, actual telemetry type, and the error) and yields `None`;
/// - if `var_info` is `None` or the plan entry is missing/unexpected, yields `None`.
///
/// # Parameters
///
/// - `index`: zero-based index of this field in the adapter's extraction plan.
/// - `field_ident`: identifier of the struct field being generated.
/// - `inner_type`: the Rust type `T` inside the `Option<T>` target.
/// - `field_name`: telemetry variable name used in diagnostics.
///
/// # Returns
///
/// A `proc_macro2::TokenStream` containing the expression that initializes the struct field to an `Option<inner_type>` according to the behavior above.
///
/// # Examples
///
/// ```
/// # use syn::{Ident, Type};
/// # use proc_macro2::TokenStream;
/// // Construct inputs for a hypothetical field `speed: Option<f32>` mapped to telemetry "Speed"
/// let index = 0usize;
/// let field_ident: Ident = syn::parse_str("speed").unwrap();
/// let inner_type: Type = syn::parse_str("f32").unwrap();
/// let field_name = "Speed";
///
/// // Call the generator (assumes visibility in the same crate)
/// let tokens: TokenStream = crate::generate_optional_assignment(index, &field_ident, &inner_type, field_name);
///
/// // Generated tokens should reference the field identifier
/// let tokens_str = tokens.to_string();
/// assert!(tokens_str.contains("speed"));
/// ```
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
                                    ::iracing_sdk::__private::tracing::warn!(
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

/// Generate the struct field assignment tokens for a field classified as "Critical".
///
/// This produces code that reads the extraction plan at `index`, expects a
/// `FieldExtraction::Required { name, var_info }` entry, decodes the variable
/// with `<T as VarData>::from_bytes(&data, var_info)`, returns the decoded
/// value on success and panics on decode errors or if the plan entry is missing
/// or of an unexpected variant.
///
/// # Panics
///
/// Panics if the extraction plan entry at `index` is missing, is not
/// `FieldExtraction::Required`, or if decoding the variable fails.
///
/// # Returns
///
/// A `proc_macro2::TokenStream` containing the generated assignment expression
/// for the given field.
///
/// # Examples
///
/// ```
/// // Generated snippet (conceptual):
/// // my_field: {
/// //     match validation.extraction_plan.get(3) {
/// //         Some(::iracing_sdk::adapters::FieldExtraction::Required { name, var_info }) => {
/// //             match <MyType as ::iracing_sdk::VarData>::from_bytes(&data, var_info) {
/// //                 Ok(value) => value,
/// //                 Err(err) => panic!("Failed to decode critical field '{}' during adapt: {err:?}", name),
/// //             }
/// //         }
/// //         Some(other) => panic!("Validation plan entry for 'MyField' is {:?}, expected Required", other),
/// //         None => panic!("Validation plan missing required field 'MyField'"),
/// //     }
/// // }
/// ```
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

/// Generate the struct-field assignment tokens for a `BitfieldHas` extraction strategy.
///
/// This produces the TokenStream used to initialize a single struct field when the field is
/// extracted from a `::iracing_sdk::BitField` mask. For `target_is_option == true` the
/// generated code returns `Option<bool>` (using `None` on missing/mis-parse); otherwise it
/// returns `bool` and uses `default_expr` or `false` as the fallback when the bitfield is
/// absent or fails to decode.
///
/// # Examples
///
/// ```
/// use syn::parse_str;
/// use quote::ToTokens;
///
/// // Prepare inputs
/// let ident = parse_str::<syn::Ident>("flag_field").unwrap();
/// let mask_expr = parse_str::<syn::Expr>("0x04u32").unwrap();
/// let default_expr = Some(parse_str::<syn::Expr>("true").unwrap());
///
/// // Generate tokens for a non-option bool field with explicit fallback
/// let ts = iracing_sdk_derive::generate_bitfield_has_assignment(
///     0,
///     &ident,
///     "FLAG_NAME",
///     false,
///     &default_expr,
///     &mask_expr,
/// );
///
/// let s = ts.to_string();
/// // Generated code should attempt to call `has_flag` on the decoded BitField
/// assert!(s.contains("has_flag"));
/// ```
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
                                        ::iracing_sdk::__private::tracing::warn!(
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
                                        ::iracing_sdk::__private::tracing::warn!(
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

/// Generate the struct-field assignment tokens for a `BitfieldMap` extraction strategy.
///
/// The produced code reads the corresponding `validation.extraction_plan` entry at `index`,
/// decodes a `::iracing_sdk::BitField` from `data` when present, applies `decoder_expr` to the
/// decoded `BitField`, and returns either the decoded/mapped value, an explicit `default_expr` or
/// `Default::default()` fallback, or `None` for `Option` targets. For critical (`Required`)
/// plan entries a decode error will panic; for non-critical entries a one-time `tracing::warn!`
/// is emitted on decode failures and the fallback is used.
///
/// # Examples
///
/// ```no_run
/// use syn::{Ident, Expr};
/// // Construct a simple decoder expression and an ident for demonstration purposes.
/// let ident = Ident::new("mapped_field", proc_macro2::Span::call_site());
/// let decoder: Expr = syn::parse_str("|bits: ::iracing_sdk::BitField| bits.some_map()").unwrap();
/// let tokens = iracing_sdk_derive::generate_bitfield_map_assignment(
///     0,
///     &ident,
///     "SomeBitfield",
///     false,
///     &None,
///     &decoder,
/// );
/// // `tokens` contains the generated assignment for use in the derived `adapt` implementation.
/// ```
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
                                        ::iracing_sdk::__private::tracing::warn!(
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
                                        ::iracing_sdk::__private::tracing::warn!(
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
/// ```
/// # use std::collections::HashMap;
/// # use syn::Type;
/// # use proc_macro2::TokenStream;
/// # fn _example() -> Result<(), syn::Error> {
/// let strategies: Vec<crate::FieldStrategy> = Vec::new();
/// let telemetry_map: HashMap<String, (usize, Type)> = HashMap::new();
/// let assignments = crate::generate_extraction_phase(&strategies, &telemetry_map)?;
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
}
