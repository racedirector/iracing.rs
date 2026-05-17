import { Controller, useForm } from "react-hook-form";
import { IbtFileInput } from "./IbtFileInput";
import { IbtFormActions } from "./IbtFormActions";

interface IbtFileFormValues {
  ibtFile: File | null;
}

interface IbtFileFormProps {
  onClear: () => void;
  onLoad: (file: File) => void;
}

function validateIbtFile(file: File | null) {
  if (!file) {
    return "Choose an iRacing telemetry file before loading.";
  }

  if (!file.name.toLowerCase().endsWith(".ibt")) {
    return "Choose an iRacing telemetry file with a .ibt extension.";
  }

  return true;
}

function IbtFileForm({ onClear, onLoad }: IbtFileFormProps) {
  const {
    control,
    formState: { errors },
    handleSubmit,
    reset,
    setValue,
    watch,
  } = useForm<IbtFileFormValues>({
    defaultValues: {
      ibtFile: null,
    },
    mode: "onChange",
  });

  const selectedFile = watch("ibtFile");
  const canLoad = Boolean(selectedFile && !errors.ibtFile);

  function handleClear() {
    reset();
    onClear();
  }

  function handleLoad(values: IbtFileFormValues) {
    if (values.ibtFile) {
      onLoad(values.ibtFile);
    }
  }

  return (
    <form
      className="ibt-picker"
      onSubmit={handleSubmit(handleLoad)}
      noValidate
    >
      <Controller
        control={control}
        name="ibtFile"
        rules={{ validate: validateIbtFile }}
        render={({ fieldState }) => (
          <IbtFileInput
            id="ibt-file"
            selectedFile={selectedFile}
            errorMessage={fieldState.error?.message}
            onFileSelect={(file) => {
              setValue("ibtFile", file, {
                shouldDirty: true,
                shouldTouch: true,
                shouldValidate: true,
              });
            }}
          />
        )}
      />

      <IbtFormActions canLoad={canLoad} onClear={handleClear} />
    </form>
  );
}

export { IbtFileForm };
export type { IbtFileFormProps, IbtFileFormValues };
