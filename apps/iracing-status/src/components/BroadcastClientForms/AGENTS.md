# Broadcast Client Form Guidance

Use this guidance when adding or changing forms in this directory.

## Scope

- Forms collect user input for broadcast-client testing.
- Forms should not translate input into gRPC/protobuf request payloads.
- Any message-specific payload mapping belongs in a later integration layer, not in these presentation/data-collection components.

## Validation And Types

- Define a Zod schema in the form module for the data that the form collects.
- Use `zodResolver(schema)` with `react-hook-form`.
- Derive form types from the schema:
  - `z.input<typeof schema>` for `useForm` input/default values.
  - `z.output<typeof schema>` for the submitted form values.
- Do not create parallel hand-written `FormValues` or `Payload` types when they can be derived from the schema.
- The form `onSubmit` callback should receive the schema output directly.

## Field Modeling

- Text inputs for numeric protobuf fields may stay as strings in the form schema when the UI is only collecting user input.
- Validate numeric strings with Zod, but do not coerce them into numbers inside the form unless the form schema explicitly models them as numbers.
- Optional numeric fields can use empty strings to represent unfilled UI state.
- Keep request forms independent and small so they can be iterated individually.

## Enums

- Back enum dropdowns with a single source array and derive both Select options and the Zod enum from it.
- If a protobuf enum has an `UNKNOWN` case, do not include it in UI dropdowns or form validation. `UNKNOWN` exists for protocol fallback handling, not as a user-selectable option.
- Use an explicit UI-only placeholder value such as `unset` when a form needs to represent an unfilled optional enum.

## Output Contract

- Form submission should return an object shaped exactly like the Zod schema output.
- Do not rename the submitted object to `payload` in form code unless it truly is a transport-ready payload.
- Screens that preview submitted form data should treat it as collected `values`, not a protobuf request.
