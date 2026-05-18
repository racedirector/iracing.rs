import { zodResolver } from "@hookform/resolvers/zod";
import { Controller, useForm } from "react-hook-form";
import { z } from "zod";
import {
  BroadcastSelectField,
  BroadcastTextField,
  type SelectOption,
} from "./BroadcastFormFields";
import { optionalUint32Schema } from "./BroadcastFormValidation";

const chatCommandModeValues = [
  "unset",
  "CHAT_COMMAND_MODE_MACRO",
  "CHAT_COMMAND_MODE_BEGIN_CHAT",
  "CHAT_COMMAND_MODE_REPLY",
  "CHAT_COMMAND_MODE_CANCEL",
] as const;

const chatCommandModeOptions: SelectOption[] = chatCommandModeValues.map(
  (value) => ({
    label: value === "unset" ? "Unset" : value,
    value,
  }),
);

export const chatCommandRequestSchema = z.object({
  macro: optionalUint32Schema,
  mode: z.enum(chatCommandModeValues),
});

type ChatCommandRequestFormInput = z.input<typeof chatCommandRequestSchema>;

type ChatCommandRequestFormOutput = z.output<typeof chatCommandRequestSchema>;

type ChatCommandRequestFormProps = {
  isSubmitting?: boolean;
  onSubmit: (values: ChatCommandRequestFormOutput) => void;
  submitLabel?: string;
};

export function ChatCommandRequestForm({
  isSubmitting = false,
  onSubmit,
  submitLabel = "Prepare request",
}: ChatCommandRequestFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
  } = useForm<
    ChatCommandRequestFormInput,
    unknown,
    ChatCommandRequestFormOutput
  >({
    defaultValues: { macro: "", mode: "unset" },
    mode: "onChange",
    resolver: zodResolver(chatCommandRequestSchema),
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
              options={chatCommandModeOptions}
              placeholder="Select mode"
              value={field.value}
            />
          )}
        />

        <Controller
          control={control}
          name="macro"
          render={({ field }) => (
            <BroadcastTextField
              errorMessage={errors.macro?.message}
              inputMode="numeric"
              label="Macro"
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
  ChatCommandRequestFormInput,
  ChatCommandRequestFormOutput,
  ChatCommandRequestFormProps,
};
