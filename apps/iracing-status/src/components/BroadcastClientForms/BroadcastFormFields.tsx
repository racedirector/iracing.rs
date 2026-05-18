import * as Select from "@radix-ui/react-select";

type SelectOption = {
  label: string;
  value: string;
};

type BroadcastTextFieldProps = {
  errorMessage?: string;
  inputMode: "decimal" | "numeric" | "text";
  label: string;
  name: string;
  onBlur: () => void;
  onChange: (value: string) => void;
  pattern?: string;
  value: string;
};

type BroadcastSelectFieldProps = {
  errorMessage?: string;
  label: string;
  name: string;
  onBlur: () => void;
  onChange: (value: string) => void;
  options: SelectOption[];
  placeholder: string;
  value: string;
};

export function BroadcastTextField({
  errorMessage,
  inputMode,
  label,
  name,
  onBlur,
  onChange,
  pattern,
  value,
}: BroadcastTextFieldProps) {
  return (
    <label className="broadcast-field" htmlFor={name}>
      <span>{label}</span>
      <input
        autoComplete="off"
        id={name}
        inputMode={inputMode}
        onBlur={onBlur}
        onChange={(event) => onChange(event.currentTarget.value)}
        pattern={pattern}
        type="text"
        value={value}
      />
      {errorMessage ? (
        <small className="broadcast-field__error">{errorMessage}</small>
      ) : null}
    </label>
  );
}

export function BroadcastSelectField({
  errorMessage,
  label,
  name,
  onBlur,
  onChange,
  options,
  placeholder,
  value,
}: BroadcastSelectFieldProps) {
  return (
    <div className="broadcast-field">
      <label htmlFor={name}>{label}</label>
      <Select.Root value={value} onValueChange={onChange}>
        <Select.Trigger
          className="broadcast-select-trigger"
          id={name}
          onBlur={onBlur}
        >
          <Select.Value placeholder={placeholder} />
          <Select.Icon className="broadcast-select-trigger__icon">v</Select.Icon>
        </Select.Trigger>

        <Select.Portal>
          <Select.Content className="broadcast-select-content" position="popper">
            <Select.Viewport className="broadcast-select-viewport">
              {options.map((option) => (
                <Select.Item
                  className="broadcast-select-item"
                  key={option.value}
                  value={option.value}
                >
                  <Select.ItemText>{option.label}</Select.ItemText>
                </Select.Item>
              ))}
            </Select.Viewport>
          </Select.Content>
        </Select.Portal>
      </Select.Root>
      {errorMessage ? (
        <small className="broadcast-field__error">{errorMessage}</small>
      ) : null}
    </div>
  );
}

export type { SelectOption };
