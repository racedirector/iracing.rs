import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import { BroadcastTextField } from "./BroadcastFormFields";
import { optionalUint32Schema } from "./BroadcastFormValidation";

export const reloadTexturesRequestSchema = z.object({
  carIdx: optionalUint32Schema,
});

type ReloadTexturesRequestFormInput = z.input<
  typeof reloadTexturesRequestSchema
>;

type ReloadTexturesRequestFormOutput = z.output<
  typeof reloadTexturesRequestSchema
>;

type ReloadTexturesRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ReloadTexturesRequestFormOutput) => void;
  submitLabel?: string;
};

export function ReloadTexturesRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ReloadTexturesRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ReloadTexturesRequestFormInput,
    unknown,
    ReloadTexturesRequestFormOutput
  >({
    defaultValues: { carIdx: "" },
    mode: "onChange",
    resolver: zodResolver(reloadTexturesRequestSchema),
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
          name="carIdx"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.carIdx?.message}
              inputMode="numeric"
              label="Car Index"
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
  ReloadTexturesRequestFormInput,
  ReloadTexturesRequestFormOutput,
  ReloadTexturesRequestFormProps,
};
