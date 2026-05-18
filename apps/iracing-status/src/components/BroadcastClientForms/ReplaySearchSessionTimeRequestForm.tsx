import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";
import { requiredUint32Schema } from "./BroadcastFormValidation";

export const replaySearchSessionTimeRequestSchema = z.object({
  sessionNumber: requiredUint32Schema,
  sessionTimeMs: requiredUint32Schema,
});

type ReplaySearchSessionTimeRequestFormInput = z.input<
  typeof replaySearchSessionTimeRequestSchema
>;

type ReplaySearchSessionTimeRequestFormOutput = z.output<
  typeof replaySearchSessionTimeRequestSchema
>;

type ReplaySearchSessionTimeRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReplaySearchSessionTimeRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReplaySearchSessionTimeRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReplaySearchSessionTimeRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReplaySearchSessionTimeRequestFormInput,
    unknown,
    ReplaySearchSessionTimeRequestFormOutput
  >({
    defaultValues: { sessionNumber: "", sessionTimeMs: "" },
    mode: "onChange",
    resolver: zodResolver(replaySearchSessionTimeRequestSchema),
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
          name="sessionNumber"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.sessionNumber?.message}
              inputMode="numeric"
              label="Session Number"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              pattern="[0-9]*"
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="sessionTimeMs"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.sessionTimeMs?.message}
              inputMode="numeric"
              label="Session Time Ms"
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
  ReplaySearchSessionTimeRequestFormInput,
  ReplaySearchSessionTimeRequestFormOutput,
  ReplaySearchSessionTimeRequestFormProps,
};
