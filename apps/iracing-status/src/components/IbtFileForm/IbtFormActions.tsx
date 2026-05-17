interface IbtFormActionsProps {
  canLoad: boolean;
  onClear: () => void;
}

function IbtFormActions({ canLoad, onClear }: IbtFormActionsProps) {
  return (
    <div className="ibt-picker__actions">
      <button
        className="ibt-picker__secondary-button"
        type="button"
        onClick={onClear}
        data-testid="ibt-clear-button"
      >
        Clear
      </button>
      <button
        className="ibt-picker__button"
        type="submit"
        disabled={!canLoad}
        data-testid="ibt-load-button"
      >
        Load
      </button>
    </div>
  );
}

export { IbtFormActions };
export type { IbtFormActionsProps };
