import { isTauri } from '../backend';
import { Seg, Slider, ToggleRow, fnt } from '../components/atoms';
import { channelName, langName, quote, t, useT, type MessageKey, targetName } from '../i18n';
import { SAMPLE_OVERLAY, type OverlayLine } from '../state/data';
import { useApp, type AppState, type Corner, type OverlayFormat } from '../state/store';
import { StreamCard } from './StreamCard';

const FORMATS: OverlayFormat[] = ['minimal', 'bilingue', 'accessible'];
export const CORNERS: [Corner, MessageKey][] = [
  ['tl', 'ov.corner.tl'],
  ['tr', 'ov.corner.tr'],
  ['bl', 'ov.corner.bl'],
  ['br', 'ov.corner.br'],
];

export interface OverlayBlock {
  key: string;
  color: string;
  head: string;
  orig: string;
  line: string;
  opacity: number;
}

/** Overlay subtitle blocks: the latest live lines, or samples before any exist. */
export function overlayBlocks(feed: OverlayLine[], maxBlocks: number): OverlayBlock[] {
  const src = feed.length ? feed : SAMPLE_OVERLAY;
  return src.slice(0, maxBlocks).map((f, i) => {
    const voice = f.kind === 'voice';
    const kind = voice ? t('feed.voice') : `${t('feed.chat')} • ${channelName(f.ch)}`;
    const to = `→ ${targetName()}`;
    const text = f.shown || f.orig;
    // A real transcript is the original text: never labelled as French.
    const transcript = f.status === 'transcrit';
    // Provisional French of a line still being spoken (or whose final translation is pending).
    if (transcript && f.shown) {
      return {
        key: `${i}-${f.orig}`,
        color: voice ? '#ffa24a' : '#7fd4ff',
        head: `${kind} • ${langName(f.src)} ${to} ${t('ov.provisional')}`,
        orig: f.orig,
        line: `${f.sender ? f.sender + ' : ' : ''}${quote(f.shown + (f.live ? ' …' : ''))}`,
        opacity: i === maxBlocks - 1 && maxBlocks > 2 ? 0.5 : 1,
      };
    }
    return {
      key: `${i}-${f.orig}`,
      color: voice ? '#ffa24a' : '#7fd4ff',
      head: transcript
        ? voice
          ? `${kind} • ${langName(f.src)} • ${t('ov.voiceUntranslated')}`
          : `${kind} • ${langName(f.src)} ${t('ov.chatUntranslated')}`
        : `${kind} • ${langName(f.src)} ${to}`,
      orig: transcript ? '' : f.orig,
      line: transcript
        ? `${f.sender ? f.sender + ' : ' : `[${f.src === 'xx' ? '??' : f.src.toUpperCase()}] `}${quote(text + (f.live ? ' …' : ''))}`
        : f.sender
          ? `${f.sender} : ${quote(text)}`
          : quote(text),
      opacity: i === maxBlocks - 1 && maxBlocks > 2 ? 0.5 : 1,
    };
  });
}

/** The subtitle stack itself; shared by the preview and the real overlay window. */
export function OverlayStack({ s, outline }: { s: Pick<AppState, 'feed' | 'maxBlocks' | 'fmt' | 'opacity' | 'textSize'>; outline?: string }) {
  useT(); // re-render when the language changes
  const blocks = overlayBlocks(s.feed, s.maxBlocks);
  const size = s.fmt === 'accessible' ? s.textSize + 6 : s.textSize;
  return (
    <div className="flex w-[380px] flex-col gap-2 rounded" style={{ outline: outline ?? '1px solid transparent', outlineOffset: 8 }}>
      {blocks.map((o) => (
        <div
          key={o.key}
          className="flex flex-col gap-[5px] rounded-[10px] px-[15px] py-[11px]"
          style={{ background: `rgba(6,6,8,${(s.opacity / 100).toFixed(2)})`, opacity: o.opacity, border: '1px solid rgba(255,255,255,.05)' }}
        >
          {s.fmt !== 'minimal' && <div style={{ font: fnt(600, 9.5, 'mono'), letterSpacing: '.14em', color: o.color }}>{o.head}</div>}
          {s.fmt === 'bilingue' && <div style={{ font: fnt(400, 11.5, 'mono', 1.4), color: '#a49d92', textShadow: '0 1px 2px #000' }}>{o.orig}</div>}
          <div style={{ font: fnt(500, size, 'sans', 1.32), color: '#fff', textShadow: '0 1px 2px #000,0 0 10px rgba(0,0,0,.85)' }}>{o.line}</div>
        </div>
      ))}
    </div>
  );
}

export function OverlayPage() {
  const t = useT();
  const s = useApp();
  const pad = 22;
  const c = s.corner;
  return (
    <div className="flex flex-col gap-5">
      <div className="grid items-start gap-5" style={{ gridTemplateColumns: 'minmax(0,1fr) 330px' }}>
        <div className="flex flex-col gap-3">
          <div
            className="relative aspect-video overflow-hidden rounded-[14px]"
            style={{ border: '1px solid rgba(255,255,255,.07)', background: 'radial-gradient(90% 80% at 60% 40%,#2a2620 0%,#121114 55%,#08080a 100%)' }}
          >
            <div className="absolute inset-0" style={{ background: 'repeating-linear-gradient(135deg,rgba(255,255,255,.018) 0 2px,transparent 2px 22px)' }} />
            <div className="absolute right-0 bottom-[22px] left-0 text-center" style={{ font: fnt(500, 10, 'mono'), letterSpacing: '.2em', color: '#4a4640' }}>
              {t('ov.game')}
            </div>
            <div className="absolute top-1/2 left-1/2 h-3.5 w-3.5 -translate-x-1/2 -translate-y-1/2">
              <div className="absolute top-0 left-1.5 h-3.5 w-0.5" style={{ background: 'rgba(127,224,168,.7)' }} />
              <div className="absolute top-1.5 left-0 h-0.5 w-3.5" style={{ background: 'rgba(127,224,168,.7)' }} />
            </div>
            {s.overlayOn && (
              <div
                data-testid="overlay-preview"
                className="absolute"
                style={{
                  top: c[0] === 't' ? pad : 'auto',
                  bottom: c[0] === 'b' ? pad : 'auto',
                  left: c[1] === 'l' ? pad : 'auto',
                  right: c[1] === 'r' ? pad : 'auto',
                  transition: 'all .6s cubic-bezier(.2,.7,.1,1)',
                }}
              >
                <OverlayStack s={s} outline={s.repos ? '1px dashed #ffa24a' : undefined} />
              </div>
            )}
          </div>
          <div className="flex justify-between" style={{ font: fnt(400, 11.5), color: '#6e6961' }}>
            <span>{s.feed.length ? t('ov.previewLive') : t('ov.previewSamples')}</span>
            <span>{t('ov.exclusive')}</span>
          </div>
        </div>

        <div className="glass flex flex-col gap-3.5 p-5">
          <div className="flex flex-col gap-[9px]">
            <div className="lbl">{t('ov.format')}</div>
            <Seg options={FORMATS.map((f) => [f, t(`ov.fmt.${f}`)] as [OverlayFormat, string])} value={s.fmt} onChange={(v) => s.set({ fmt: v })} grid={3} item={{ padding: '6px 0', textAlign: 'center', font: fnt(500, 11.5) }} />
          </div>
          <div className="flex items-center gap-4">
            <div className="grid h-[52px] w-[88px] flex-none grid-cols-2 gap-1 rounded-lg p-1" style={{ border: '1px solid rgba(255,255,255,.08)' }}>
              {CORNERS.map(([id, label]) => (
                <div
                  key={id}
                  role="button"
                  aria-label={t(label)}
                  aria-pressed={s.corner === id}
                  onClick={() => s.set({ corner: id })}
                  className="cursor-pointer rounded-[3px]"
                  style={{ background: s.corner === id ? '#ffa24a' : 'rgba(255,255,255,.07)', boxShadow: s.corner === id ? '0 0 12px rgba(255,162,74,.6)' : 'none', transition: 'all .3s' }}
                />
              ))}
            </div>
            <div className="flex flex-col gap-1">
              <div className="lbl">{t('ov.position')}</div>
              <div style={{ font: fnt(400, 20, 'serif'), color: '#ece6dc' }}>{t(CORNERS.find(([id]) => id === s.corner)![1])}</div>
            </div>
          </div>
          <Slider label={t('ov.opacity')} unit="%" min={0} max={100} value={s.opacity} onChange={(v) => s.set({ opacity: v })} />
          <Slider label={t('ov.textSize')} unit="px" min={14} max={28} value={s.textSize} onChange={(v) => s.set({ textSize: v })} />
          <Slider label={t('ov.fade')} unit="s" min={4} max={8} value={s.duration} onChange={(v) => s.set({ duration: v })} />
          <div className="flex items-center justify-between">
            <span style={{ font: fnt(400, 12.5), color: '#bdb6ab' }}>{t('ov.blocks')}</span>
            <Seg
              options={[
                [2, '2'],
                [3, '3'],
              ]}
              value={s.maxBlocks}
              onChange={(v) => s.set({ maxBlocks: v })}
              item={{ padding: '3px 12px', font: fnt(500, 11, 'mono') }}
            />
          </div>
          <div className="h-px" style={{ background: 'rgba(255,255,255,.06)' }} />
          <ToggleRow title={t('ov.click')} sub={t('ov.clickSub')} on={s.tg.click} onClick={() => s.toggle('click')} />
          <ToggleRow title={t('ov.shown')} sub="⌃⌥F8" subMono on={s.overlayOn} onClick={s.toggleOverlay} />
          <div
            role="button"
            onClick={() => s.set({ repos: !s.repos })}
            className="cursor-pointer rounded-full py-2.5 text-center"
            style={{ border: '1px solid rgba(255,255,255,.1)', font: fnt(500, 12.5), color: '#ece6dc', background: s.repos ? 'rgba(255,162,74,.1)' : 'transparent', transition: 'background .3s' }}
          >
            {s.repos ? t('ov.reposDone') : t('ov.repos')}
          </div>
        </div>
      </div>
      {isTauri && <StreamCard />}
    </div>
  );
}
