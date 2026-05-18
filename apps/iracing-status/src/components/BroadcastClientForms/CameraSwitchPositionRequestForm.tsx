import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";

const uint32Max = 4_294_967_295;

const optionalUint32Schema = z
  .string()
  .trim()
  .refine(
    (value) => {
      if (!value) {
        return true;
      }

      return /^\d+$/.test(value);
    },
    { message: "Use digits only." },
  )
  .refine(
    (value) => {
      if (!value) {
        return true;
      }

      return Number(value) <= uint32Max;
    },
    { message: `Use a value up to ${uint32Max}.` },
  );

export const cameraSwitchPositionRequestSchema = z.object({
  camera: optionalUint32Schema,
  group: optionalUint32Schema,
  position: optionalUint32Schema,
});

type CameraSwitchPositionRequestFormInput = z.input<
  typeof cameraSwitchPositionRequestSchema
>;

type CameraSwitchPositionRequestFormOutput = z.output<
  typeof cameraSwitchPositionRequestSchema
>;

type CameraSwitchPositionRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: CameraSwitchPositionRequestFormOutput) => void;
  submitLabel?: string;
};

export function CameraSwitchPositionRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: CameraSwitchPositionRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    CameraSwitchPositionRequestFormInput,
    unknown,
    CameraSwitchPositionRequestFormOutput
  >({
    defaultValues: {
      camera: "",
      group: "",
      position: "",
    },
    mode: "onChange",
    resolver: zodResolver(cameraSwitchPositionRequestSchema),
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
          name="position"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.position?.message}
              inputMode="numeric"
              label="Position"
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
  CameraSwitchPositionRequestFormInput,
  CameraSwitchPositionRequestFormOutput,
  CameraSwitchPositionRequestFormProps,
};
