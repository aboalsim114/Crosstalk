import { useEffect, useState } from 'react';
import { streamOverlay, type StreamStatus } from '../../shared/ipc';
import { Slider, ToggleRow, fnt } from '../../shared/ui/atoms';
import { t, useT } from '../../shared/i18n';
import { copyText } from '../reply/ReplyBox';
import { TEST_LINE } from './stream';
import { useApp } from '../../app/state/store';

/** What the card says about the OBS page. */
export function streamSummary(st: StreamStatus | null, enabled: boolean): { value: string; detail: string; color: string } {
  if (!enabled) return { value: t('stream.off'), detail: t('stream.offDetail'), color: '#6e6961' };
  if (!st) return { value: t('stream.starting'), detail: '', color: '#6e6961' };
  if (st.error) return { value: t('stream.unavailable'), detail: st.error, color: '#ff8a7a' };
  if (!st.listening) return { value: t('stream.starting'), detail: '', color: '#ffa24a' };
  if (!st.clients) return { value: t('stream.waitingObs'), detail: t('stream.addSource'), color: '#ffa24a' };
  return {
    value: st.clients > 1 ? t('stream.sources', { count: st.clients }) : t('stream.connected'),
    detail: t('stream.sent', { count: st.sent }),
    color: '#7fe0a8',
  };
}

/** Mode streamer: translated subtitles as an OBS browser source. */
export function StreamCard() {
  const opts = useApp((s) => s.stream);
  const { setStream, flash } = useApp.getState();
  const t = useT();
  const [st, setSt] = useState<StreamStatus | null>(null);
  useEffect(() => {
    const load = () => streamOverlay.status().then(setSt);
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, []);
  const sum = streamSummary(st, opts.enabled);
  const url = st?.url ?? 'http://127.0.0.1:31983/';
  return (
    <div className="glass flex flex-col gap-4 p-5" data-testid="stream-card">
      <div className="flex items-baseline justify-between">
        <span className="lbl">{t('stream.title')}</span>
        <span style={{ font: fnt(400, 10, 'mono'), color: '#5a5650' }}>local · 127.0.0.1</span>
      </div>
      <div className="grid gap-5" style={{ gridTemplateColumns: 'minmax(0,1fr) minmax(0,1fr)' }}>
        <div className="flex flex-col gap-3">
          <ToggleRow title={t('stream.enable')} sub={t('stream.enableSub')} on={opts.enabled} onClick={() => setStream({ enabled: !opts.enabled })} />
          <div className="flex items-start gap-2.5">
            <div className="mt-[7px] h-1.5 w-1.5 flex-none rounded-[3px]" style={{ background: sum.color, boxShadow: `0 0 8px ${sum.color}` }} />
            <div className="flex min-w-0 flex-col gap-0.5">
              <span data-testid="stream-state" style={{ font: fnt(500, 13), color: '#ece6dc' }}>
                {sum.value}
              </span>
              <span style={{ font: fnt(400, 11, 'sans', 1.45), color: '#6e6961' }}>{sum.detail}</span>
            </div>
          </div>
          <div className="flex gap-2">
            <input className="text-input min-w-0 flex-1" readOnly value={url} aria-label={t('stream.urlLabel')} style={{ fontFamily: 'var(--font-mono)' }} />
            <div className="pill px-3.5 py-2" role="button" onClick={() => copyText(url).then((ok) => flash(ok ? t('stream.copied') : t('stream.copyFailed')))} style={{ font: fnt(500, 12) }}>
              {t('stream.copy')}
            </div>
          </div>
          <span style={{ font: fnt(400, 11, 'sans', 1.55), color: '#6e6961' }}>
            {t('stream.help')}
          </span>
          <div className="pill self-start px-3.5 py-2" role="button" aria-disabled={!opts.enabled} onClick={() => opts.enabled && streamOverlay.push(TEST_LINE)} style={{ font: fnt(500, 12) }}>
            {t('stream.test')}
          </div>
        </div>
        <div className="flex flex-col gap-3">
          <ToggleRow title={t('stream.names')} sub={t('stream.namesSub')} on={opts.names} onClick={() => setStream({ names: !opts.names })} />
          <ToggleRow title={t('stream.original')} sub={t('stream.originalSub')} on={opts.original} onClick={() => setStream({ original: !opts.original })} />
          <ToggleRow title={t('stream.mark')} sub={t('stream.markSub')} on={opts.watermark} onClick={() => setStream({ watermark: !opts.watermark })} />
          <Slider label={t('stream.size')} unit="px" min={18} max={72} value={opts.size} onChange={(v) => setStream({ size: v })} />
          <Slider label={t('stream.duration')} unit="s" min={3} max={20} value={opts.duration} onChange={(v) => setStream({ duration: v })} />
          <Slider label={t('stream.lines')} unit="" min={1} max={5} value={opts.max} onChange={(v) => setStream({ max: v })} />
        </div>
      </div>
    </div>
  );
}
