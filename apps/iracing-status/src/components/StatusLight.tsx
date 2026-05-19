import type { ConnectionStatus } from "../contexts/IRacingConnectionStateContext";

type StatusLightProps = {
  status: ConnectionStatus;
};

export function StatusLight({ status }: StatusLightProps) {
  return <span className={`status-light status-light--${status}`} />;
}
