package gemini

// annotateCellStyles marks table cells whose text is bold and cells that lie on
// a grey fill. Legends such as "fett geschriebene LP-Zahlen sind Pflichtmodule;
// grau angelegte Zellen stellen einen moeglichen Studienplan dar" depend on it.
func annotateCellStyles(tables []pdfTable, g pdfPageGeometry) {
	for _, t := range tables {
		for _, row := range t.boxes {
			for _, b := range row {
				if b == nil {
					continue
				}
				bold, total := 0, 0
				for _, gl := range g.glyphs {
					if gl.rotation != 0 || gl.text == " " {
						continue
					}
					x, y := gl.x+gl.width/2, gl.y
					if x >= b.x0 && x < b.x1 && y >= b.y0 && y < b.y1 {
						total++
						if gl.bold {
							bold++
						}
					}
				}
				b.bold = total > 0 && bold*2 > total
				cx, cy := (b.x0+b.x1)/2, (b.y0+b.y1)/2
				for _, s := range g.shades {
					if cx >= s.x0 && cx <= s.x1 && cy >= s.y0 && cy <= s.y1 && s.x1-s.x0 >= (b.x1-b.x0)*.5 {
						b.shaded = true
						break
					}
				}
			}
		}
	}
}
