#!/usr/bin/env python3
"""Generate fixed-profile CLDR recipes with a pinned runtime's pattern selection.

Usage: python3 tools/generate-date-time.py --archives DIR [--check]
Download the five pinned npm archives into DIR first; ordinary builds stay offline.
"""
import argparse
import hashlib
import io
import json
import subprocess
import tarfile
from pathlib import Path

PACKAGES = {
    "cldr-dates-full": "0256f1cefeca14f7d515be4872dda48fcdd7e75381430f25aafd0143eae5b430",
    "cldr-core": "5310e0c7a06c1feb83dc8e54c8584bbe0b9c2ea8320172a18d541986b124585d",
    "cldr-numbers-full": "2d17a1453c559a62112caeed52e0bcfe3cb8539c99d239ae7b7ed4d0827679d9",
    "cldr-cal-buddhist-full": "3429cb832bef99a978863f11a6afd8b36f5ce981ab18c51481648cef479400ff",
    "cldr-cal-persian-full": "937af05a2e84a3a38e9fd6880888c622ff14db6577fc254d71a91d2e0442f41f",
}
VERSION = "48.2.0"
ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "crates/maestro-text/data/date-time"
# CLDR supplies the domain and preferences. The pinned runtime resolves numeric
# skeleton matching and final whitespace, which cannot be copied from yMd alone.
PROBE = r"""
const assert = require('node:assert/strict');
assert.equal(process.versions.node, '24.20.0');
assert.equal(process.versions.icu, '78.3');
assert.equal(process.versions.cldr, '48.0');
const {spawnSync} = require('node:child_process');
const locales = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const expanded = [...new Set(locales.flatMap(id => [id, ...['US','FR','IR','TH'].map(region => new Intl.Locale(id, {region}).baseName)]))].sort();
const source = `
const f = new Intl.DateTimeFormat(undefined, {year:'numeric',month:'numeric',day:'numeric',hour:'numeric',minute:'numeric',second:'numeric',timeZone:'UTC'});
const d = new Date('2006-01-02T03:04:05Z');
const pm = new Date('2006-01-02T21:04:05Z');
console.log(JSON.stringify([f.resolvedOptions(), f.format(d), f.formatToParts(d), f.formatToParts(pm), f.format(pm)]));`;
const codes = {year:'y',month:'M',day:'d',hour:'H',minute:'m',second:'s',dayPeriod:'p',era:'e'};
const rows = [], records = [], ids = new Map();
for (const locale of expanded) {
  const env = {...process.env, LANG:locale.replaceAll('-','_')+'.UTF-8', TZ:'UTC'};
  for (const key of ['LC_ALL','LC_MESSAGES','LC_TIME']) delete env[key];
  const child = spawnSync(process.execPath, ['-e',source], {env,encoding:'utf8'});
  assert.equal(child.status,0,child.stderr);
  const [o,text,parts,evening,pmText] = JSON.parse(child.stdout);
  const joined = parts.map(p=>p.value).join('');
  const normalize = s=>s.replace(/[\u00a0\u202f]/g,' ');
  assert(joined === text || normalize(joined) === text);
  const clean = s=>joined === text ? s : normalize(s);
  const pmJoined = evening.map(p=>p.value).join('');
  assert(pmJoined === pmText || normalize(pmJoined) === pmText);
  const cleanPM = s=>pmJoined === pmText ? s : normalize(s);
  let am='',pm='',era='';
  const pattern = parts.map(p=>{
    if (p.type==='literal') { assert(!p.value.includes('%')); return clean(p.value); }
    assert(codes[p.type]);
    if (p.type==='month') assert(['numeric','2-digit'].includes(o.month));
    if (p.type==='dayPeriod') { am=clean(p.value); pm=cleanPM(evening.find(q=>q.type==='dayPeriod').value); }
    if (p.type==='era') era=clean(p.value);
    return '%'+(o[p.type]==='2-digit'?'2':'')+codes[p.type];
  }).join('');
  assert(['gregory','persian','buddhist'].includes(o.calendar));
  assert(['h12','h23'].includes(o.hourCycle));
  const record=[o.calendar,o.numberingSystem,o.hourCycle,pattern,am,pm,era];
  assert(record.every(s=>!/[\t\r\n]/.test(s)));
  const key=JSON.stringify(record);
  if (!ids.has(key)) { ids.set(key,records.length); records.push(record); }
  rows.push([locale,ids.get(key)]);
}
console.log(JSON.stringify({rows,records}));
"""


def archive(directory, package):
    """Read only a hash-verified archive; never extract paths to disk."""
    path = directory / f"{package}-{VERSION}.tgz"
    if not path.exists():
        path = directory / f"{package}.tgz"
    raw = path.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == PACKAGES[package], path
    return tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz")


def read_json(archive_file, name):
    """Read a named JSON member from a verified archive."""
    member = archive_file.extractfile("package/" + name)
    assert member is not None
    return json.load(member)


def tsv(rows):
    """Encode delimiter-free rows deterministically."""
    assert all(not any(c in str(field) for c in "\t\r\n") for row in rows for field in row)
    return ("\n".join("\t".join(map(str, row)) for row in rows) + "\n").encode()


def main():
    """Generate assets or compare their exact bytes in check mode."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archives", required=True, type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    archives = {name: archive(args.archives, name) for name in PACKAGES}
    dates = archives["cldr-dates-full"]
    locales = sorted({name.split('/')[2] for name in dates.getnames() if name.startswith('package/main/') and name.endswith('/ca-gregorian.json')})
    assert len(locales) == 766
    for name in ("cldr-numbers-full", "cldr-cal-buddhist-full", "cldr-cal-persian-full"):
        assert archives[name].getnames()
    result = subprocess.run(["node", "-e", PROBE], input=json.dumps(locales), text=True, check=True, capture_output=True)
    data = json.loads(result.stdout)
    assert len(data["rows"]) == 2301 and len(data["records"]) == 444
    core = archives["cldr-core"]
    hours = read_json(core, "supplemental/timeData.json")["supplemental"]["timeData"]
    calendars = read_json(core, "supplemental/calendarPreferenceData.json")["supplemental"]["calendarPreferenceData"]
    defaults = sorted((region, preferences[0]) for region, preferences in calendars.items() if preferences[0] != "gregorian")
    assert {calendar for _, calendar in defaults} == {"persian", "buddhist"}
    licence = core.extractfile("package/LICENSE")
    assert licence is not None
    assets = {
        "index.tsv": tsv(data["rows"]),
        "recipes.tsv": tsv(data["records"]),
        "hours.tsv": tsv(sorted((region, 'h12' if entry['_preferred']=='h' else 'h23') for region, entry in hours.items())),
        "calendars.tsv": tsv(defaults),
        "LICENSE": licence.read(),
    }
    DEST.mkdir(parents=True, exist_ok=True)
    for name, content in assets.items():
        if args.check:
            assert (DEST / name).read_bytes() == content, f"stale asset: {name}"
        else:
            (DEST / name).write_bytes(content)
    print(f"PASS: {len(data['rows'])} locales, {len(data['records'])} recipes, {sum(map(len, assets.values()))} bytes")


if __name__ == "__main__":
    main()
