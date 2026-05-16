import {
  formatStatus,
  getConnectionStatus,
  getLifecycleStatus,
  type IRacingConnectionState,
} from "../connection";
import { ConnectionStatusPopover } from "./ConnectionStatusPopover";
import { StatusLight } from "./StatusLight";

type ConnectionStatusProps = {
  connectionState: IRacingConnectionState;
};

export function ConnectionStatus({
  connectionState,
}: ConnectionStatusProps) {
  const aggregateStatus = getConnectionStatus(connectionState);
  const lifecycleStatus = getLifecycleStatus(connectionState);

  return (
    <div className="connection-summary">
      <button
        className="connection-trigger"
        type="button"
        aria-describedby="connection-popover"
        aria-label={`iRacing connection ${formatStatus(aggregateStatus)}`}
      >
        <StatusLight status={aggregateStatus} />
        <span>iRacing connection</span>
        <span className="status-value">{formatStatus(aggregateStatus)}</span>
      </button>

      <ConnectionStatusPopover items={lifecycleStatus} />
    </div>
  );
}
