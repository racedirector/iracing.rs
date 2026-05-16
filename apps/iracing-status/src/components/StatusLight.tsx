import type { ConnectionStatus } from "../connection";

type StatusLightProps = {
  status: ConnectionStatus;
};

export function StatusLight({ status }: StatusLightProps) {
  return <span className={`status-light status-light--${status}`} />;
}
