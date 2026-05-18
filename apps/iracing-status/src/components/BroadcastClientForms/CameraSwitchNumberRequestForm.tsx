import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";
import {
  requiredStringSchema,
  requiredUint32Schema,
} from "./BroadcastFormValidation";

export const cameraSwitchNumberRequestSchema = z.object({
  camera: requiredUint32Schema,
  carNumber: requiredStringSchema,
  group: requiredUint32Schema,
});

type CameraSwitchNumberRequestFormInput = z.input<
  typeof cameraSwitchNumberRequestSchema
>;

type CameraSwitchNumberRequestFormOutput = z.output<
  typeof cameraSwitchNumberRequestSchema
>;

type CameraSwitchNumberRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: CameraSwitchNumberRequestFormOutput) => void;
  submitLabel?: string;
};

export function CameraSwitchNumberRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: CameraSwitchNumberRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    CameraSwitchNumberRequestFormInput,
    unknown,
    CameraSwitchNumberRequestFormOutput
  >({
    defaultValues: {
      camera: "",
      carNumber: "",
      group: "",
    },
    mode: "onChange",
    resolver: zodResolver(cameraSwitchNumberRequestSchema),
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
          name="carNumber"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.carNumber?.message}
              inputMode="text"
              label="Car Number"
              name={field.name}
              onBlur={field.onBlur}
              onChange={field.onChange}
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="group"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.group?.message}
              inputMode="numeric"
              label="Group"
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
          name="camera"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.camera?.message}
              inputMode="numeric"
              label="Camera"
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
  CameraSwitchNumberRequestFormInput,
  CameraSwitchNumberRequestFormOutput,
  CameraSwitchNumberRequestFormProps,
};
