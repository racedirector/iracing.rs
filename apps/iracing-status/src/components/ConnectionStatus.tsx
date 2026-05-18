import { capitalize } from "lodash";
import { StatusLight } from "./StatusLight";
import { type ConnectionStatus } from "../contexts/IRacingConnectionStateContext";

type StatusIndicator = {
  label: string;
  status: ConnectionStatus;
};

interface PopoverProps {
  items: StatusIndicator[];
}

function Popover({ items }: PopoverProps) {
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
            <span className="status-value">{capitalize(item.status)}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

export interface ConnectionStatusProps extends PopoverProps {
  connectionStatus: ConnectionStatus;
}

export function ConnectionStatus({
  connectionStatus,
  items,
}: ConnectionStatusProps) {
  return (
    <div className="connection-summary">
      <button
        className="connection-trigger"
        type="button"
        aria-describedby="connection-popover"
        aria-label={`iRacing connection ${capitalize(connectionStatus)}`}
      >
        <StatusLight status={connectionStatus} />
        <span>iRacing connection</span>
        <span className="status-value">{capitalize(connectionStatus)}</span>
      </button>

      <Popover items={items} />
    </div>
  );
}
