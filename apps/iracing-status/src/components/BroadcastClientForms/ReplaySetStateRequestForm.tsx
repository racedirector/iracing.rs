import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastSelectField, type SelectOption } from "./BroadcastFormFields";

const replayStateModeValues = ["unset", "REPLAY_STATE_MODE_ERASE_TAPE"] as const;

const replayStateModeOptions: SelectOption[] = replayStateModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const replaySetStateRequestSchema = z.object({
  state: z.enum(replayStateModeValues),
});

type ReplaySetStateRequestFormInput = z.input<
  typeof replaySetStateRequestSchema
>;

type ReplaySetStateRequestFormOutput = z.output<
  typeof replaySetStateRequestSchema
>;

type ReplaySetStateRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReplaySetStateRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReplaySetStateRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReplaySetStateRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReplaySetStateRequestFormInput,
    unknown,
    ReplaySetStateRequestFormOutput
  >({
    defaultValues: { state: "unset" },
    mode: "onChange",
    resolver: zodResolver(replaySetStateRequestSchema),
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
          name="state"
          render={({ field }) => (
            <BroadcastSelectField
              errorMessage={errors.state?.message}
              label="State"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              options={replayStateModeOptions}
              placeholder="Select state"
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
  ReplaySetStateRequestFormInput,
  ReplaySetStateRequestFormOutput,
  ReplaySetStateRequestFormProps,
};
