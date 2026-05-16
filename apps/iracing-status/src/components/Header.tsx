import type { IRacingConnectionState } from "../connection";
import { ConnectionStatus } from "./ConnectionStatus";

type HeaderProps = {
  connectionState: IRacingConnectionState;
};

export function Header({ connectionState }: HeaderProps) {
  return (
    <header className="app-nav">
      <h1 className="app-title">iRacing Status</h1>
      <ConnectionStatus connectionState={connectionState} />
    </header>
  );
}
