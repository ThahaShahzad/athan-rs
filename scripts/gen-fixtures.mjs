// Generates tests/fixtures.json from the reference `adhan` npm package.
//
// Usage (from the athan-rs repo root):
//   cd scripts && npm install && cd ..
//   TZ=UTC node scripts/gen-fixtures.mjs
//
// TZ=UTC is required: the adhan library reads the local date components of
// the Date object; pinning UTC makes the fixtures timezone-independent.

import { Coordinates, CalculationMethod, Madhab, PrayerTimes } from 'adhan';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const outPath = join(here, '..', 'tests', 'fixtures.json');

const METHODS = {
  MuslimWorldLeague: () => CalculationMethod.MuslimWorldLeague(),
  Egyptian: () => CalculationMethod.Egyptian(),
  Karachi: () => CalculationMethod.Karachi(),
  UmmAlQura: () => CalculationMethod.UmmAlQura(),
  Dubai: () => CalculationMethod.Dubai(),
  MoonsightingCommittee: () => CalculationMethod.MoonsightingCommittee(),
  NorthAmerica: () => CalculationMethod.NorthAmerica(),
  Kuwait: () => CalculationMethod.Kuwait(),
  Qatar: () => CalculationMethod.Qatar(),
  Singapore: () => CalculationMethod.Singapore(),
  Tehran: () => CalculationMethod.Tehran(),
  Turkey: () => CalculationMethod.Turkey(),
};

const MADHABS = { Shafi: Madhab.Shafi, Hanafi: Madhab.Hanafi };

const LOCATIONS = [
  [40.7128, -74.006], // New York City
  [21.4225, 39.8262], // Makkah
  [51.5074, -0.1278], // London
  [69.6492, 18.9553], // Tromso (high latitude)
  [-33.8688, 151.2093], // Sydney
];

const DATES = ['2026-01-15', '2026-03-20', '2026-06-21', '2026-09-22', '2026-12-21'];

function isoOrNull(date) {
  return date instanceof Date && !isNaN(date.valueOf()) ? date.toISOString() : null;
}

const rows = [];
for (const [methodName, makeParams] of Object.entries(METHODS)) {
  for (const [madhabName, madhab] of Object.entries(MADHABS)) {
    for (const [lat, lng] of LOCATIONS) {
      for (const dateStr of DATES) {
        const [y, m, d] = dateStr.split('-').map(Number);
        const date = new Date(Date.UTC(y, m - 1, d));
        const params = makeParams();
        params.madhab = madhab;
        const times = new PrayerTimes(new Coordinates(lat, lng), date, params);
        rows.push({
          method: methodName,
          madhab: madhabName,
          lat,
          lng,
          date: dateStr,
          fajr: isoOrNull(times.fajr),
          sunrise: isoOrNull(times.sunrise),
          dhuhr: isoOrNull(times.dhuhr),
          asr: isoOrNull(times.asr),
          maghrib: isoOrNull(times.maghrib),
          isha: isoOrNull(times.isha),
        });
      }
    }
  }
}

writeFileSync(outPath, JSON.stringify(rows, null, 2) + '\n');
console.log(`wrote ${rows.length} fixture rows to ${outPath}`);
const nulls = rows.reduce(
  (n, r) => n + ['fajr', 'sunrise', 'dhuhr', 'asr', 'maghrib', 'isha'].filter((p) => r[p] === null).length,
  0,
);
console.log(`${nulls} null (uncomputable) prayer times across all rows`);
