use std::collections::BTreeMap;

use schemars::schema::{
    InstanceType, Metadata, ObjectValidation, RootSchema, Schema, SchemaObject, SingleOrVec,
};

use crate::{VariableInfo, VariableSchema, VariableType};

fn scalar_schema_for_variable_type(ty: &VariableType) -> Schema {
    let instance_type = match ty {
        VariableType::Char => InstanceType::String,
        VariableType::Bool => InstanceType::Boolean,
        VariableType::Float32 | VariableType::Float64 => InstanceType::Number,
        VariableType::Int8
        | VariableType::UInt8
        | VariableType::Int16
        | VariableType::UInt16
        | VariableType::Int32
        | VariableType::UInt32
        | VariableType::BitField => InstanceType::Integer,
    };

    Schema::Object(SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(instance_type))),
        ..Default::default()
    })
}

fn is_char_buffer(info: &VariableInfo) -> bool {
    info.data_type == VariableType::Char && info.count > 1
}

fn telemetry_property_schema(info: &VariableInfo) -> Schema {
    // If it's a char buffer, treat it as a string (not an array of single-char strings).
    if is_char_buffer(info) {
        let mut obj = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
            ..Default::default()
        };

        obj.metadata
            .get_or_insert_with(Default::default)
            .description = Some(info.description.clone());

        obj.extensions
            .insert("x-units".into(), info.units.clone().into());
        obj.extensions.insert(
            "x-iracing-var-type".into(),
            format!("{:?}", info.data_type).into(),
        );
        obj.extensions
            .insert("x-count".into(), (info.count as u64).into());
        obj.extensions
            .insert("x-offset".into(), (info.offset as u64).into());

        return Schema::Object(obj);
    }

    // Otherwise: scalar type, possibly array if count > 1.
    let base = scalar_schema_for_variable_type(&info.data_type);

    let mut obj = match base {
        Schema::Object(o) => o,
        _ => SchemaObject::default(),
    };

    obj.metadata
        .get_or_insert_with(Default::default)
        .description = Some(info.description.clone());

    obj.extensions
        .insert("x-units".into(), info.units.clone().into());
    obj.extensions.insert(
        "x-iracing-var-type".into(),
        format!("{:?}", info.data_type).into(),
    );
    obj.extensions
        .insert("x-count-as-time".into(), info.count_as_time.into());
    obj.extensions
        .insert("x-count".into(), (info.count as u64).into());
    obj.extensions
        .insert("x-offset".into(), (info.offset as u64).into());

    if info.count > 1 {
        Schema::Object(array_schema_for_count(obj, info.count))
    } else {
        Schema::Object(obj)
    }
}

fn array_schema_for_count(item_schema: SchemaObject, count: usize) -> SchemaObject {
    use schemars::schema::ArrayValidation;

    SchemaObject {
        instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Array))),
        array: Some(Box::new(ArrayValidation {
            items: Some(SingleOrVec::Single(Box::new(Schema::Object(item_schema)))),
            min_items: Some(count as u32),
            max_items: Some(count as u32),
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn schema_for_variable_schema(schema: &VariableSchema) -> RootSchema {
    let mut properties: BTreeMap<String, Schema> = BTreeMap::new();

    for (name, info) in &schema.variables {
        let prop_name = name.clone();
        properties.insert(prop_name, telemetry_property_schema(info));
    }

    RootSchema {
        schema: SchemaObject {
            metadata: Some(Box::new(Metadata {
                title: Some("Telemetry".to_string()),
                description: Some("Telemetry from iRacing".to_string()),
                ..Default::default()
            })),
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
            object: Some(Box::new(ObjectValidation {
                properties,
                additional_properties: Some(Box::new(Schema::Bool(false))),
                ..Default::default()
            })),
            ..Default::default()
        },
        definitions: Default::default(),
        ..Default::default()
    }
}

impl From<&VariableSchema> for RootSchema {
    fn from(v: &VariableSchema) -> Self {
        schema_for_variable_schema(v)
    }
}

impl From<VariableSchema> for RootSchema {
    fn from(v: VariableSchema) -> Self {
        RootSchema::from(&v)
    }
}
