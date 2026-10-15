// Schematic plans of the CS2 maps for the callout radar: each callout is a point
// placed roughly where it is on the in-game radar (x to the right, y down, 0..1).
// Drawn by hand from the usual callouts (src-tauri/src/gsi/maps.json): no game
// file is read or shipped. Positions are approximate, the plan is a diagram.

export type Side = 'a' | 'b' | 'mid' | 't' | 'ct';

export interface Zone {
  id: string;
  /** Callout as players say it in English, shown on the plan. */
  label: string;
  x: number;
  y: number;
  side: Side;
  /**
   * Other ways to say it (lowercase, `ё` written `е`). A word ending in `*` is a
   * prefix, for Russian endings: `банан*` matches банан, банане, бананом.
   */
  aliases: string[];
}

export interface MapLayout {
  id: string;
  name: string;
  zones: Zone[];
}

type Row = [id: string, label: string, x: number, y: number, side: Side, aliases?: string];

/** Zones every map has; their place comes from the map's own rows. */
const COMMON: Record<string, { label: string; side: Side; aliases: string[] }> = {
  a: { label: 'A', side: 'a', aliases: [] }, // contextual, see detect.ts
  b: { label: 'B', side: 'b', aliases: [] },
  mid: {
    label: 'Mid',
    side: 'mid',
    aliases: ['mid', 'middle', 'мид*', 'середин*', 'milieu'],
  },
  ct: {
    label: 'CT',
    side: 'ct',
    aliases: ['ct', 'ct spawn', 'кт', 'кт спавн', 'спавн кт'],
  },
  t: {
    label: 'T',
    side: 't',
    aliases: ['t spawn', 'tspawn', 'т спавн', 'тспавн', 'спавн т'],
  },
};

function layout(id: string, name: string, rows: Row[]): MapLayout {
  const zones = rows.map(([zid, label, x, y, side, aliases]): Zone => {
    const common = COMMON[zid];
    const extra = aliases ? aliases.split('|').map((s) => s.trim()) : [];
    const own = common ? common.aliases : [label.toLowerCase()];
    return {
      id: zid,
      label: common?.label ?? label,
      x,
      y,
      side: common?.side ?? side,
      aliases: [...own, ...extra],
    };
  });
  return { id, name, zones };
}

export const LAYOUTS: Record<string, MapLayout> = {
  de_mirage: layout('de_mirage', 'Mirage', [
    ['a', 'A', 0.33, 0.76, 'a'],
    ['b', 'B', 0.17, 0.2, 'b'],
    ['mid', 'Mid', 0.5, 0.44, 'mid'],
    ['ct', 'CT', 0.15, 0.56, 'ct'],
    ['t', 'T', 0.88, 0.42, 't'],
    ['palace', 'Palace', 0.58, 0.86, 'a', 'палас*|пэлас*|дворец*|pala'],
    ['ramp', 'Ramp', 0.6, 0.7, 'a', 'рамп*|a ramp'],
    ['tetris', 'Tetris', 0.48, 0.75, 'a', 'тетрис*'],
    ['sandwich', 'Sandwich', 0.41, 0.68, 'a', 'сэндвич*|сендвич*|бутерброд*'],
    ['firebox', 'Firebox', 0.27, 0.7, 'a', 'fire box|файрбокс*|фаербокс*'],
    ['ticket', 'Ticket', 0.2, 0.75, 'a', 'тикет*|билет*|касс*|booth'],
    ['triple', 'Triple', 0.37, 0.86, 'a', 'трипл*|triple box'],
    ['stairs', 'Stairs', 0.43, 0.6, 'a', 'стэйрс*|стейрс*|лестниц*'],
    ['jungle', 'Jungle', 0.33, 0.57, 'a', 'джангл*|джунгл*'],
    ['connector', 'Connector', 0.4, 0.49, 'mid', 'коннектор*|конектор*|conn'],
    ['topmid', 'Top mid', 0.71, 0.44, 'mid', 'топ мид*|верх мид*'],
    ['window', 'Window', 0.28, 0.43, 'mid', 'окн*|виндоу*|sniper nest'],
    ['short', 'Short', 0.29, 0.32, 'b', 'шорт*'],
    ['catwalk', 'Catwalk', 0.4, 0.34, 'mid', 'cat|кэт|кэтвок*|катвок*'],
    ['underpass', 'Underpass', 0.48, 0.34, 'mid', 'андерпас*|подземк*'],
    ['apps', 'Apps', 0.6, 0.18, 'b', 'apartments|апп*|апы|апах|апсы|апартамент*|квартир*'],
    ['market', 'Market', 0.17, 0.33, 'b', 'маркет*|рынок|рынк*'],
    ['kitchen', 'Kitchen', 0.29, 0.13, 'b', 'кухн*'],
    ['van', 'Van', 0.24, 0.23, 'b', 'вэн*|фургон*'],
    ['bench', 'Bench', 0.09, 0.25, 'b', 'бенч*|скамейк*|лавк*'],
  ]),
  de_inferno: layout('de_inferno', 'Inferno', [
    ['a', 'A', 0.82, 0.46, 'a'],
    ['b', 'B', 0.4, 0.16, 'b'],
    ['mid', 'Mid', 0.56, 0.62, 'mid'],
    ['ct', 'CT', 0.7, 0.2, 'ct'],
    ['t', 'T', 0.12, 0.82, 't'],
    ['secondmid', 'Second mid', 0.44, 0.74, 'mid', 'второй мид*|2nd mid|second middle'],
    ['apps', 'Apps', 0.68, 0.8, 'a', 'apartments|апп*|апы|апах|апсы|квартир*'],
    ['balcony', 'Balcony', 0.8, 0.66, 'a', 'балкон*|balc'],
    ['boiler', 'Boiler', 0.7, 0.7, 'a', 'бойлер*|котельн*'],
    ['pit', 'Pit', 0.92, 0.55, 'a', 'пит|яма|яме|яму|ямой'],
    ['graveyard', 'Graveyard', 0.89, 0.34, 'a', 'кладбищ*|грейвярд*|могил*'],
    ['library', 'Library', 0.76, 0.3, 'a', 'lib|библиотек*|либ*|лайбрари*'],
    ['arch', 'Arch', 0.65, 0.38, 'mid', 'арк*'],
    ['short', 'Short', 0.7, 0.54, 'a', 'шорт*'],
    ['long', 'Long', 0.63, 0.58, 'a', 'лонг*'],
    ['quad', 'Quad', 0.88, 0.44, 'a', 'квад*'],
    ['banana', 'Banana', 0.38, 0.53, 'b', 'банан*'],
    ['topbanana', 'Top banana', 0.4, 0.35, 'b', 'топ банан*|верх банан*'],
    ['logs', 'Logs', 0.42, 0.43, 'b', 'логс|бревн*'],
    ['car', 'Car', 0.47, 0.32, 'b', 'машин*'],
    ['ruins', 'Ruins', 0.31, 0.45, 'b', 'руин*'],
    ['coffins', 'Coffins', 0.5, 0.13, 'b', 'coffin|гроб*'],
    ['fountain', 'Fountain', 0.29, 0.19, 'b', 'фонтан*'],
    ['church', 'Church', 0.26, 0.09, 'b', 'церк*|черч*'],
    ['construction', 'Construction', 0.56, 0.21, 'b', 'construct|констракшн*|стройк*'],
    ['spools', 'Spools', 0.45, 0.24, 'b', 'spool|спул*|катушк*'],
  ]),
  de_dust2: layout('de_dust2', 'Dust II', [
    ['a', 'A', 0.78, 0.22, 'a'],
    ['b', 'B', 0.2, 0.2, 'b'],
    ['mid', 'Mid', 0.5, 0.53, 'mid'],
    ['ct', 'CT', 0.62, 0.17, 'ct'],
    ['t', 'T', 0.45, 0.9, 't'],
    ['longa', 'Long A', 0.85, 0.46, 'a', 'long|лонг*'],
    ['shorta', 'Short A', 0.63, 0.31, 'a', 'short|шорт*'],
    ['catwalk', 'Catwalk', 0.58, 0.4, 'a', 'cat|кэт|кэтвок*|катвок*|мостик*'],
    ['pit', 'Pit', 0.9, 0.6, 'a', 'пит|яма|яме|яму|ямой'],
    ['goose', 'Goose', 0.87, 0.17, 'a', 'гус*'],
    ['car', 'Car', 0.85, 0.32, 'a', 'машин*'],
    ['blue', 'Blue', 0.7, 0.78, 'a', 'блю|синий|синем|синего'],
    ['tunnels', 'Tunnels', 0.25, 0.55, 'b', 'tunnel|tuns|туннел*|тоннел*|тунел*|туны'],
    ['uppertunnels', 'Upper tunnels', 0.21, 0.67, 'b', 'upper|верх* туннел*|верх* тоннел*|верх* туны'],
    ['lowertunnels', 'Lower tunnels', 0.38, 0.57, 'mid', 'lower|низ* туннел*|низ* тоннел*|нижн* туны'],
    ['xbox', 'Xbox', 0.5, 0.42, 'mid', 'иксбокс*|x box'],
    ['middoors', 'Mid doors', 0.5, 0.3, 'mid', 'doors|двер*'],
    ['window', 'Window', 0.3, 0.22, 'b', 'окн*'],
    ['platform', 'Platform', 0.14, 0.14, 'b', 'plat|платформ*'],
    ['backplat', 'Back plat', 0.08, 0.09, 'b', 'бэк плат*'],
  ]),
  de_nuke: layout('de_nuke', 'Nuke', [
    ['a', 'A', 0.52, 0.48, 'a'],
    ['b', 'B', 0.52, 0.75, 'b'],
    ['mid', 'Mid', 0.36, 0.38, 'mid'],
    ['ct', 'CT', 0.88, 0.45, 'ct'],
    ['t', 'T', 0.08, 0.55, 't'],
    ['outside', 'Outside', 0.6, 0.22, 'mid', 'аутсайд*|улиц*'],
    ['lobby', 'Lobby', 0.29, 0.53, 'a', 'лобби'],
    ['squeaky', 'Squeaky', 0.4, 0.58, 'a', 'сквики|скрипучк*'],
    ['hut', 'Hut', 0.4, 0.45, 'a', 'хат*|будк*'],
    ['heaven', 'Heaven', 0.58, 0.39, 'a', 'хевен*|хэвен*'],
    ['hell', 'Hell', 0.56, 0.58, 'a', 'хелл|ад'],
    ['mini', 'Mini', 0.65, 0.5, 'a', 'мини'],
    ['rafters', 'Rafters', 0.48, 0.41, 'a', 'стропил*'],
    ['trophy', 'Trophy', 0.67, 0.42, 'a', 'трофи*|трофе*'],
    ['mustang', 'Mustang', 0.67, 0.33, 'a', 'мустанг*'],
    ['ramp', 'Ramp', 0.37, 0.7, 'b', 'рамп*'],
    ['vents', 'Vents', 0.51, 0.63, 'b', 'vent|вент*'],
    ['secret', 'Secret', 0.75, 0.66, 'b', 'секрет*'],
    ['silo', 'Silo', 0.4, 0.22, 'mid', 'силос*|сило'],
    ['garage', 'Garage', 0.77, 0.22, 'mid', 'гараж*'],
    ['control', 'Control room', 0.63, 0.69, 'b', 'control|контрол*'],
    ['decon', 'Decon', 0.63, 0.83, 'b', 'декон*'],
    ['toxic', 'Toxic', 0.45, 0.85, 'b', 'токсик*'],
    ['lockers', 'Lockers', 0.43, 0.63, 'b', 'локер*|шкафчик*'],
  ]),
  de_ancient: layout('de_ancient', 'Ancient', [
    ['a', 'A', 0.26, 0.32, 'a'],
    ['b', 'B', 0.76, 0.32, 'b'],
    ['mid', 'Mid', 0.5, 0.52, 'mid'],
    ['ct', 'CT', 0.52, 0.15, 'ct'],
    ['t', 'T', 0.5, 0.9, 't'],
    ['main', 'Main', 0.23, 0.63, 'a', 'мейн*'],
    ['donut', 'Donut', 0.4, 0.38, 'a', 'донат*|пончик*'],
    ['temple', 'Temple', 0.19, 0.23, 'a', 'храм*|темпл*'],
    ['elbow', 'Elbow', 0.34, 0.52, 'a', 'элбоу*|локот*|локт*'],
    ['jaguar', 'Jaguar', 0.33, 0.43, 'a', 'ягуар*'],
    ['house', 'House', 0.31, 0.18, 'a', 'хаус*|дом|доме'],
    ['treeroom', 'Tree room', 0.41, 0.26, 'a', 'tree|дерев*'],
    ['water', 'Water', 0.42, 0.67, 'mid', 'вод*'],
    ['redroom', 'Red room', 0.58, 0.67, 'mid', 'red room|красн* комнат*'],
    ['cave', 'Cave', 0.72, 0.48, 'b', 'пещер*|кейв*'],
    ['ruins', 'Ruins', 0.81, 0.6, 'b', 'руин*'],
    ['ramp', 'Ramp', 0.7, 0.6, 'b', 'рамп*'],
    ['alley', 'Alley', 0.62, 0.39, 'b', 'аллей*|переул*'],
    ['pillar', 'Pillar', 0.83, 0.27, 'b', 'столб*|пиллар*'],
    ['street', 'Street', 0.8, 0.75, 'b', 'улиц*|стрит*'],
  ]),
  de_anubis: layout('de_anubis', 'Anubis', [
    ['a', 'A', 0.78, 0.3, 'a'],
    ['b', 'B', 0.22, 0.3, 'b'],
    ['mid', 'Mid', 0.5, 0.53, 'mid'],
    ['ct', 'CT', 0.5, 0.14, 'ct'],
    ['t', 'T', 0.5, 0.9, 't'],
    ['canal', 'Canal', 0.47, 0.4, 'mid', 'канал*'],
    ['water', 'Water', 0.56, 0.47, 'mid', 'вод*'],
    ['bridge', 'Bridge', 0.44, 0.63, 'mid', 'мост*'],
    ['connector', 'Connector', 0.63, 0.38, 'a', 'conn|коннектор*|конектор*'],
    ['heaven', 'Heaven', 0.83, 0.21, 'a', 'хевен*|хэвен*'],
    ['walkway', 'Walkway', 0.72, 0.47, 'a', 'уокуэй*|воквей*'],
    ['main', 'Main', 0.8, 0.63, 'a', 'мейн*'],
    ['camera', 'Camera', 0.66, 0.23, 'a', 'камер*'],
    ['palace', 'Palace', 0.2, 0.63, 'b', 'палас*|дворец*'],
    ['street', 'Street', 0.31, 0.47, 'b', 'улиц*|стрит*'],
    ['alley', 'Alley', 0.34, 0.29, 'b', 'аллей*|переул*'],
    ['ruins', 'Ruins', 0.13, 0.23, 'b', 'руин*'],
  ]),
  de_vertigo: layout('de_vertigo', 'Vertigo', [
    ['a', 'A', 0.76, 0.28, 'a'],
    ['b', 'B', 0.22, 0.36, 'b'],
    ['mid', 'Mid', 0.5, 0.5, 'mid'],
    ['ct', 'CT', 0.5, 0.2, 'ct'],
    ['t', 'T', 0.55, 0.88, 't'],
    ['aramp', 'A ramp', 0.78, 0.57, 'a', 'ramp|рамп*'],
    ['sandbags', 'Sandbags', 0.82, 0.19, 'a', 'мешк*'],
    ['scaffolding', 'Scaffolding', 0.66, 0.17, 'a', 'scaffold|скаффолд*|леса|лесах'],
    ['heaven', 'Heaven', 0.7, 0.37, 'a', 'хевен*|хэвен*'],
    ['bstairs', 'B stairs', 0.24, 0.58, 'b', 'stairs|лестниц*'],
    ['elevator', 'Elevator', 0.37, 0.24, 'b', 'лифт*'],
    ['generator', 'Generator', 0.36, 0.41, 'b', 'генератор*'],
    ['window', 'Window', 0.29, 0.29, 'b', 'окн*'],
    ['tunnel', 'Tunnel', 0.28, 0.71, 'b', 'tunnels|туннел*|тоннел*'],
    ['construction', 'Construction', 0.43, 0.63, 'mid', 'construct|стройк*'],
    ['yellow', 'Yellow', 0.65, 0.63, 'mid', 'желт*'],
  ]),
  de_overpass: layout('de_overpass', 'Overpass', [
    ['a', 'A', 0.42, 0.24, 'a'],
    ['b', 'B', 0.71, 0.5, 'b'],
    ['mid', 'Mid', 0.47, 0.56, 'mid'],
    ['ct', 'CT', 0.62, 0.13, 'ct'],
    ['t', 'T', 0.3, 0.89, 't'],
    ['long', 'Long', 0.23, 0.38, 'a', 'лонг*'],
    ['bathrooms', 'Bathrooms', 0.3, 0.3, 'a', 'toilets|bathroom|туалет*'],
    ['party', 'Party', 0.19, 0.2, 'a', 'пати'],
    ['truck', 'Truck', 0.37, 0.16, 'a', 'трак*|грузовик*'],
    ['fountain', 'Fountain', 0.46, 0.37, 'a', 'фонтан*'],
    ['bank', 'Bank', 0.52, 0.2, 'a', 'банк*'],
    ['stairs', 'Stairs', 0.35, 0.43, 'a', 'лестниц*'],
    ['connector', 'Connector', 0.52, 0.47, 'mid', 'conn|коннектор*|конектор*'],
    ['playground', 'Playground', 0.39, 0.63, 'mid', 'площадк*|плейграунд*'],
    ['short', 'Short', 0.59, 0.58, 'b', 'шорт*'],
    ['heaven', 'Heaven', 0.77, 0.4, 'b', 'хевен*|хэвен*'],
    ['pillar', 'Pillar', 0.75, 0.57, 'b', 'столб*|пиллар*'],
    ['barrels', 'Barrels', 0.7, 0.43, 'b', 'barrel|бочк*'],
    ['graffiti', 'Graffiti', 0.64, 0.4, 'b', 'граффити|графити'],
    ['water', 'Water', 0.62, 0.65, 'b', 'вод*'],
    ['monster', 'Monster', 0.8, 0.72, 'b', 'монстр*'],
    ['sewers', 'Sewers', 0.66, 0.77, 'b', 'sewer|канализ*'],
  ]),
  de_train: layout('de_train', 'Train', [
    ['a', 'A', 0.5, 0.42, 'a'],
    ['b', 'B', 0.55, 0.76, 'b'],
    ['mid', 'Mid', 0.33, 0.56, 'mid'],
    ['ct', 'CT', 0.87, 0.4, 'ct'],
    ['t', 'T', 0.08, 0.5, 't'],
    ['ivy', 'Ivy', 0.29, 0.29, 'a', 'айви|плющ*'],
    ['olof', 'Olof', 0.39, 0.35, 'a', 'олоф*'],
    ['pinktrain', 'Pink train', 0.46, 0.34, 'a', 'pink|розов*'],
    ['bombtrain', 'Bombtrain', 0.53, 0.47, 'a', 'bomb train|бомбтрейн*'],
    ['sandwich', 'Sandwich', 0.45, 0.47, 'a', 'сэндвич*|сендвич*'],
    ['ladder', 'Ladder room', 0.63, 0.29, 'a', 'ladder|лэддер*'],
    ['heaven', 'Heaven', 0.62, 0.45, 'a', 'хевен*|хэвен*'],
    ['hell', 'Hell', 0.67, 0.53, 'a', 'хелл|ад'],
    ['zconnector', 'Z connector', 0.73, 0.5, 'a', 'z|зет*|зэт*'],
    ['popdog', 'Popdog', 0.4, 0.62, 'mid', 'pop dog|попдог*'],
    ['connector', 'Connector', 0.43, 0.54, 'mid', 'conn|коннектор*|конектор*'],
    ['upper', 'Upper', 0.6, 0.84, 'b', 'верх*'],
    ['lower', 'Lower', 0.5, 0.69, 'b', 'низ*'],
  ]),
};

/** Lanes drawn between nearby zones: each zone is joined to its two closest neighbours. */
export function lanes(m: MapLayout, maxDist = 0.26): [Zone, Zone][] {
  const out = new Map<string, [Zone, Zone]>();
  for (const z of m.zones) {
    const near = m.zones
      .filter((o) => o !== z)
      .map((o) => ({ o, d: Math.hypot(o.x - z.x, o.y - z.y) }))
      .filter((n) => n.d <= maxDist)
      .sort((p, q) => p.d - q.d)
      .slice(0, 2);
    for (const { o } of near) {
      const key = [z.id, o.id].sort().join('|');
      if (!out.has(key)) out.set(key, z.id < o.id ? [z, o] : [o, z]);
    }
  }
  return [...out.values()];
}
