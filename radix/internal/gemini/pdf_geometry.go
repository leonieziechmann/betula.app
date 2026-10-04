package gemini

import (
	"context"
	"fmt"
	"math"
	"sort"
	"strings"

	"github.com/ledongthuc/pdf"
)

// All table coordinates use PDF points, measured from the top left. Keeping
// empty cells and missing internal borders is essential to semester assignment.
const geometryTolerance = 3.0

type pdfPoint struct{ x, y float64 }
type pdfBox struct {
	x0, y0, x1, y1 float64
	// bold: the text in the cell is set in a bold face; shaded: the cell is
	// painted with a grey fill (both are only meaningful for table cells).
	bold, shaded bool
}
type pdfEdge struct {
	horizontal           bool
	position, start, end float64
}
type pdfGlyph struct {
	x, y, width float64
	text        string
	rotation    int
	fontSize    float64
	bold        bool
}
type pdfTable struct {
	boxes [][]*pdfBox
	rows  [][]*string
	// origins is parallel to rows and only set for rows joined from a later
	// page (see mergeContinuedTables); nil means every row is on its own page.
	origins []rowOrigin
}

// rowOrigin says where a joined row really is: its page and the vertical
// offsets that were subtracted from its boxes to place it below the earlier rows.
type rowOrigin struct {
	page   int
	dx, dy float64
}

func (t pdfTable) origin(ri int) rowOrigin {
	if ri < len(t.origins) {
		return t.origins[ri]
	}
	return rowOrigin{}
}

func (t pdfTable) paddedOrigins() []rowOrigin {
	out := make([]rowOrigin, len(t.rows))
	copy(out, t.origins)
	return out
}

type pdfPageGeometry struct {
	shades []pdfBox
	edges  []pdfEdge
	glyphs []pdfGlyph
}
type pdfMatrix [6]float64

type pdfTextState struct {
	bold                                             bool
	font                                             *pdf.Font
	size, spacing, wordSpacing, scale, leading, rise float64
	matrix, line                                     pdfMatrix
}

// Font widths use character codes (or CIDs), never UTF-8 byte offsets.
func pdfCharacterWidth(font *pdf.Font, code int) float64 {
	if font.V.Key("Subtype").Name() != "Type0" {
		if font.V.Key("Widths").IsNull() {
			panic("font without explicit glyph widths requires manual review")
		}
		return font.Width(code)
	}
	desc := font.V.Key("DescendantFonts").Index(0)
	w := desc.Key("W")
	for i := 0; i < w.Len(); {
		first := int(w.Index(i).Int64())
		next := w.Index(i + 1)
		i += 2
		if next.Kind() == pdf.Array {
			if code >= first && code < first+next.Len() {
				return next.Index(code - first).Float64()
			}
		} else {
			last := int(next.Int64())
			width := w.Index(i).Float64()
			i++
			if code >= first && code <= last {
				return width
			}
		}
	}
	if dw := desc.Key("DW"); !dw.IsNull() {
		return dw.Float64()
	}
	return 1000
}

var identityPDFMatrix = pdfMatrix{1, 0, 0, 1, 0, 0}

func (m pdfMatrix) point(p pdfPoint) pdfPoint {
	return pdfPoint{m[0]*p.x + m[2]*p.y + m[4], m[1]*p.x + m[3]*p.y + m[5]}
}

func (m pdfMatrix) concat(n pdfMatrix) pdfMatrix {
	o := m.point(pdfPoint{n[4], n[5]})
	return pdfMatrix{m[0]*n[0] + m[2]*n[1], m[1]*n[0] + m[3]*n[1], m[0]*n[2] + m[2]*n[3], m[1]*n[2] + m[3]*n[3], o.x, o.y}
}

func inheritedPDFValue(p pdf.Value, key string) pdf.Value {
	for i := 0; i < 100 && !p.IsNull(); i++ {
		if v := p.Key(key); !v.IsNull() {
			return v
		}
		p = p.Key("Parent")
	}
	return pdf.Value{}
}

func readPageGeometry(ctx context.Context, page pdf.Page) (result pdfPageGeometry, err error) {
	defer func() {
		if r := recover(); r != nil {
			result = pdfPageGeometry{}
			if cause, ok := r.(error); ok {
				err = fmt.Errorf("unsupported or malformed PDF content: %w", cause)
			} else {
				err = fmt.Errorf("unsupported or malformed PDF content: %v", r)
			}
		}
	}()
	if err := ctx.Err(); err != nil {
		return result, err
	}
	if page.V.Key("Contents").IsNull() {
		return result, nil
	}
	media := inheritedPDFValue(page.V, "MediaBox")
	if media.Len() != 4 {
		return result, fmt.Errorf("missing page MediaBox")
	}
	x0, y0, x1, y1 := media.Index(0).Float64(), media.Index(1).Float64(), media.Index(2).Float64(), media.Index(3).Float64()
	rotation := ((inheritedPDFValue(page.V, "Rotate").Int64() % 360) + 360) % 360
	toPage := func(p pdfPoint) pdfPoint {
		switch rotation {
		case 0:
			return pdfPoint{p.x - x0, y1 - p.y}
		case 90:
			return pdfPoint{p.y - y0, p.x - x0}
		case 180:
			return pdfPoint{x1 - p.x, p.y - y0}
		case 270:
			return pdfPoint{y1 - p.y, x1 - p.x}
		default:
			panic("unsupported page rotation")
		}
	}
	if err := ctx.Err(); err != nil {
		return result, err
	}
	ctm := identityPDFMatrix
	var stack []pdfMatrix
	ts := pdfTextState{scale: 1, matrix: identityPDFMatrix, line: identityPDFMatrix}
	var textStack []pdfTextState
	fonts := map[string]*pdf.Font{}
	translate := func(x, y float64) pdfMatrix { return pdfMatrix{1, 0, 0, 1, x, y} }
	nextLine := func() { ts.line = ts.line.concat(translate(0, -ts.leading)); ts.matrix = ts.line }
	showText := func(raw string) {
		if ts.font == nil {
			panic("text without a font")
		}
		step := 1
		if ts.font.V.Key("Subtype").Name() == "Type0" {
			if ts.font.V.Key("Encoding").Name() != "Identity-H" {
				panic("unsupported composite font encoding")
			}
			step = 2
		}
		if len(raw)%step != 0 {
			panic("truncated font character code")
		}
		encoder := ts.font.Encoder()
		if encoder == nil {
			panic("missing font character map")
		}
		for i := 0; i < len(raw); i += step {
			if i%1024 == 0 {
				if err := ctx.Err(); err != nil {
					panic(err)
				}
			}
			if len(result.glyphs) >= 1000000 {
				panic("PDF text complexity limit exceeded")
			}
			code := int(raw[i])
			if step == 2 {
				code = code<<8 | int(raw[i+1])
			}
			s := encoder.Decode(raw[i : i+step])
			if s == "" {
				panic("undecodable font character")
			}
			width := pdfCharacterWidth(ts.font, code) / 1000 * ts.size * ts.scale
			m := ctm.concat(ts.matrix)
			a := toPage(m.point(pdfPoint{0, ts.rise + ts.size*.35}))
			b := toPage(m.point(pdfPoint{width, ts.rise + ts.size*.35}))
			dx, dy := b.x-a.x, b.y-a.y
			orientation := 0
			if math.Abs(dy) > .1 {
				if math.Abs(dx) > .1 {
					panic("oblique text requires manual review")
				}
				orientation = 90
				if dy < 0 {
					orientation = 270
				}
			} else if dx < -.1 {
				orientation = 180
			}
			result.glyphs = append(result.glyphs, pdfGlyph{a.x, a.y, math.Hypot(dx, dy), s, orientation, ts.size, ts.bold})
			advance := width + ts.spacing*ts.scale
			if step == 1 && code == 32 {
				advance += ts.wordSpacing * ts.scale
			}
			ts.matrix = ts.matrix.concat(translate(advance, 0))
		}
	}
	fill := 0.0 // luminance of the current non-stroking color: 0 black, 1 white
	var fillStack []float64
	type segment struct{ a, b pdfPoint }
	var path []segment
	var current, origin pdfPoint
	hasCurrent := false
	line := func(p pdfPoint) {
		if hasCurrent {
			path = append(path, segment{current, p})
		}
		current = p
		hasCurrent = true
	}
	paint := func() {
		for _, s := range path {
			a, b := toPage(s.a), toPage(s.b)
			if math.Abs(a.y-b.y) < 0.1 && math.Abs(a.x-b.x) > 0.1 {
				result.edges = append(result.edges, pdfEdge{true, (a.y + b.y) / 2, math.Min(a.x, b.x), math.Max(a.x, b.x)})
			} else if math.Abs(a.x-b.x) < 0.1 && math.Abs(a.y-b.y) > 0.1 {
				result.edges = append(result.edges, pdfEdge{false, (a.x + b.x) / 2, math.Min(a.y, b.y), math.Max(a.y, b.y)})
			}
		}
	}
	// A grey rectangle behind text marks a cell (the "possible study plan" in
	// some regulations); black fills are only used as thin rules.
	shade := func() {
		if fill <= .05 || fill >= .98 || len(path) < 3 {
			return
		}
		x0, y0, x1, y1 := math.Inf(1), math.Inf(1), math.Inf(-1), math.Inf(-1)
		for _, s := range path {
			for _, p := range []pdfPoint{toPage(s.a), toPage(s.b)} {
				x0, y0, x1, y1 = math.Min(x0, p.x), math.Min(y0, p.y), math.Max(x1, p.x), math.Max(y1, p.y)
			}
		}
		if x1-x0 > 3 && y1-y0 > 3 && len(result.shades) < 20000 {
			result.shades = append(result.shades, pdfBox{x0, y0, x1, y1, false, false})
		}
	}
	ops := 0
	resources := page.Resources()
	depth := 0
	var interpret func(*pdf.Stack, string)
	interpret = func(stk *pdf.Stack, op string) {
		ops++
		if ops%1024 == 0 {
			if err := ctx.Err(); err != nil {
				panic(err)
			}
		}
		if ops > 2000000 || len(result.edges) > 20000 {
			panic("PDF page complexity limit exceeded")
		}
		a := make([]pdf.Value, stk.Len())
		for i := len(a) - 1; i >= 0; i-- {
			a[i] = stk.Pop()
		}
		point := func(i int) pdfPoint { return ctm.point(pdfPoint{a[i].Float64(), a[i+1].Float64()}) }
		switch op {
		case "q":
			stack = append(stack, ctm)
			fillStack = append(fillStack, fill)
			textStack = append(textStack, ts)
		case "Q":
			if len(stack) == 0 {
				panic("unbalanced graphics state")
			}
			ctm = stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			fill = fillStack[len(fillStack)-1]
			fillStack = fillStack[:len(fillStack)-1]
			// Text matrices belong to the text object, not the saved graphics state.
			m, l := ts.matrix, ts.line
			ts = textStack[len(textStack)-1]
			textStack = textStack[:len(textStack)-1]
			ts.matrix, ts.line = m, l
		case "cm":
			ctm = ctm.concat(pdfMatrix{a[0].Float64(), a[1].Float64(), a[2].Float64(), a[3].Float64(), a[4].Float64(), a[5].Float64()})
		case "BT":
			ts.matrix = identityPDFMatrix
			ts.line = identityPDFMatrix
		case "Tf":
			name := a[0].Name()
			font := fonts[name]
			if font == nil {
				f := pdf.Font{V: resources.Key("Font").Key(name)}
				font = &f
				fonts[name] = font
			}
			ts.font = font
			ts.size = a[1].Float64()
			ts.bold = strings.Contains(strings.ToLower(font.V.Key("BaseFont").Name()), "bold")
		case "Tm":
			ts.matrix = pdfMatrix{a[0].Float64(), a[1].Float64(), a[2].Float64(), a[3].Float64(), a[4].Float64(), a[5].Float64()}
			ts.line = ts.matrix
		case "Td", "TD":
			if op == "TD" {
				ts.leading = -a[1].Float64()
			}
			ts.line = ts.line.concat(translate(a[0].Float64(), a[1].Float64()))
			ts.matrix = ts.line
		case "T*":
			nextLine()
		case "Tc":
			ts.spacing = a[0].Float64()
		case "Tw":
			ts.wordSpacing = a[0].Float64()
		case "Tz":
			ts.scale = a[0].Float64() / 100
		case "TL":
			ts.leading = a[0].Float64()
		case "Ts":
			ts.rise = a[0].Float64()
		case "Tj":
			showText(a[0].RawString())
		case "'":
			nextLine()
			showText(a[0].RawString())
		case "\"":
			ts.wordSpacing = a[0].Float64()
			ts.spacing = a[1].Float64()
			nextLine()
			showText(a[2].RawString())
		case "TJ":
			v := a[0]
			for i := 0; i < v.Len(); i++ {
				item := v.Index(i)
				if item.Kind() == pdf.String {
					showText(item.RawString())
				} else {
					ts.matrix = ts.matrix.concat(translate(-item.Float64()/1000*ts.size*ts.scale, 0))
				}
			}
		case "gs":
			if !resources.Key("ExtGState").Key(a[0].Name()).Key("Font").IsNull() {
				panic("external graphics-state fonts require manual review")
			}
		case "m":
			current = point(0)
			origin = current
			hasCurrent = true
		case "l":
			line(point(0))
		case "h":
			line(origin)
		case "re":
			x, y, w, h := a[0].Float64(), a[1].Float64(), a[2].Float64(), a[3].Float64()
			current = ctm.point(pdfPoint{x, y})
			origin = current
			hasCurrent = true
			line(ctm.point(pdfPoint{x + w, y}))
			line(ctm.point(pdfPoint{x + w, y + h}))
			line(ctm.point(pdfPoint{x, y + h}))
			line(origin)
		case "c":
			current = point(4)
		case "v", "y":
			current = point(2)
		case "g":
			if len(a) == 1 {
				fill = a[0].Float64()
			}
		case "rg":
			if len(a) == 3 {
				fill = .299*a[0].Float64() + .587*a[1].Float64() + .114*a[2].Float64()
			}
		case "k":
			if len(a) == 4 {
				fill = 1 - math.Min(1, .3*a[0].Float64()+.59*a[1].Float64()+.11*a[2].Float64()+a[3].Float64())
			}
		case "sc", "scn":
			switch {
			case len(a) == 1 && a[0].Kind() != pdf.Name:
				fill = a[0].Float64()
			case len(a) == 3 && a[2].Kind() != pdf.Name:
				fill = .299*a[0].Float64() + .587*a[1].Float64() + .114*a[2].Float64()
			}
		case "s", "b", "b*":
			line(origin)
			shade()
			paint()
			path = nil
			hasCurrent = false
		case "f", "F", "f*", "B", "B*":
			shade()
			paint()
			path = nil
			hasCurrent = false
		case "S":
			paint()
			path = nil
			hasCurrent = false
		case "n":
			path = nil
			hasCurrent = false // A clipping-only rectangle is not a table border.
		case "Do":
			form := resources.Key("XObject").Key(a[0].Name())
			if form.Key("Subtype").Name() == "Form" {
				depth++
				if depth > 16 {
					panic("Form XObjects recursion limit exceeded")
				}
				savedCTM, savedTS, savedResources, savedFonts := ctm, ts, resources, fonts
				savedStack, savedTextStack := stack, textStack
				savedPath, savedCurrent, savedOrigin, savedHasCurrent := path, current, origin, hasCurrent
				matrix := form.Key("Matrix")
				if matrix.Len() == 6 {
					ctm = ctm.concat(pdfMatrix{matrix.Index(0).Float64(), matrix.Index(1).Float64(), matrix.Index(2).Float64(), matrix.Index(3).Float64(), matrix.Index(4).Float64(), matrix.Index(5).Float64()})
				}
				if !form.Key("Resources").IsNull() {
					resources = form.Key("Resources")
				}
				fonts = map[string]*pdf.Font{}
				stack = nil
				textStack = nil
				path = nil
				hasCurrent = false
				pdf.Interpret(form, interpret)
				if len(stack) > 0 {
					panic("unbalanced Form XObjects graphics state")
				}
				ctm, ts, resources, fonts = savedCTM, savedTS, savedResources, savedFonts
				stack, textStack = savedStack, savedTextStack
				path, current, origin, hasCurrent = savedPath, savedCurrent, savedOrigin, savedHasCurrent
				depth--
			}
		}
	}
	pdf.Interpret(page.V.Key("Contents"), interpret)
	return result, ctx.Err()
}

func snapEdges(edges []pdfEdge) []pdfEdge {
	sort.Slice(edges, func(i, j int) bool {
		if edges[i].horizontal != edges[j].horizontal {
			return edges[i].horizontal
		}
		return edges[i].position < edges[j].position
	})
	for i := 0; i < len(edges); {
		j := i + 1
		sum := edges[i].position
		for j < len(edges) && edges[j].horizontal == edges[i].horizontal && edges[j].position-edges[j-1].position <= geometryTolerance {
			sum += edges[j].position
			j++
		}
		for k := i; k < j; k++ {
			edges[k].position = sum / float64(j-i)
		}
		i = j
	}
	sort.Slice(edges, func(i, j int) bool {
		a, b := edges[i], edges[j]
		if a.horizontal != b.horizontal {
			return a.horizontal
		}
		if a.position != b.position {
			return a.position < b.position
		}
		return a.start < b.start
	})
	joined := []pdfEdge{}
	for _, e := range edges {
		if len(joined) > 0 {
			p := &joined[len(joined)-1]
			if e.horizontal == p.horizontal && e.position == p.position && e.start <= p.end+geometryTolerance {
				p.end = math.Max(p.end, e.end)
				continue
			}
		}
		joined = append(joined, e)
	}
	out := joined[:0]
	for _, e := range joined {
		if e.end-e.start >= geometryTolerance {
			out = append(out, e)
		}
	}
	return out
}

func tableGeometry(ctx context.Context, edges []pdfEdge, glyphs []pdfGlyph) ([]pdfTable, error) {
	edges = snapEdges(edges)
	var hs, vs []pdfEdge
	for _, e := range edges {
		if e.horizontal {
			hs = append(hs, e)
		} else {
			vs = append(vs, e)
		}
	}
	if len(hs)*len(vs) > 2000000 {
		return nil, fmt.Errorf("PDF table grid complexity limit exceeded")
	}
	type crossing struct{ h, v int }
	points := map[pdfPoint]crossing{}
	for hi, h := range hs {
		for vi, v := range vs {
			if v.position >= h.start-3 && v.position <= h.end+3 && h.position >= v.start-3 && h.position <= v.end+3 {
				points[pdfPoint{v.position, h.position}] = crossing{hi, vi}
			}
		}
	}
	ordered := make([]pdfPoint, 0, len(points))
	for p := range points {
		ordered = append(ordered, p)
	}
	sort.Slice(ordered, func(i, j int) bool {
		if ordered[i].y != ordered[j].y {
			return ordered[i].y < ordered[j].y
		}
		return ordered[i].x < ordered[j].x
	})
	byX, byY := map[float64][]pdfPoint{}, map[float64][]pdfPoint{}
	for _, p := range ordered {
		byX[p.x] = append(byX[p.x], p)
		byY[p.y] = append(byY[p.y], p)
	}
	var cells []pdfBox
	for _, p := range ordered {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		a := points[p]
		found := false
		for _, below := range byX[p.x] {
			if below.y <= p.y || points[below].v != a.v {
				continue
			}
			for _, right := range byY[p.y] {
				if right.x <= p.x || points[right].h != a.h {
					continue
				}
				br, ok := points[pdfPoint{right.x, below.y}]
				if ok && br.h == points[below].h && br.v == points[right].v {
					cells = append(cells, pdfBox{p.x, p.y, right.x, below.y, false, false})
					found = true
					break
				}
			}
			if found {
				break
			}
		}
	}
	// Connected cells form a table; disconnected ruling elsewhere stays separate.
	parent := make([]int, len(cells))
	for i := range parent {
		parent[i] = i
	}
	var root func(int) int
	root = func(i int) int {
		if parent[i] != i {
			parent[i] = root(parent[i])
		}
		return parent[i]
	}
	corners := map[pdfPoint]int{}
	for i, c := range cells {
		for _, p := range []pdfPoint{{c.x0, c.y0}, {c.x1, c.y0}, {c.x0, c.y1}, {c.x1, c.y1}} {
			if j, ok := corners[p]; ok {
				parent[root(i)] = root(j)
			} else {
				corners[p] = i
			}
		}
	}
	groups := map[int][]pdfBox{}
	var order []int
	for i, c := range cells {
		r := root(i)
		if _, ok := groups[r]; !ok {
			order = append(order, r)
		}
		groups[r] = append(groups[r], c)
	}
	var tables []pdfTable
	for _, r := range order {
		group := groups[r]
		if len(group) < 2 {
			continue
		}
		xset, yset := map[float64]bool{}, map[float64]bool{}
		for _, c := range group {
			xset[c.x0] = true
			yset[c.y0] = true
		}
		xs, ys := sortedCoordinates(xset), sortedCoordinates(yset)
		t := pdfTable{boxes: make([][]*pdfBox, len(ys)), rows: make([][]*string, len(ys))}
		for i := range ys {
			t.boxes[i] = make([]*pdfBox, len(xs))
			t.rows[i] = make([]*string, len(xs))
		}
		for _, c := range group {
			ri, ci := sort.SearchFloat64s(ys, c.y0), sort.SearchFloat64s(xs, c.x0)
			b := c
			s := textInBox(glyphs, &c)
			t.boxes[ri][ci] = &b
			t.rows[ri][ci] = &s
		}
		tables = append(tables, t)
	}
	return tables, nil
}

func sortedCoordinates(set map[float64]bool) []float64 {
	v := make([]float64, 0, len(set))
	for x := range set {
		v = append(v, x)
	}
	sort.Float64s(v)
	return v
}

func textInBox(glyphs []pdfGlyph, box *pdfBox) string {
	// Read each writing direction in its own coordinate system. Selection stays
	// in page coordinates so vertical labels cannot leak into adjacent cells.
	var groups [4][]pdfGlyph
	for _, g := range glyphs {
		x, y := g.x, g.y
		switch g.rotation {
		case 0:
			x += g.width / 2
		case 90:
			y += g.width / 2
		case 180:
			x -= g.width / 2
		case 270:
			y -= g.width / 2
		}
		if box != nil && !(x >= box.x0 && x < box.x1 && y >= box.y0 && y < box.y1) {
			continue
		}
		index := g.rotation / 90
		switch g.rotation {
		case 90:
			g.x, g.y = g.y, -g.x
		case 180:
			g.x, g.y = -g.x, -g.y
		case 270:
			g.x, g.y = -g.y, g.x
		}
		groups[index] = append(groups[index], g)
	}
	var pieces []string
	for _, group := range groups {
		if s := horizontalText(group); s != "" {
			pieces = append(pieces, s)
		}
	}
	return strings.Join(pieces, "\n")
}

func horizontalText(glyphs []pdfGlyph) string {
	var chars []pdfGlyph
	for _, g := range glyphs {
		chars = append(chars, g)
	}
	sort.SliceStable(chars, func(i, j int) bool { return chars[i].y < chars[j].y })
	var result strings.Builder
	for i := 0; i < len(chars); {
		j := i + 1
		for j < len(chars) && chars[j].y-chars[i].y <= 3 {
			j++
		}
		line := chars[i:j]
		// The baseline is the row of the body text: the largest face, and among
		// equal faces the widest glyph. Generators that report one font size for
		// the whole page would otherwise put the baseline on a footnote marker.
		maxSize, maxWidth, baseline := 0.0, 0.0, 0.0
		baseSize, baseWidth := -1.0, -1.0
		for _, g := range line {
			if strings.TrimSpace(g.text) == "" {
				continue
			}
			if g.fontSize > maxSize {
				maxSize = g.fontSize
			}
			if g.width > maxWidth {
				maxWidth = g.width
			}
			if g.fontSize > baseSize || (g.fontSize == baseSize && g.width > baseWidth) {
				baseSize, baseWidth, baseline = g.fontSize, g.width, g.y
			}
		}
		sort.SliceStable(line, func(a, b int) bool { return line[a].x < line[b].x })
		var lineText strings.Builder
		last := math.Inf(-1)
		var previous *pdfGlyph
		for k, g := range line {
			// Some generators fake a bold face by drawing the same glyph twice with
			// a hairline offset. Printing it twice would turn a 6 LP cell into 66.
			if previous != nil && previous.text == g.text && math.Abs(previous.x-g.x) < 0.5 && math.Abs(previous.y-g.y) < 0.5 {
				continue
			}
			previous = &line[k]
			if k > 0 && g.x-last > 3 {
				lineText.WriteByte(' ')
			}
			// A footnote reference is set smaller and off the baseline. Some
			// generators report one font size for the whole page, so the glyph
			// advance is the only remaining evidence of the smaller face.
			smaller := g.fontSize < maxSize*.8 || (maxWidth > 0 && g.width < maxWidth*.8)
			if smaller && g.y < baseline-.5 && len(g.text) == 1 && g.text[0] >= '0' && g.text[0] <= '9' {
				lineText.WriteRune([]rune("⁰¹²³⁴⁵⁶⁷⁸⁹")[g.text[0]-'0'])
			} else {
				lineText.WriteString(g.text)
			}
			last = g.x + g.width
		}
		if text := strings.TrimSpace(lineText.String()); text != "" {
			if result.Len() > 0 {
				result.WriteByte('\n')
			}
			result.WriteString(text)
		}
		i = j
	}
	return result.String()
}
