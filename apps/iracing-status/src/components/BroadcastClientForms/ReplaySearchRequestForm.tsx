import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastSelectField, type SelectOption } from "./BroadcastFormFields";

const replaySearchModeValues = [
  "unset",
  "REPLAY_SEARCH_MODE_TO_START",
  "REPLAY_SEARCH_MODE_TO_END",
  "REPLAY_SEARCH_MODE_PREVIOUS_SESSION",
  "REPLAY_SEARCH_MODE_NEXT_SESSION",
  "REPLAY_SEARCH_MODE_PREVIOUS_LAP",
  "REPLAY_SEARCH_MODE_NEXT_LAP",
  "REPLAY_SEARCH_MODE_PREVIOUS_FRAME",
  "REPLAY_SEARCH_MODE_NEXT_FRAME",
  "REPLAY_SEARCH_MODE_PREVIOUS_INCIDENT",
  "REPLAY_SEARCH_MODE_NEXT_INCIDENT",
] as const;

const replaySearchModeOptions: SelectOption[] = replaySearchModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const replaySearchRequestSchema = z.object({
  mode: z.enum(replaySearchModeValues),
});

type ReplaySearchRequestFormInput = z.input<typeof replaySearchRequestSchema>;

type ReplaySearchRequestFormOutput = z.output<typeof replaySearchRequestSchema>;

type ReplaySearchRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReplaySearchRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReplaySearchRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReplaySearchRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReplaySearchRequestFormInput,
    unknown,
    ReplaySearchRequestFormOutput
  >({
    defaultValues: { mode: "unset" },
    mode: "onChange",
    resolver: zodResolver(replaySearchRequestSchema),
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
              options={replaySearchModeOptions}
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
  ReplaySearchRequestFormInput,
  ReplaySearchRequestFormOutput,
  ReplaySearchRequestFormProps,
};
