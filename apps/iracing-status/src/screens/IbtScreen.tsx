import { useState } from "react";
import { IbtFileForm } from "../components/IbtFileForm";

export function IbtScreen() {
  const [loadedFileName, setLoadedFileName] = useState<string | null>(null);

  return (
    <section className="ibt-screen" aria-labelledby="ibt-screen-title">
      <div className="ibt-screen__content">
        <div className="ibt-screen__header">
          <h2 id="ibt-screen-title">IBT</h2>
          <p>Select an iRacing telemetry file from your computer.</p>
        </div>

        <IbtFileForm
          onClear={() => setLoadedFileName(null)}
          onLoad={(file) => setLoadedFileName(file.name)}
        />

        {loadedFileName ? (
          <p className="ibt-screen__status" role="status">
            Loaded {loadedFileName}
          </p>
        ) : null}
      </div>
    </section>
  );
}
