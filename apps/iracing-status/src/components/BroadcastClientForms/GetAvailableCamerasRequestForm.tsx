import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { z } from "zod";

export const getAvailableCamerasRequestSchema = z.object({});

type GetAvailableCamerasRequestFormInput = z.input<
  typeof getAvailableCamerasRequestSchema
>;

type GetAvailableCamerasRequestFormOutput = z.output<
  typeof getAvailableCamerasRequestSchema
>;

type GetAvailableCamerasRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: GetAvailableCamerasRequestFormOutput) => void;
  submitLabel?: string;
};

export function GetAvailableCamerasRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: GetAvailableCamerasRequestFormProps) {
  const { handleSubmit } = useForm<
    GetAvailableCamerasRequestFormInput,
    unknown,
    GetAvailableCamerasRequestFormOutput
  >({
    defaultValues: {},
    resolver: zodResolver(getAvailableCamerasRequestSchema),
  });

  return (
    <form
      className="broadcast-request-form"
      onSubmit={handleSubmit(onSubmit)}
      noValidate
    >
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
  GetAvailableCamerasRequestFormInput,
  GetAvailableCamerasRequestFormOutput,
  GetAvailableCamerasRequestFormProps,
};
