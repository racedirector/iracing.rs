import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";
import { requiredInt32Schema } from "./BroadcastFormValidation";

export const replaySetPlaySpeedRequestSchema = z.object({
  isSlowMotion: z.boolean(),
  speed: requiredInt32Schema,
});

type ReplaySetPlaySpeedRequestFormInput = z.input<
  typeof replaySetPlaySpeedRequestSchema
>;

type ReplaySetPlaySpeedRequestFormOutput = z.output<
  typeof replaySetPlaySpeedRequestSchema
>;

type ReplaySetPlaySpeedRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReplaySetPlaySpeedRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReplaySetPlaySpeedRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReplaySetPlaySpeedRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReplaySetPlaySpeedRequestFormInput,
    unknown,
    ReplaySetPlaySpeedRequestFormOutput
  >({
    defaultValues: { isSlowMotion: false, speed: "" },
    mode: "onChange",
    resolver: zodResolver(replaySetPlaySpeedRequestSchema),
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
          name="speed"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.speed?.message}
              inputMode="numeric"
              label="Speed"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="isSlowMotion"
          render={({ field }) => (
            <label className="broadcast-checkbox-field">
              <span>Slow Motion</span>
              <input
                checked={field.value}
                name={field.name}
                onBlur={field.onBlur}
                onChange={(event) => field.onChange(event.currentTarget.checked)}
                type="checkbox"
              />
            </label>
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
  ReplaySetPlaySpeedRequestFormInput,
  ReplaySetPlaySpeedRequestFormOutput,
  ReplaySetPlaySpeedRequestFormProps,
};
