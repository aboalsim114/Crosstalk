import { useT } from '../../shared/i18n';
import { fnt } from '../../shared/ui/atoms';
import { useApp } from '../../app/state/store';
import { detectLocal, localServer, serviceChange } from './llm';

export type EngineChoice = 'cloud' | 'ollama' | 'lmstudio' | 'later';

/** Asked once, after the languages, unless an engine is already connected (settings of an earlier version). */
export function engineSetupShown(s: { langSetupDone: boolean; engineSetupDone: boolean; llm: { enabled: boolean } }): boolean {
  return s.langSetupDone && !s.engineSetupDone && !s.llm.enabled;
}

/**
 * Applies a first-launch choice. Nothing is connected or downloaded here: the user finishes in
 * Audio & IA › Traduction IA (key and consent for the cloud, model of the local server).
 */
export async function chooseEngine(choice: EngineChoice): Promise<void> {
  const app = useApp.getState();
  useApp.setState({ engineSetupDone: true });
  if (choice === 'later') return;
  if (choice === 'cloud') {
    app.setLlm({ ...serviceChange('openai', app.llm), enabled: false });
  } else {
    app.setLlm({ ...serviceChange(choice, app.llm), enabled: false });
    const local = await detectLocal();
    const models = localServer(local, choice)?.models ?? [];
    const cur = useApp.getState().llm;
    if (cur.service === choice && models.length && !models.includes(cur.model)) app.setLlm({ model: models[0] });
  }
  app.setPage('audio');
}

/** First launch: which translation engine (or later). The app works without one. */
export function EngineSetup() {
  const t = useT();
  const shown = useApp(engineSetupShown);
  if (!shown) return null;
  const choices: [EngineChoice, string, string][] = [
    ['cloud', t('setupEngine.cloud'), t('setupEngine.cloudSub')],
    ['ollama', t('setupEngine.ollama'), t('setupEngine.ollamaSub')],
    ['lmstudio', t('setupEngine.lmstudio'), t('setupEngine.lmstudioSub')],
    ['later', t('setupEngine.later'), t('setupEngine.laterSub')],
  ];
  return (
    <div className="absolute inset-0 z-[50] flex items-center justify-center" style={{ background: 'rgba(8,7,6,.78)', backdropFilter: 'blur(6px)' }} role="dialog" aria-modal="true" aria-label={t('setupEngine.title')}>
      <div className="glass flex w-[560px] max-w-[92%] flex-col gap-5 p-7" style={{ background: 'rgba(18,16,14,.94)' }}>
        <div className="flex flex-col gap-2">
          <span style={{ font: fnt(400, 30, 'serif'), color: '#ece6dc' }}>{t('setupEngine.title')}</span>
          <span style={{ font: fnt(400, 12.5, 'sans', 1.55), color: '#8f8a83' }}>{t('setupEngine.lead')}</span>
        </div>
        <div className="flex flex-col gap-2">
          {choices.map(([id, title, sub]) => (
            <div
              key={id}
              role="button"
              tabIndex={0}
              data-testid={`engine-${id}`}
              onClick={() => void chooseEngine(id)}
              onKeyDown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), void chooseEngine(id))}
              className="cursor-pointer rounded-xl px-4 py-3"
              style={{ border: `1px solid ${id === 'later' ? 'rgba(255,255,255,.06)' : 'rgba(255,162,74,.28)'}`, background: id === 'later' ? 'transparent' : 'rgba(255,162,74,.05)' }}
            >
              <div style={{ font: fnt(500, 13), color: id === 'later' ? '#bdb6ab' : '#ffc68a' }}>{title}</div>
              <div style={{ font: fnt(400, 11.5, 'sans', 1.5), color: '#8f8a83' }}>{sub}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
