"""Static cuts of Inter for the link-preview cards the server draws (server/src/cards.rs).

    python design/cards/make-fonts.py        (needs fontTools and brotli)

The app ships Inter as one variable WOFF2 (app/assets/inter-latin.woff2). The renderer on the
server (resvg) reads plain TrueType, one file per weight, so the weights the cards use are cut
out of the variable font here. Output: server/assets/inter-{400,500,600,800}.ttf. The files keep the
family name "Inter"; the renderer tells them apart by their weight class.
"""
import pathlib

from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "app" / "assets" / "inter-latin.woff2"
OUT = ROOT / "server" / "assets"
OUT.mkdir(parents=True, exist_ok=True)

for weight in (400, 500, 600, 800):
    font = TTFont(SOURCE)
    axes = {axis.axisTag: axis.defaultValue for axis in font["fvar"].axes}
    axes["wght"] = weight
    static = instancer.instantiateVariableFont(font, axes)
    static.flavor = None  # plain TrueType, not WOFF2
    static["OS/2"].usWeightClass = weight
    target = OUT / f"inter-{weight}.ttf"
    static.save(target)
    print(target.relative_to(ROOT), target.stat().st_size)
