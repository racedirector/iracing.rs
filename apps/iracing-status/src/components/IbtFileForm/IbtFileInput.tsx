import { ChangeEvent, useEffect, useRef } from "react";

interface IbtFileInputProps {
  id: string;
  selectedFile: File | null;
  errorMessage?: string;
  onFileSelect: (file: File | null) => void;
}

function formatFileSize(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KB", "MB", "GB", "TB"];
  let size = bytes / 1024;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 ? 0 : 1)} ${units[unitIndex]}`;
}

function IbtFileInput({
  id,
  selectedFile,
  errorMessage,
  onFileSelect,
}: IbtFileInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!selectedFile && inputRef.current) {
      inputRef.current.value = "";
    }
  }, [selectedFile]);

  function handleChooseFile() {
    inputRef.current?.click();
  }

  function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    onFileSelect(event.target.files?.[0] ?? null);
  }

  return (
    <div className="ibt-picker__field">
      <label className="ibt-picker__label" htmlFor={id}>
        Telemetry file
      </label>

      <input
        ref={inputRef}
        id={id}
        className="ibt-picker__input"
        type="file"
        accept=".ibt"
        onChange={handleFileChange}
        aria-invalid={errorMessage ? "true" : "false"}
        aria-describedby={`${id}-status`}
      />

      <button
        className="ibt-picker__button"
        type="button"
        onClick={handleChooseFile}
        data-testid="ibt-file-picker-button"
      >
        Choose .ibt file
      </button>

      <div id={`${id}-status`} aria-live="polite">
        {selectedFile ? (
          <dl className="ibt-picker__details">
            <div>
              <dt>File</dt>
              <dd>{selectedFile.name}</dd>
            </div>
            <div>
              <dt>Size</dt>
              <dd>{formatFileSize(selectedFile.size)}</dd>
            </div>
          </dl>
        ) : (
          <p className="ibt-picker__hint">No file selected</p>
        )}

        {errorMessage ? (
          <p className="ibt-picker__error" role="alert">
            {errorMessage}
          </p>
        ) : null}
      </div>
    </div>
  );
}

export { IbtFileInput };
export type { IbtFileInputProps };
