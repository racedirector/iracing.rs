import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import {
  BroadcastSelectField,
  BroadcastTextField,
  type SelectOption,
} from "./BroadcastFormFields";

const pitCommandModeValues = [
  "unset",
  "PIT_COMMAND_MODE_CLEAR",
  "PIT_COMMAND_MODE_TEAR_OFF",
  "PIT_COMMAND_MODE_FUEL",
  "PIT_COMMAND_MODE_LF_TIRE",
  "PIT_COMMAND_MODE_RF_TIRE",
  "PIT_COMMAND_MODE_LR_TIRE",
  "PIT_COMMAND_MODE_RR_TIRE",
  "PIT_COMMAND_MODE_CLEAR_TIRES",
  "PIT_COMMAND_MODE_FAST_REPAIR",
  "PIT_COMMAND_MODE_CLEAR_TEAR_OFF",
  "PIT_COMMAND_MODE_CLEAR_FAST_REPAIR",
  "PIT_COMMAND_MODE_CLEAR_FUEL",
] as const;

const pitCommandModeOptions: SelectOption[] = pitCommandModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const pitCommandRequestSchema = z.object({
  mode: z.enum(pitCommandModeValues),
  value: z
    .string()
    .trim()
    .refine(
      (value) => {
        if (!value) {
          return true;
        }

        return /^-?\d+(\.\d+)?$/.test(value);
      },
      { message: "Use a numeric value." },
    ),
});

type PitCommandRequestFormInput = z.input<typeof pitCommandRequestSchema>;

type PitCommandRequestFormOutput = z.output<typeof pitCommandRequestSchema>;

type PitCommandRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: PitCommandRequestFormOutput) => void;
  submitLabel?: string;
};

export function PitCommandRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: PitCommandRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    PitCommandRequestFormInput,
    unknown,
    PitCommandRequestFormOutput
  >({
    defaultValues: {
      mode: "unset",
      value: "",
    },
    mode: "onChange",
    resolver: zodResolver(pitCommandRequestSchema),
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
              options={pitCommandModeOptions}
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
  PitCommandRequestFormInput,
  PitCommandRequestFormOutput,
  PitCommandRequestFormProps,
};
