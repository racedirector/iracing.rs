import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";
import { requiredUint32Schema } from "./BroadcastFormValidation";

export const cameraSetStateRequestSchema = z.object({
  state: requiredUint32Schema,
});

type CameraSetStateRequestFormInput = z.input<
  typeof cameraSetStateRequestSchema
>;

type CameraSetStateRequestFormOutput = z.output<
  typeof cameraSetStateRequestSchema
>;

type CameraSetStateRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: CameraSetStateRequestFormOutput) => void;
  submitLabel?: string;
};

export function CameraSetStateRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: CameraSetStateRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    CameraSetStateRequestFormInput,
    unknown,
    CameraSetStateRequestFormOutput
  >({
    defaultValues: { state: "" },
    mode: "onChange",
    resolver: zodResolver(cameraSetStateRequestSchema),
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
            <BroadcastTextField
              errorMessage={errors.state?.message}
              inputMode="numeric"
              label="State"
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
  CameraSetStateRequestFormInput,
  CameraSetStateRequestFormOutput,
  CameraSetStateRequestFormProps,
};
