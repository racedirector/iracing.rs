import { formatStatus, type StatusIndicator } from "../connection";
import { StatusLight } from "./StatusLight";

type ConnectionStatusPopoverProps = {
  items: StatusIndicator[];
};

export function ConnectionStatusPopover({
  items,
}: ConnectionStatusPopoverProps) {
  return (
    <div
      className="connection-popover"
      id="connection-popover"
      role="status"
      aria-live="polite"
      aria-label="iRacing connection details"
    >
      {items.map((item) => (
        <div className="status-detail" key={item.label}>
          <StatusLight status={item.status} />
          <div className="status-copy">
            <span className="status-label">{item.label}</span>
            <span className="status-value">{formatStatus(item.status)}</span>
          </div>
        </div>
      ))}
    </div>
  );
}
