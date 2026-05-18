import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastSelectField, type SelectOption } from "./BroadcastFormFields";

const videoCaptureModeValues = [
  "unset",
  "VIDEO_CAPTURE_MODE_SCREENSHOT",
  "VIDEO_CAPTURE_MODE_START",
  "VIDEO_CAPTURE_MODE_STOP",
  "VIDEO_CAPTURE_MODE_TOGGLE",
  "VIDEO_CAPTURE_MODE_SHOW_TIMER",
  "VIDEO_CAPTURE_MODE_HIDE_TIMER",
] as const;

const videoCaptureModeOptions: SelectOption[] = videoCaptureModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const videoCaptureRequestSchema = z.object({
  mode: z.enum(videoCaptureModeValues),
});

type VideoCaptureRequestFormInput = z.input<typeof videoCaptureRequestSchema>;

type VideoCaptureRequestFormOutput = z.output<typeof videoCaptureRequestSchema>;

type VideoCaptureRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: VideoCaptureRequestFormOutput) => void;
  submitLabel?: string;
};

export function VideoCaptureRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: VideoCaptureRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    VideoCaptureRequestFormInput,
    unknown,
    VideoCaptureRequestFormOutput
  >({
    defaultValues: { mode: "unset" },
    mode: "onChange",
    resolver: zodResolver(videoCaptureRequestSchema),
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
              options={videoCaptureModeOptions}
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
  VideoCaptureRequestFormInput,
  VideoCaptureRequestFormOutput,
  VideoCaptureRequestFormProps,
};
