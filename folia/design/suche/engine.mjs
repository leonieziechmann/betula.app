// The search of the module catalog twice, on the same data: as Folia has it on 2026-10-19
// (`searchOld`, folia/crates/search/src/lib.rs: its SQL scores ported, the same 49 hits for „TI“ in
// the same order) and as the concept proposes it (`searchNew`, folia/design/suche/konzept.html).
// One file for the concept's page (build.mjs inlines it) and for Node; no dependencies.

// ---------------------------------------------------------------- folding, as Folia and Radix fold
const FOLD = new Map();
for (const c of "äàáâãå") FOLD.set(c, "a");
for (const c of "öòóôõø") FOLD.set(c, "o");
for (const c of "üùúû") FOLD.set(c, "u");
for (const c of "èéêë") FOLD.set(c, "e");
for (const c of "ìíîï") FOLD.set(c, "i");
FOLD.set("ç", "c"); FOLD.set("ñ", "n"); FOLD.set("ß", "ss");

export function fold(text) {
  let out = "";
  for (const ch of text) for (const c of ch.toLowerCase()) out += FOLD.get(c) ?? c;
  return out;
}

const NOT_WORD = /[^\p{Alphabetic}\p{N}]+/u;
export const words = (text) => fold(text).split(NOT_WORD).filter(Boolean);

const ROMAN = ["i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x"];
const isDigits = (s) => /^[0-9]+$/.test(s);
const isLetters = (s) => /^\p{Alphabetic}+$/u.test(s);
const nocase = (s) => s.replace(/[A-Z]/g, (c) => c.toLowerCase());
const compareTitle = (a, b) => { const x = nocase(a), y = nocase(b); return x < y ? -1 : x > y ? 1 : 0; };

function queryWords(text, fillers) {
  const all = words(text);
  const content = all.filter((w) => !fillers.has(w));
  return [...new Set(content.length ? content : all)];
}

/** A number up to ten is also found as its Roman numeral and the other way round; a number of one
 *  or two digits and a numeral only as a whole word. */
function forms(word) {
  const whole = (isDigits(word) && [...word].length < 3) || ROMAN.includes(word);
  const out = [{ text: word, whole }];
  if (isDigits(word)) { const n = parseInt(word, 10); if (n >= 1 && n <= 10) out.push({ text: ROMAN[n - 1], whole: true }); }
  const r = ROMAN.indexOf(word);
  if (r >= 0) out.push({ text: String(r + 1), whole: true });
  return out;
}

// ---------------------------------------------------------------- the data
// data.modules, a row each: [id, the title the list shows, the other title or "", 1 if the shown one
// is English, status (0 offered, 1 phase-out, 2 no longer offered), Radix's folded abbreviations (its
// own, the programs', the known forms), [its own and the programs' abbreviations as written],
// programs (latest PO) whose curriculum has it, [their indices], [[a lecturer's index, 0 responsible
// or 1 teaching]]]. data.lecturers: the names as QIS writes them („Hofstedt, Petra").
export function buildIndex(data) {
  const names = (data.lecturers ?? []).map(parseName);
  const modules = data.modules.map((r) => {
    const [id, title, other, shownEn, status, radixAbbrevs, abbrevs, pop, progs, lecturers = []] = r;
    const de = shownEn ? other || null : title;
    const en = shownEn ? title : other || title;
    const initials = [];
    for (const t of [de, en]) {
      const i = t == null ? "" : searchInitials(t);
      if (i && !initials.includes(i)) initials.push(i);
    }
    return {
      id, title, other: other || null, shownEn: !!shownEn, status, pop, progs: new Set(progs), de, en,
      // what today's SQL compares (v_module_folded, as Radix folds it)
      old: { de: searchText(de), en: searchText(en), ini: initials.join(" ") || null, ab: radixAbbrevs || null },
      abbrevs: abbrevs.map((raw) => ({ raw, folded: words(raw).join("") })).filter((a) => a.folded),
      lecturers: lecturers.map(([i, role]) => ({ ...names[i], role })),
      shownT: null, otherT: null,
    };
  });
  return { modules, programs: data.programs, snapshot: data.snapshot, vocabulary: makeVocabulary(modules) };
}

// ================================================================= today
// Radix's folded names (normalize.SearchText, SearchInitials) and Folia's scores (search::Form::case).
export const OLD_FILLERS = new Set(["of", "and", "und", "der", "die", "das", "in", "im", "fur", "the", "zur", "zum", "von", "mit"]);

function searchText(title) {
  if (title == null) return null;
  const ws = words(title);
  for (const chunk of title.split(/\s+/).filter(Boolean)) {
    const parts = words(chunk);
    if (parts.length > 1) ws.push(parts.join(""));
  }
  return ws.length ? ws.join(" ") : null;
}

function searchInitials(title) {
  let out = "", n = 0;
  for (const w of words(title)) {
    if (OLD_FILLERS.has(w)) continue;
    out += [...w][0];
    n++;
  }
  return n < 2 ? "" : out;
}

const ID = 1200, ABBREV = 1100, FIRST_WORD = 1050, WHOLE_WORD = 1000, ID_START = 900, FIRST_WORD_START = 850,
  WORD_START = 800, INITIALS = 600, WORD_PART = 500;
export const OLD_KIND = { [ID]: "Modulnummer", [ABBREV]: "Kürzel", [FIRST_WORD]: "erstes Wort", [WHOLE_WORD]: "ganzes Wort",
  [ID_START]: "Anfang der Nummer", [FIRST_WORD_START]: "Anfang des ersten Worts", [WORD_START]: "Wortanfang",
  [INITIALS]: "Anfangsbuchstaben", [WORD_PART]: "im Wort" };

function oldCase(form, m) {
  const t = form.text, len = [...t].length, number = isDigits(t), letters = isLetters(t);
  const o = m.old;
  const de = " " + (o.de ?? "") + " ", en = " " + (o.en ?? "") + " ", ini = " " + (o.ini ?? "") + " ", ab = " " + (o.ab ?? "") + " ";
  const hit = (score, where) => ({ score, where, text: t });
  if (number && len >= 3 && m.id === t) return hit(ID, "id");
  if (len >= 2 && ab.includes(" " + t + " ")) return hit(ABBREV, "ab");
  if (de.startsWith(" " + t + " ")) return hit(FIRST_WORD, "de");
  if (en.startsWith(" " + t + " ")) return hit(FIRST_WORD, "en");
  if (de.includes(" " + t + " ")) return hit(WHOLE_WORD, "de");
  if (en.includes(" " + t + " ")) return hit(WHOLE_WORD, "en");
  if (number && len >= 3 && m.id.startsWith(t)) return hit(ID_START, "id");
  if (!form.whole) {
    if (de.startsWith(" " + t)) return hit(FIRST_WORD_START, "de");
    if (en.startsWith(" " + t)) return hit(FIRST_WORD_START, "en");
    if (de.includes(" " + t)) return hit(WORD_START, "de");
    if (en.includes(" " + t)) return hit(WORD_START, "en");
    if (letters && len >= 2 && ini.includes(t)) return hit(INITIALS, "ini");
    if (len >= 4 && de.includes(t)) return hit(WORD_PART, "de");
    if (len >= 4 && en.includes(t)) return hit(WORD_PART, "en");
  }
  return null;
}

function oldRun(index, text, resolution) {
  const plan = queryWords(text, OLD_FILLERS).map((typed) => {
    const c = resolution?.corrected.find((c) => c.typed === typed);
    return forms(c ? c.word : typed);
  });
  if (!plan.length) return [];
  const out = [];
  for (const m of index.modules) {
    let score = 0, matched = 0;
    const why = plan.map((wordForms) => {
      let best = null;
      for (const f of wordForms) { const h = oldCase(f, m); if (h && (!best || h.score > best.score)) best = h; }
      if (best) { score += best.score; matched++; }
      return best;
    });
    if (resolution?.mostWords ? matched > 0 : matched === plan.length) out.push({ m, score, matched, why });
  }
  // matched DESC, score DESC, title COLLATE NOCASE, id (query::sql::order_terms)
  out.sort((a, b) => b.matched - a.matched || b.score - a.score || compareTitle(a.m.title, b.m.title) || (a.m.id < b.m.id ? -1 : 1));
  return out;
}

/** Where a hit of today is in the title one sees: whether it is the other title, and the letters. */
export function oldWhere(m, h) {
  if (!h || h.where === "id" || h.where === "ab") return { other: false, spans: [] };
  const t = h.text;
  const titles = [{ raw: m.de, key: "de", hidden: m.shownEn }, { raw: m.en, key: "en", hidden: !m.shownEn }]
    .filter((x) => x.raw != null && (h.where === "ini" || h.where === x.key));
  for (const { raw, hidden } of titles) {
    const T = tokenize(raw);
    let spans = [];
    if (h.where === "ini") {
      const content = T.toks.filter((x) => !OLD_FILLERS.has(x.text));
      const letters = content.map((x) => x.text[0]).join("");
      const at = letters.indexOf(t);
      if (at < 0 || content.length < 2) continue;
      spans = content.slice(at, at + t.length).map((x) => [x.start, x.map[1] ?? x.end]);
    } else {
      const whole = h.score === FIRST_WORD || h.score === WHOLE_WORD;
      const tok = T.toks.find((x) => (whole ? x.text === t : h.score === WORD_PART ? x.text.includes(t) : x.text.startsWith(t)));
      if (!tok) continue;
      const from = h.score === WORD_PART ? tok.text.indexOf(t) : 0;
      spans = [spanOf(tok, from, whole ? tok.text.length : t.length)];
    }
    return { other: hidden && m.other != null, raw, spans };
  }
  return { other: false, spans: [] };
}

// ---------------------------------------------------------------- typos (search::resolve), for both
function editDistance(a, b) {
  const width = b.length + 1;
  let before = new Array(width).fill(0), previous = [...Array(width).keys()];
  for (let i = 0; i < a.length; i++) {
    const current = new Array(width).fill(i + 1);
    for (let j = 0; j < b.length; j++) {
      let value = Math.min(previous[j + 1] + 1, current[j] + 1, previous[j] + (a[i] !== b[j] ? 1 : 0));
      if (i > 0 && j > 0 && a[i - 1] === b[j] && b[j - 1] === a[i]) value = Math.min(value, before[j - 1] + 1);
      current[j + 1] = value;
    }
    before = previous; previous = current;
  }
  return previous[previous.length - 1];
}

/** One typo from four letters on, two from eight, the first letter right (fuzzy::typos_at_start). */
function typosAtStart(query, word) {
  if (query.length < 4 || query[0] !== word[0]) return null;
  const allowed = query.length >= 8 ? 2 : 1;
  let min = null;
  for (const length of [query.length - 1, query.length, query.length + 1]) {
    if (length > word.length) continue;
    const d = editDistance(query, word.slice(0, length));
    if (min === null || d < min) min = d;
  }
  return min !== null && min >= 1 && min <= allowed ? min : null;
}

function makeVocabulary(modules) {
  const counted = new Map();
  for (const m of modules) {
    const seen = new Set();
    for (const title of [m.de, m.en]) {
      if (title == null) continue;
      for (const piece of title.split(NOT_WORD).filter(Boolean)) {
        for (const w of words(piece)) {
          if (seen.has(w)) continue;
          seen.add(w);
          const c = counted.get(w);
          if (c) c[0]++; else counted.set(w, [1, piece]);
        }
      }
    }
  }
  return [...counted].map(([w, [n, shown]]) => [w, n, shown]).sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
}

function correct(vocabulary, word) {
  const typed = [...word];
  if (!typed.every((c) => /\p{Alphabetic}/u.test(c))) return null;
  let best = null;
  for (const [cand, n, shown] of vocabulary) {
    const t = typosAtStart(typed, [...cand]);
    if (t === null) continue;
    const better = !best || t < best.t || (t === best.t && (n > best.n || (n === best.n &&
      (cand.length < best.cand.length || (cand.length === best.cand.length && cand < best.cand)))));
    if (better) best = { t, n, cand, shown };
  }
  return best ? { word: best.cand, shown: best.shown } : null;
}

/** Where the words as typed find nothing: their typos corrected, else the modules with most of them. */
function resolveWith(index, text, run, fillers) {
  const typed = queryWords(text, fillers);
  if (!typed.length || run(text, null).length) return null;
  const any = run(text, { corrected: [], mostWords: true });
  const corrected = [];
  typed.forEach((w, i) => {
    if (any.some((r) => r.why[i])) return;
    const c = correct(index.vocabulary, w);
    if (c) corrected.push({ typed: w, word: c.word, shown: c.shown });
  });
  if (corrected.length) {
    const r = { corrected, mostWords: false };
    if (run(text, r).length) return r;
  }
  if (typed.length > 1) {
    const r = { corrected, mostWords: true };
    if (run(text, r).length) return r;
  }
  return null;
}

export function searchOld(index, text) {
  const run = (t, r) => oldRun(index, t, r);
  const resolution = resolveWith(index, text, run, OLD_FILLERS);
  return { resolution, rows: run(text, resolution) };
}

// ================================================================= the concept
// Fillers, for the query and for the titles: a query leaves them out, a short form may take their
// letter („AuD“, „MfI“) or leave them out, and they end a group of words. „Prof.“ and „Dr.“ too,
// so that „Prof. Hofstedt“ is Hofstedt.
export const FILLERS = new Set([
  "und", "oder", "sowie", "der", "die", "das", "des", "dem", "den", "ein", "eine", "einer", "eines", "einem", "einen",
  "in", "im", "am", "an", "auf", "aus", "bei", "fur", "mit", "nach", "uber", "unter", "von", "vom", "zu", "zur", "zum",
  "als", "durch", "ist", "sind", "ich", "wir", "wie", "was", "kann", "auch", "ihr", "ihre", "ihrer", "ihren", "sein",
  "seine", "seiner", "seinen", "zwischen", "gegen", "ohne", "um", "vor", "bis", "innerhalb", "ausserhalb",
  "of", "and", "or", "the", "to", "for", "from", "with", "by", "into", "on", "at", "as", "s", "is", "are", "its",
  "their", "our", "your", "this", "these", "that", "how", "what", "may", "can", "have", "has", "between", "within",
  "without", "about", "across", "through", "towards", "under", "over", "after", "before", "beyond", "among", "per",
  "via", "vs", "versus", "prof", "dr",
]);
// Short forms everybody knows (Radix's abbrev.knownForms): a word that is or contains the long form.
const KNOWN = { bwl: "betriebswirtschaftslehre", vwl: "volkswirtschaftslehre", sw: "software", hw: "hardware" };
const isNumeral = (w) => ROMAN.includes(w) || /[0-9]/.test(w);

/** The points of a find, and what changes them. The page's sliders change `program`, `other` and
 *  `caseMatch`; everything else is the concept's. */
export const WEIGHTS = {
  // the query names the whole module
  number: 125, abbrev: 125, abbrevPart: 120, title: 120,
  // a word of the query in a title
  word: 80, short: 80, prefix: 60, shortSkip: 55, numberStart: 60, inside: 35,
  // a word of the query in the name of a lecturer
  name: 75, nameStart: 55, nameLike: 50,
  // factors
  other: 0.75, // a find in the title of the other language
  caseMatch: 1.1, // capitals as typed (only where the query has capitals)
  perWord: 0.03, maxWords: 5, // every word before the find in the title, up to five
  phrase: 1.05, // the words of the query side by side, in their order
  instructor: 0.9, // a lecturer who teaches the module, not responsible for it
  program: 1.2, // a module of my program: the order only, never what is found
};
export const DEFAULT_WEIGHTS = { ...WEIGHTS };

/** What a module's find is called on the page, by its points. */
export const LEVELS = [[120, "Genau"], [75, "Wort oder Kürzel"], [50, "Wortanfang"], [0, "Im Wort"]];

/** The words of a text with where each stands (for the marks), and what the short forms need:
 *  fillers, numerals, and what lies between two words. */
export function tokenize(raw) {
  const toks = [];
  const re = /[\p{Alphabetic}\p{N}]+/gu;
  let mt;
  while ((mt = re.exec(raw))) {
    const start = mt.index;
    let text = "";
    const map = [];
    for (let i = 0; i < mt[0].length;) {
      const ch = String.fromCodePoint(mt[0].codePointAt(i));
      for (const f of fold(ch)) { text += f; map.push(start + i); }
      i += ch.length;
    }
    toks.push({ text, start, end: start + mt[0].length, map });
  }
  let content = 0;
  toks.forEach((t, i) => {
    const gap = raw.slice(i ? toks[i - 1].end : 0, t.start);
    // none: a space („Kinder- und“ too); compound: inside a word written in parts („Real-Time“,
    // „B.Sc.“); weak: „/“, „&“, „+“; strong: any other mark („:“, „,“, „(“, „ - “)
    t.gap = i === 0 ? (gap.trim() ? "strong" : "none")
      : /^\s+$/.test(gap) || /^-\s+$/.test(gap) ? "none"
      : /^[-.'’]+$/.test(gap) ? "compound"
      : /^[\s/&+]+$/.test(gap) || /^-\s*[&+/]\s*$/.test(gap) ? "weak" : "strong";
    t.filler = FILLERS.has(t.text);
    t.numeral = isNumeral(t.text);
    t.content = !t.filler && !t.numeral;
    t.pos = content;
    if (t.content) content++;
  });
  // A word written in parts is one word too („E-Technik“ is etechnik, „Master-Arbeit“ masterarbeit).
  const joined = [];
  for (let i = 0; i < toks.length;) {
    let j = i + 1;
    while (j < toks.length && toks[j].gap === "compound") j++;
    if (j - i > 1) {
      const parts = toks.slice(i, j);
      joined.push({ text: parts.map((p) => p.text).join(""), start: parts[0].start, end: parts[parts.length - 1].end,
        map: parts.flatMap((p) => p.map), from: i, to: j - 1 });
    }
    i = j;
  }
  const firstContent = toks.findIndex((t) => t.content);
  let lastContent = -1;
  toks.forEach((t, i) => { if (t.content) lastContent = i; });
  return { raw, toks, joined, firstContent, lastContent };
}

const spanOf = (tok, from, len) => [tok.map[from] ?? tok.end, from + len < tok.map.length ? tok.map[from + len] : tok.end];

// A short form names the end of a group of words: it ends at the title's end, a filler, a numeral
// or a mark („Explainable Machine Learning“ is ML, „Power Plant Technology“ no PP). In the title one
// sees it may begin anywhere but inside a word written in parts; in the other one only where a
// group begins too.
const startsGroup = (T, i, strict) => strict
  ? i === 0 || T.toks[i].gap === "strong" || T.toks[i].gap === "weak" || !T.toks[i - 1].content
  : T.toks[i].gap !== "compound";
const endsGroup = (T, i) => i === T.toks.length - 1 || T.toks[i + 1].gap === "strong" || T.toks[i + 1].gap === "weak"
  || !T.toks[i + 1].content;

/** Every way `q` is the short form of words that follow each other: at least two pieces, each the
 *  start of a word, none longer than the one before („TheoInf“, „LinA“, „AuD“, „TI“; not „b|au“);
 *  no word left out but fillers; a numeral never a piece („Technik I“ is no „TI“). */
function shortForms(q, T, strict) {
  const out = [];
  const toks = T.toks;
  const walk = (i, off, pieces, from, skipped) => {
    const t = toks[i];
    if (!t || t.numeral) return;
    const prev = pieces.length ? pieces[pieces.length - 1][1] : Infinity;
    for (let len = Math.min(q.length - off, t.text.length, prev); len >= 1; len--) {
      if (!t.text.startsWith(q.slice(off, off + len))) continue;
      const next = [...pieces, [i, len]];
      if (off + len === q.length) {
        if (next.length >= 2 && t.content && endsGroup(T, i)) out.push({ pieces: next, from, to: i, skipped });
        continue;
      }
      // the next word, or past fillers; never past a mark or past a word that is no filler
      for (let j = i + 1, skip = 0; j < toks.length && toks[j].gap !== "strong"; j++, skip++) {
        walk(j, off + len, next, from, skipped + skip);
        if (!toks[j].filler) break;
      }
    }
  };
  for (let s = 0; s < toks.length; s++) if (toks[s].content && startsGroup(T, s, strict)) walk(s, 0, [], s, 0);
  return out;
}

/** Capitals as typed: where the query word has a capital, the letters a find marks are written as
 *  typed („TI“ is so in „TIN*“, not in „Tissue“). `null` where the query has no capital. */
function caseOf(qw, raw, spans) {
  if (!/\p{Lu}/u.test(qw.raw)) return null;
  let typed = "";
  for (const [a, b] of spans) typed += raw.slice(a, b);
  return typed === qw.raw;
}

/** Every way one word of the query is found in one title, with its points. */
function findsIn(qw, T, other) {
  const W = WEIGHTS;
  const { q, strong } = qw;
  const out = [];
  const whole = (isDigits(q) && q.length < 3) || ROMAN.includes(q);
  const alts = forms(q).map((f) => f.text);
  const known = KNOWN[q];
  const add = (kind, base, from, to, spans, extra = {}) => {
    const pos = T.toks[from].pos;
    const c = kind === "inside" ? null : caseOf(qw, T.raw, spans);
    const points = base * (1 - W.perWord * Math.min(pos, W.maxWords)) * (other ? W.other : 1) * (c ? W.caseMatch : 1);
    out.push({ kind, from, to, pos, spans, points, other, caseMatch: c === true, ...extra });
  };
  T.toks.forEach((t, i) => {
    if (alts.includes(t.text)) add("word", W.word, i, i, [[t.start, t.end]]);
    else if (known && t.text.includes(known)) add("word", W.word, i, i, [[t.start, t.end]], { known: q.toUpperCase() });
    else if (!whole && t.text.startsWith(q)) add("prefix", W.prefix, i, i, [spanOf(t, 0, q.length)]);
    else if (!whole && q.length >= 4 && t.text.indexOf(q) > 0) add("inside", W.inside, i, i, [spanOf(t, t.text.indexOf(q), q.length)]);
  });
  for (const j of T.joined) {
    if (j.text === q) add("word", W.word, j.from, j.to, [[j.start, j.end]]);
    else if (!whole && j.text.startsWith(q) && !T.toks[j.from].text.startsWith(q)) add("prefix", W.prefix, j.from, j.to, [spanOf(j, 0, q.length)]);
  }
  if (strong && !whole && isLetters(q) && q.length >= 2) {
    for (const s of shortForms(q, T, other)) {
      add("short", s.skipped ? W.shortSkip : W.short, s.from, s.to, s.pieces.map(([i, len]) => spanOf(T.toks[i], 0, len)), { skipped: s.skipped });
    }
  }
  // In the title one does not see, a word of one or two letters finds no word's start („ti“ not
  // in „Timber“), only a whole word, a short form, or the start of a word in capitals („DB“ in „DBS“).
  if (other && q.length <= 2) {
    return out.filter((x) => x.kind === "word" || (x.kind === "short" && !x.skipped)
      || (x.kind === "prefix" && /^\p{Lu}{2,}$/u.test(T.raw.slice(T.toks[x.from].start, T.toks[x.from].end))));
  }
  return out;
}

/** Do the query's words, in their order, name every word of the title but fillers and numerals? One
 *  word only as a whole word or a short form („Datenbanken“, „TI“), several also as starts
 *  („theo inf“, „mathe 1“). With capitals as typed, if it can (`true`), else without (`false`);
 *  `null`: no. */
function covers(qs, T, all) {
  const toks = T.toks;
  const run = (strictCase) => {
    const memo = new Map();
    const go = (k, t) => {
      const key = k * 1000 + t;
      if (memo.has(key)) return memo.get(key);
      let ok = false;
      if (k === qs.length) ok = toks.slice(t).every((x) => !x.content);
      else {
        if (t < toks.length && !toks[t].content) ok = go(k, t + 1);
        for (const x of all[k]) {
          if (ok) break;
          if (x.from !== t || x.kind === "inside" || (x.kind === "prefix" && qs.length === 1) || (x.kind === "short" && x.skipped)) continue;
          if (strictCase && !x.caseMatch) continue;
          ok = go(k + 1, x.to + 1);
        }
      }
      memo.set(key, ok);
      return ok;
    };
    return go(0, 0);
  };
  const capitals = qs.some((w) => /\p{Lu}/u.test(w.raw));
  if (capitals && run(true)) return true;
  return run(false) ? false : null;
}

// ---------------------------------------------------------------- lecturers
// Restricted, but forgiving with spelling: a surname whole (from three letters), its start from
// four, or a similar spelling from four (one slip, two from eight letters, the first letter right,
// after ä/ae, dt/tt/t, ph/f, th/t, ck/k, tz/z, ie/i, y/i, ei/ai/ey/ay, a silent h and doubled
// letters are made alike); a first name only with the same person's surname. No inside a word,
// no short forms.
const TITLE_WORDS = new Set(["prof", "dr", "rer", "nat", "habil", "ing", "phil", "med", "medic", "oec", "pol", "jur",
  "techn", "paed", "apl", "pd", "dipl", "mult", "h", "c", "hc", "sc", "soc", "publ", "theol", "agr", "des", "univ", "em"]);
const PARTICLES = new Set(["von", "van", "de", "der", "den", "del", "della", "la", "le", "zu", "zur", "di", "da", "dos", "du", "ten", "ter", "vom"]);

function parseName(raw) {
  const comma = raw.indexOf(",");
  const toks = tokenize(raw).toks;
  const surname = toks.filter((t) => (comma < 0 || t.start < comma) && !TITLE_WORDS.has(t.text) && !PARTICLES.has(t.text) && !/[0-9]/.test(t.text));
  const first = toks.filter((t) => comma >= 0 && t.start > comma && !TITLE_WORDS.has(t.text));
  return { raw, surname, first };
}

/** Spellings made alike, for similar names only. */
export function alike(word) {
  return word
    .replace(/ae/g, "a").replace(/oe/g, "o").replace(/ue/g, "u")
    .replace(/dt/g, "t").replace(/th/g, "t").replace(/ph/g, "f").replace(/ck/g, "k").replace(/tz/g, "z")
    .replace(/ie/g, "i").replace(/y/g, "i").replace(/e[iy]|a[iy]/g, "ai")
    .replace(/([aeiou])h/g, "$1")
    .replace(/(.)\1+/g, "$1");
}

function similarName(q, name) {
  if (q.length < 4 || q[0] !== name[0]) return false;
  const a = alike(q), b = alike(name);
  if (a[0] !== b[0]) return false;
  return editDistance([...a], [...b]) <= (a.length >= 8 ? 2 : 1);
}

/** How the words of the query are found in the names of a module's lecturers: per word its best. */
function findsInLecturers(m, qs) {
  const W = WEIGHTS;
  const best = qs.map(() => null);
  for (const L of m.lecturers) {
    const role = L.role === 1 ? W.instructor : 1;
    const own = qs.map((w) => {
      let found = null;
      if (!isLetters(w.q)) return null;
      for (const t of L.surname) {
        const kind = w.q === t.text && w.q.length >= 3 ? "name"
          : w.q.length >= 4 && t.text.startsWith(w.q) ? "nameStart"
          : similarName(w.q, t.text) ? "nameLike" : null;
        if (!kind) continue;
        const points = W[kind] * role;
        const span = kind === "nameStart" ? spanOf(t, 0, w.q.length) : [t.start, t.end];
        if (!found || points > found.points) found = { kind, points, spans: [span], lecturer: L, surname: true };
      }
      return found;
    });
    // a first name (or its initial) counts with the surname of the same person
    if (own.some((f) => f?.surname)) {
      qs.forEach((w, k) => {
        if (own[k]) return;
        const t = L.first.find((f) => f.text === w.q || (w.q.length === 1 && f.text[0] === w.q));
        if (t) own[k] = { kind: "firstName", points: W.name * role, spans: [w.q.length === 1 ? spanOf(t, 0, 1) : [t.start, t.end]], lecturer: L };
      });
    }
    own.forEach((f, k) => { if (f && (!best[k] || f.points > best[k].points)) best[k] = f; });
  }
  return best;
}

// ---------------------------------------------------------------- a module
function evaluate(m, qs) {
  const W = WEIGHTS;
  if (!m.shownT) { m.shownT = tokenize(m.title); m.otherT = m.other ? tokenize(m.other) : null; }
  const titles = [[m.shownT, false], ...(m.otherT ? [[m.otherT, true]] : [])];
  const inTitles = titles.map(([T, other]) => qs.map((w) => findsIn(w, T, other)));
  const inNames = findsInLecturers(m, qs);
  const best = qs.map((w, k) => {
    const q = w.q;
    const cands = [];
    // the module's number and its abbreviations, whichever title
    if (isDigits(q) && q.length >= 3 && m.id === q) cands.push({ kind: "id", points: W.number, spans: [] });
    else if (isDigits(q) && q.length >= 3 && m.id.startsWith(q)) cands.push({ kind: "idstart", points: W.numberStart, spans: [] });
    if (q.length >= 2) {
      const exact = m.abbrevs.filter((a) => a.folded === q);
      const part = m.abbrevs.filter((a) => isLetters(q) && a.folded.startsWith(q) && isDigits(a.folded.slice(q.length)));
      const typed = (list, prefix) => /\p{Lu}/u.test(w.raw) && list.some((a) => (prefix ? a.raw.startsWith(w.raw) : a.raw === w.raw));
      if (exact.length) cands.push({ kind: "abbrev", points: W.abbrev * (typed(exact, false) ? W.caseMatch : 1), spans: [], abbrev: exact[0].raw, caseMatch: typed(exact, false) });
      else if (part.length) cands.push({ kind: "abbrevpart", points: W.abbrevPart * (typed(part, true) ? W.caseMatch : 1), spans: [], abbrev: part.map((a) => a.raw).join(", "), caseMatch: typed(part, true) });
    }
    for (const list of inTitles) cands.push(...list[k]);
    if (inNames[k]) cands.push(inNames[k]);
    cands.sort((a, b) => b.points - a.points || (a.to - a.from) - (b.to - b.from));
    return cands[0] ?? null;
  });
  const found = best.filter(Boolean).length;
  if (!found) return { found: 0 };

  // the whole module named: its number, its abbreviation, or every word of one of its titles
  let named = 0, namedBy = null;
  if (qs.length === 1 && ["id", "abbrev", "abbrevpart"].includes(best[0].kind)) { named = best[0].points; namedBy = best[0].kind; }
  titles.forEach(([T, other], i) => {
    if (found < qs.length) return;
    const c = covers(qs, T, inTitles[i]);
    if (c === null) return;
    const points = W.title * (other ? W.other : 1) * (c ? W.caseMatch : 1);
    if (points > named) { named = points; namedBy = other ? "other" : "shown"; }
  });

  // else the weakest word, more where the words stand side by side in their order
  const hits = best.filter(Boolean);
  let phrase = false;
  if (qs.length > 1 && hits.length === qs.length && hits.every((b) => b.from !== undefined && b.other === hits[0].other)) {
    const T = hits[0].other ? m.otherT : m.shownT;
    phrase = true;
    for (let k = 1; k < hits.length && phrase; k++) {
      let t = hits[k - 1].to + 1;
      while (t < T.toks.length && T.toks[t].filler) t++;
      if (hits[k].from !== t) phrase = false;
    }
  }
  const weakest = Math.min(...hits.map((b) => b.points)) * (phrase ? W.phrase : 1);
  const score = Math.max(named, weakest);
  const covered = new Set();
  for (const b of hits) if (b.from !== undefined && !b.other) for (let i = b.from; i <= b.to; i++) covered.add(i);
  const rest = namedBy && namedBy !== "other" && namedBy !== "shown" ? 0 : m.shownT.toks.filter((t, i) => t.content && !covered.has(i)).length;
  return { found, score, named: named >= weakest ? namedBy : null, phrase, rest, best };
}

/** A title's place in a series: what stands before its first number, and the number. */
function series(m) {
  if (m.series !== undefined) return m.series;
  const T = m.shownT ?? tokenize(m.title);
  const i = T.toks.findIndex((t) => isDigits(t.text) || ROMAN.includes(t.text));
  m.series = i > 0 ? { base: T.toks.slice(0, i).map((t) => t.text).join(" "),
    n: isDigits(T.toks[i].text) ? Number(T.toks[i].text) : ROMAN.indexOf(T.toks[i].text) + 1 } : null;
  return m.series;
}

/** Does a word of some title start with `q`? (The vocabulary is sorted.) */
function startsSomeWord(index, q) {
  const v = index.vocabulary;
  let lo = 0, hi = v.length;
  while (lo < hi) { const mid = (lo + hi) >> 1; if (v[mid][0] < q) lo = mid + 1; else hi = mid; }
  return lo < v.length && v[lo][0].startsWith(q);
}

/** The words of the query. A word is read as a short form too (strong) when it has one or two
 *  letters, capitals inside or only capitals (TI, AuD, TheoInf, LinA), or when no word of a title
 *  starts with it (theoinf); else as a word or its start only („mathe“ is no Math·E…). */
export function queryParts(index, text, resolution) {
  const out = [];
  const seen = new Set();
  for (const raw of text.split(NOT_WORD).filter(Boolean)) {
    const q = fold(raw);
    if (q && !seen.has(q)) { seen.add(q); out.push({ q, raw }); }
  }
  const content = out.filter((w) => !FILLERS.has(w.q));
  return (content.length ? content : out).map((w) => {
    const c = resolution?.corrected.find((c) => c.typed === w.q);
    const q = c ? c.word : w.q;
    const raw = c ? c.shown : w.raw;
    const capitals = !c && /\p{Lu}/u.test(w.raw.slice(1));
    const strong = q.length <= 2 || capitals || (!c && !startsSomeWord(index, q));
    return { q, raw, strong };
  });
}

const round = (x) => Math.round(x * 1e4) / 1e4;

function newRun(index, text, resolution, program) {
  const qs = queryParts(index, text, resolution);
  if (!qs.length) return [];
  const out = [];
  for (const m of index.modules) {
    const e = evaluate(m, qs);
    // Whether a module is found depends on the query alone; my program changes the order only.
    if (resolution?.mostWords ? e.found > 0 : e.found === qs.length) out.push({ m, ...e, why: e.best });
  }
  const number = qs.length === 1 && isDigits(qs[0].q);
  // The parts of a series stand together, in their order („Elektrotechnik 1“, „2“): what decides
  // after the points and the offer is the series' best (its fewest words left, its most programs).
  const groups = new Map();
  for (const r of out) {
    r.mine = program >= 0 && r.m.progs.has(program);
    r.points = round(r.score * (r.mine ? WEIGHTS.program : 1));
    const s = series(r.m);
    if (!s) continue;
    const g = groups.get(s.base) ?? { rest: Infinity, pop: -1 };
    groups.set(s.base, { rest: Math.min(g.rest, r.rest), pop: Math.max(g.pop, r.m.pop) });
  }
  for (const r of out) {
    const s = series(r.m), g = s && groups.get(s.base);
    r.tie = [g ? g.rest : r.rest, number ? Number(r.m.id) : -(g ? g.pop : r.m.pop), s ? s.base : fold(r.m.title), s ? s.n : 0];
  }
  // the words found, the points; then offered before phase-out, the fewer words left over, the more
  // programs (numbers: in their order), a series in its order, A–Z
  const cmp = (x, y) => (x < y ? -1 : x > y ? 1 : 0);
  out.sort((a, b) => b.found - a.found || b.points - a.points || a.m.status - b.m.status
    || a.tie[0] - b.tie[0] || a.tie[1] - b.tie[1] || cmp(a.tie[2], b.tie[2]) || a.tie[3] - b.tie[3]
    || compareTitle(a.m.title, b.m.title) || (a.m.id < b.m.id ? -1 : 1));
  return out;
}

export function searchNew(index, text, opts = {}) {
  const program = opts.program ?? -1;
  const run = (t, r) => newRun(index, t, r, program);
  const resolution = resolveWith(index, text, run, FILLERS);
  return { resolution, rows: run(text, resolution), parts: queryParts(index, text, resolution) };
}

/** The level a module's points stand for (for the page's pills). */
export function levelOf(points) {
  for (const [min, name] of LEVELS) if (points >= min) return name;
  return LEVELS[LEVELS.length - 1][1];
}
