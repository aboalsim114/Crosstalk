import { useEffect, useState } from 'react';
import { FALLBACK_INFO, getAppInfo, type AppInfo } from './backend';

/** Placeholder shell; the cinematic interface lands in feat/cinematic-ui. */
export default function App() {
  const [info, setInfo] = useState<AppInfo>(FALLBACK_INFO);
  useEffect(() => {
    getAppInfo().then(setInfo);
  }, []);

  return (
    <main className="flex h-full flex-col items-center justify-center gap-4 bg-[radial-gradient(120%_90%_at_38%_38%,#141113_0%,#08080a_55%,#030304_100%)]">
      <div className="font-mono text-[10.5px] font-medium tracking-[.3em] text-ember">{info.name.toUpperCase()}</div>
      <h1 className="font-serif text-[76px] leading-[.98] tracking-[-.02em] text-text-hi">
        Chaque call,
        <br />
        <span className="italic text-ember-hi">en français.</span>
      </h1>
      <div className="font-mono text-[10px] tracking-[.18em] text-muted-3">
        v{info.version} · {info.target}
      </div>
    </main>
  );
}
