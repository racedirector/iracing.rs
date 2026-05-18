import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastSelectField, type SelectOption } from "./BroadcastFormFields";

const telemetryCommandModeValues = [
  "unset",
  "TELEMETRY_COMMAND_MODE_STOP",
  "TELEMETRY_COMMAND_MODE_START",
  "TELEMETRY_COMMAND_MODE_RESTART",
] as const;

const telemetryCommandModeOptions: SelectOption[] =
  telemetryCommandModeValues.map((value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }));

export const telemetryCommandRequestSchema = z.object({
  mode: z.enum(telemetryCommandModeValues),
});

type TelemetryCommandRequestFormInput = z.input<
  typeof telemetryCommandRequestSchema
>;

type TelemetryCommandRequestFormOutput = z.output<
  typeof telemetryCommandRequestSchema
>;

type TelemetryCommandRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: TelemetryCommandRequestFormOutput) => void;
  submitLabel?: string;
};

export function TelemetryCommandRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: TelemetryCommandRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    TelemetryCommandRequestFormInput,
    unknown,
    TelemetryCommandRequestFormOutput
  >({
    defaultValues: { mode: "unset" },
    mode: "onChange",
    resolver: zodResolver(telemetryCommandRequestSchema),
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
              options={telemetryCommandModeOptions}
              placeholder="Select mode"
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
  TelemetryCommandRequestFormInput,
  TelemetryCommandRequestFormOutput,
  TelemetryCommandRequestFormProps,
};
