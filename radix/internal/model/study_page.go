package model

// StudyPage is a page of the BTU's site around the study, as its page „Studienorganisation" lists
// it (b-tu.de/studium/im-studium/studienorganisation, and its English page): the examinations, the
// semester's formalities, its services. The page puts them in groups, each an <h2> heading followed
// by its tiles; a tile is a title that links the page and a sentence about it.
type StudyPage struct {
	Section  string // the heading of its group („Prüfungen"), as the page writes it
	Title    string // the tile's title
	Abstract string // the sentence under it; "" where there is none
	URL      string // its address, absolute
}
