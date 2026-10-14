import { useEffect, useRef } from 'react';
import { HeadsetScene } from './HeadsetScene';

/** Full-bleed WebGL layer behind the UI. */
export function Stage() {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let scene: HeadsetScene | null = null;
    let cancelled = false;
    HeadsetScene.mount(ref.current!).then((s) => {
      if (cancelled) s?.dispose();
      else scene = s;
    });
    return () => {
      cancelled = true;
      scene?.dispose();
    };
  }, []);
  return <div ref={ref} className="absolute inset-0 z-0" aria-hidden />;
}
