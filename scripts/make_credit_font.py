"""Generate the OFL-licensed embedded fullwidth credit font (fonttools needed)."""
from pathlib import Path
import sys, copy
from fontTools.ttLib import TTFont
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
source=Path(sys.argv[1]); out=Path(__file__).resolve().parents[1]/'plugins/spectral_compressor/src/editor/fonts'
out.mkdir(parents=True,exist_ok=True)
font=TTFont(source)
units=font['head'].unitsPerEm
mapping=font.getBestCmap()
for code in range(0xFF01,0xFF5F):
    original=mapping.get(code-0xFEE0)
    if original is None: continue
    name=f'fullwidth{code:04X}'
    advance,lsb=font['hmtx'][original]
    shift=(units-advance)/2
    glyph_set=font.getGlyphSet()
    pen=TTGlyphPen(glyph_set)
    glyph_set[original].draw(TransformPen(pen,(1,0,0,1,shift,0)))
    font['glyf'][name]=pen.glyph()
    font['hmtx'][name]=(units,round(lsb+shift))
    font.setGlyphOrder(font['glyf'].glyphOrder)
    for table in font['cmap'].tables:
        if table.isUnicode() and table.format in (4,12): table.cmap[code]=name
for record in font['name'].names:
    if record.nameID in (1,4,6,16):
        value='TurboCredit-Regular' if record.nameID==6 else 'Turbo Credit'
        record.string=value.encode(record.getEncoding())
font.save(out/'TurboCredit-Regular.ttf')
(out/'LICENSE').write_bytes((source.parent/'LICENSE').read_bytes())
(out/'README.md').write_text('Turbo Credit is a modified Noto Sans Regular font. Original font by Google, licensed under the SIL Open Font License 1.1 (LICENSE). Fullwidth ASCII glyphs are centered in one-em cells; the modified family is renamed Turbo Credit. Reproduce with scripts/make_credit_font.py and the pinned NIH-plug assets NotoSans-Regular.ttf. FontTools is a build-time helper; the font is embedded, with no user dependency.\n')
print(out/'TurboCredit-Regular.ttf')
