import { beforeEach, describe, expect, it } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useApp } from '../../app/state/store';
import { restore, snapshot } from '../../app/state/persist';
import { useGsi } from '../recap/gsi';
import { overlayPayload } from '../../app/native';
import { visibleRadar } from '../../windows/overlay/OverlayWindow';
import { RadarCard } from './RadarCard';
import { RADAR_DEFAULTS, radarPayload } from './radar';

const app0 = useApp.getState();
const gsi0 = useGsi.getState();
beforeEach(() => {
  useApp.setState(app0, true);
  useGsi.setState(gsi0, true);
});

const playing = (map: string | null) => useGsi.setState({ status: { map } as never });
const line = (id: number, orig: string, shown: string | null = null) => ({
  id,
  kind: 'voice' as const,
  orig,
  shown,
});

describe('callout radar settings', () => {
  it('is on by default, saved with the settings and bounded on restore', () => {
    expect(app0.radar).toEqual(RADAR_DEFAULTS);
    expect(RADAR_DEFAULTS.on).toBe(true);
    const snap = snapshot({ ...app0, radar: { on: false, hold: 15 } }, null);
    expect(restore(JSON.parse(JSON.stringify(snap.config)), app0).app.radar).toEqual({ on: false, hold: 15 });
    const bad = restore({ version: 1, settings: { radar: { on: 'yes', hold: 999 } } }, app0).app.radar;
    expect(bad).toEqual(RADAR_DEFAULTS);
    // The sample asked by "Essayer" is not a setting.
    expect(JSON.stringify(snap.config)).not.toContain('radarDemo');
  });
});

describe('radar sent to the overlay', () => {
  it('lights the zones of the map being played, nothing on an unknown map or when off', () => {
    const now = Date.now();
    const feed = [line(now - 1000, 'двое на банане', 'Deux sur Banana')];
    expect(radarPayload(RADAR_DEFAULTS, 'de_inferno', feed, null, now)?.pings).toMatchObject([{ zone: 'banana', count: 2 }]);
    expect(radarPayload(RADAR_DEFAULTS, 'cs_office', feed, null, now)).toBeNull();
    expect(radarPayload(RADAR_DEFAULTS, null, feed, null, now)).toBeNull();
    expect(radarPayload({ ...RADAR_DEFAULTS, on: false }, 'de_inferno', feed, null, now)).toBeNull();
    // "Essayer": the sample on Mirage, even without a map.
    expect(radarPayload(RADAR_DEFAULTS, null, [], now - 500, now)?.map).toBe('de_mirage');
    expect(radarPayload(RADAR_DEFAULTS, null, [], now - 60_000, now)).toBeNull();
  });

  it('goes through the overlay payload and hides once every zone has faded', () => {
    const now = Date.now();
    playing('de_mirage');
    useApp.setState({
      feed: [
        {
          ...line(now - 800, 'трое на апах'),
          time: '',
          status: 'traduit',
          shown: null,
          lat: null,
          err: false,
          src: 'ru',
        },
      ] as never,
    });
    const r = overlayPayload(useApp.getState()).radar;
    expect(r?.pings).toMatchObject([{ zone: 'apps', count: 3 }]);
    expect(visibleRadar(r, false, now)?.pings).toHaveLength(1);
    expect(visibleRadar(r, false, now + 20_000)).toBeNull();
    // Repositioning shows the sample so the area is visible.
    expect(visibleRadar(r, true, now + 20_000)?.map).toBe('de_mirage');
    expect(visibleRadar(null, true, now)).toBeNull();
  });
});

describe('radar card', () => {
  it('shows the sample plan, the map hint and starts the sample on the overlay', () => {
    render(<RadarCard />);
    expect(screen.getByTestId('radar-plan')).toHaveAttribute('aria-label', 'Mirage');
    expect(screen.getByTestId('radar-ping-apps')).toBeInTheDocument();
    expect(screen.getByTestId('radar-no-map')).toHaveTextContent('Carte inconnue');
    fireEvent.click(screen.getByText('Essayer'));
    expect(useApp.getState().radarDemo).not.toBeNull();
    fireEvent.click(screen.getByRole('switch', { name: 'Radar des annonces' }));
    expect(useApp.getState().radar.on).toBe(false);
  });

  it('follows the map being played and the callouts heard', () => {
    playing('de_inferno');
    useApp.setState({
      feed: [
        {
          ...line(Date.now() - 500, 'двое на банане'),
          time: '',
          status: 'transcrit',
          shown: null,
          lat: null,
          err: false,
          src: 'ru',
        },
      ] as never,
    });
    render(<RadarCard />);
    expect(screen.getByText('Carte : Inferno')).toBeInTheDocument();
    expect(screen.getByTestId('radar-plan')).toHaveAttribute('aria-label', 'Inferno');
    expect(screen.getByTestId('radar-ping-banana')).toHaveTextContent('2');
    expect(screen.getByTestId('radar-panel')).toHaveTextContent('Banana×2');
    expect(screen.queryByTestId('radar-no-map')).toBeNull();
    act(() => useApp.getState().set({ radar: { ...RADAR_DEFAULTS, hold: 8 } }));
    expect(screen.getByRole('slider', { name: 'Zone allumée' })).toHaveValue('8');
  });
});
