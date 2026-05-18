import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import {
  BroadcastSelectField,
  BroadcastTextField,
  type SelectOption,
} from "./BroadcastFormFields";
import { requiredFiniteNumberSchema } from "./BroadcastFormValidation";

const forceFeedbackCommandModeValues = [
  "unset",
  "FORCE_FEEDBACK_COMMAND_MODE_MAX_FORCE",
] as const;

const forceFeedbackCommandModeOptions: SelectOption[] =
  forceFeedbackCommandModeValues.map((value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }));

export const forceFeedbackCommandRequestSchema = z.object({
  mode: z.enum(forceFeedbackCommandModeValues),
  value: requiredFiniteNumberSchema,
});

type ForceFeedbackCommandRequestFormInput = z.input<
  typeof forceFeedbackCommandRequestSchema
>;

type ForceFeedbackCommandRequestFormOutput = z.output<
  typeof forceFeedbackCommandRequestSchema
>;

type ForceFeedbackCommandRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ForceFeedbackCommandRequestFormOutput) => void;
  submitLabel?: string;
};

export function ForceFeedbackCommandRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ForceFeedbackCommandRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ForceFeedbackCommandRequestFormInput,
    unknown,
    ForceFeedbackCommandRequestFormOutput
  >({
    defaultValues: { mode: "unset", value: "" },
    mode: "onChange",
    resolver: zodResolver(forceFeedbackCommandRequestSchema),
  });

  return (
    <form
      className="broadcast-request-form"
      onSubmit={handleSubmit(onSubmit)}
      noValidate
    >
      <div className="broadcast-form-grid">
        <Controller
          control={control}
          name="mode"
          render={({ field }) => (
            <BroadcastSelectField
              errorMessage={errors.mode?.message}
              label="Mode"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              options={forceFeedbackCommandModeOptions}
              placeholder="Select mode"
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="value"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.value?.message}
              inputMode="decimal"
              label="Value"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              value={field.value}
            />
          )}
        />
      </div>

      <button
        className="broadcast-submit-button"
        disabled={isSubmitting}
        type="submit"
      >
        {isSubmitting ? "Sending..." : submitLabel}
      </button>
    </form>
  );
}

export type {
  ForceFeedbackCommandRequestFormInput,
  ForceFeedbackCommandRequestFormOutput,
  ForceFeedbackCommandRequestFormProps,
};
