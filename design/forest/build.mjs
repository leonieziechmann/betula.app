// The birch wood behind the start page, as a page to compare its directions (owner, 2026-09-29:
// a Scherenschnitt of a birch wood in the room beside the column instead of the branches):
//   node design/forest/build.mjs   → design/forest/forest.html (not checked in)
// forest.src.html draws a start page at its real size and the wood behind it (wood.mjs, the same
// wood forest.mjs draws for the site); the crowns of the seasons come from app/assets/birch and are
// written into the page as data URIs.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
let html = readFileSync(here("forest.src.html"), "utf8");
// The page's script is no module: the wood goes in as plain functions.
const wood = readFileSync(here("wood.mjs"), "utf8").replace(/^export /gm, "");
html = html.replace("%%WOOD%%", () => wood);
for (const season of ["spring", "summer", "autumn", "winter"]) {
  const svg = readFileSync(here(`../../app/assets/birch/${season}-crown.svg`), "utf8");
  const uri = "data:image/svg+xml," + svg.replace(/"/g, "'").replace(/[\r\n]+/g, " ").replace(/[%#<>{}|\\^`]/g, (c) => "%" + c.charCodeAt(0).toString(16).toUpperCase());
  html = html.replaceAll(`%%CROWN_${season}%%`, uri);
}
writeFileSync(here("forest.html"), `<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"></head><body style="margin:0">${html}</body></html>`);
