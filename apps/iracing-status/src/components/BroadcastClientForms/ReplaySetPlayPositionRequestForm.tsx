import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import {
  BroadcastSelectField,
  BroadcastTextField,
  type SelectOption,
} from "./BroadcastFormFields";
import { requiredUint32Schema } from "./BroadcastFormValidation";

const replayPositionModeValues = [
  "unset",
  "REPLAY_POSITION_MODE_BEGIN",
  "REPLAY_POSITION_MODE_CURRENT",
  "REPLAY_POSITION_MODE_END",
] as const;

const replayPositionModeOptions: SelectOption[] = replayPositionModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const replaySetPlayPositionRequestSchema = z.object({
  frame: requiredUint32Schema,
  mode: z.enum(replayPositionModeValues),
});

type ReplaySetPlayPositionRequestFormInput = z.input<
  typeof replaySetPlayPositionRequestSchema
>;

type ReplaySetPlayPositionRequestFormOutput = z.output<
  typeof replaySetPlayPositionRequestSchema
>;

type ReplaySetPlayPositionRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReplaySetPlayPositionRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReplaySetPlayPositionRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReplaySetPlayPositionRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReplaySetPlayPositionRequestFormInput,
    unknown,
    ReplaySetPlayPositionRequestFormOutput
  >({
    defaultValues: { frame: "", mode: "unset" },
    mode: "onChange",
    resolver: zodResolver(replaySetPlayPositionRequestSchema),
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
              options={replayPositionModeOptions}
              placeholder="Select mode"
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="frame"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.frame?.message}
              inputMode="numeric"
              label="Frame"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              pattern="[0-9]*"
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
  ReplaySetPlayPositionRequestFormInput,
  ReplaySetPlayPositionRequestFormOutput,
  ReplaySetPlayPositionRequestFormProps,
};
