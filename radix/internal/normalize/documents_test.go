package normalize

import "testing"

// The issues of the documents Folia names, as the BTU's page of Grundsatzordnungen linked them on
// 2026-11-10, with the titles it wrote: the number at the start of the file's name, the year after
// it, on the old OPUS the year it filed it under, else the year of the title's date. A wrong date
// in the title (17/2022 „vom 12. September 2016") does not win over the file's name.
func TestGazetteIssue(t *testing.T) {
	for _, c := range []struct{ url, title, want string }{
		{"https://opus4.kobv.de/opus4-btu/files/3983/AMbl-13_2016-RahmenO-BA.pdf", "Allgemeine Prüfungs- und Studienordnung für Bachelor-Studiengänge vom 12. September 2016", "13/2016"},
		{"https://opus4.kobv.de/opus4-btu/files/5396/AMbl-1_2021_RahmenO_BA_1.AeS.pdf", "Erste Änderungssatzung … vom 26. Januar 2021", "01/2021"},
		{"https://opus4.kobv.de/opus4-btu/files/6092/AMbl-17_2022_2.AeS_RahmenO_BA.pdf", "Zweite Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom 12. September 2016", "17/2022"},
		{"https://opus4.kobv.de/opus4-btu/files/6349/06_3.AES_RahmenO_BA.pdf", "Dritte Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge vom am 25. April 2023", "06/2023"},
		{"https://opus4.kobv.de/opus4-btu/files/6709/14_4.AS_RahmenO_BA_.pdf", "Vierte Änderungssatzung … vom 24. Juni 2024", "14/2024"},
		{"https://opus4.kobv.de/opus4-btu/files/6772/29_RahmenO.pdf", "Fünfte Änderungssatzung … vom 29. August 2024", "29/2024"},
		{"https://opus4.kobv.de/opus4-btu/files/7446/AMbl-35_2026.pdf", "Sechste Änderungssatzung … vom 29. Juli 2026", "35/2026"},
		{"https://www-docs.b-tu.de/studierende/public/files/AMbl-14_2016-RahmenO-MA.pdf", "Allgemeine Prüfungs- und Studienordnung für Master-Studiengänge vom 12. September 2016", "14/2016"},
		{"https://opus4.kobv.de/opus4-btu/files/5397/AMbl-2_2021_RahmenO_MA_1.AeS.pdf", "Erste Änderungssatzung … vom 26. September.2021", "02/2021"},
		{"https://opus4.kobv.de/opus4-btu/files/6350/07_3.AES_RahmenO_MA.pdf", "Dritte Änderungssatzung … erschienen am 25. April 2023", "07/2023"},
		{"https://opus4.kobv.de/opus4-btu/files/6773/30_RahmenO.pdf", "Fünfte Änderungssatzung … vom 29. August 2024", "30/2024"},
		{"https://opus4.kobv.de/opus4-btu/files/7450/AMbl-38_2026.pdf", "Sixth Amendment Ordinance … of 15 September 2024", "38/2026"},
		{"https://opus4.kobv.de/opus4-btu/files/5109/AMbl-01_2020_ImmaO.pdf", "Immatrikulationsordnung der BTU Cottbus-Senftenberg vom 28. Januar 2020", "01/2020"},
		{"https://opus4.kobv.de/opus4-btu/files/6002/AMbl-10_2022_ImmaO_Aussetzung_Paragraph_16.pdf", "Aussetzung …", "10/2022"},
		{"https://opus4.kobv.de/opus4-btu/files/6827/AMbl-39_24.pdf", "Dritte Änderungssatzung der Grundordnung", "39/2024"},
		{"https://opus4.kobv.de/opus4-btu/files/4199/AMbl-11_2017-%C3%84S_WahlO_BTU.pdf", "Wahlordnung vom 17. November 2016", "11/2017"},
		{"https://www-docs.b-tu.de/studierende/public/files/Amtsblatt_36_2017_Promotion_RahmenO.pdf", "Rahmenordnung für Promotionsverfahren", "36/2017"},
		{"http://opus.kobv.de/btu/volltexte/2010/1748/pdf/04_FAS_Rili_230410.pdf", "Ordnung zum Fachübergreifenden Studium", "04/2010"},
		{"http://opus.kobv.de/btu/volltexte/2012/2425/pdf/Abl_12.pdf", "Richtlinien zur Ausfertigung von Abschlussdokumenten", "12/2012"},
		{"https://opus4.kobv.de/opus4-btu/files/6778/Ambl-34_BeitragsO_SoSe2.pdf", "Beitragsordnung … vom 15. Oktober 2024 für das Sommersemester 2025", "34/2024"},
		{"https://opus4.kobv.de/opus4-btu/files/7094/AMbl-34_2025.pdf", "", "34/2025"},
		{"https://opus4.kobv.de/opus4-btu/files/9999/12_Plan.pdf", "dated 29.08.2024", "12/2024"},
		// No issue: a translation on the BTU's own server, a number 0, no year anywhere, a page.
		{"https://www-docs.b-tu.de/studierende/public/rahmenordnungen/bachelor-en/29-RahmnO%20BA_5-Aenderungssatzung_EN.pdf", "dated 29.08.2024", ""},
		{"https://www-docs.b-tu.de/studierende/public/files/18-Satzung%20Fl%C3%BCchtlinge_27.09.16.pdf", "Satzung … vom 27. September 2016", ""},
		{"https://www-docs.b-tu.de/studierende/public/ImmaO/ImmaO_28.01.2020_englih.pdf", "Enrolment Regulations", ""},
		{"http://opus.kobv.de/btu/volltexte/2011/2275/pdf/00_RahmenO_PhD_111011.pdf", "Rahmenordnung für strukturierte Promotionsprogramme vom 08. Februar 2011", ""},
		{"https://opus4.kobv.de/opus4-btu/files/6490/23_BeitragO+StuPa_SoSe24_WS24_25.pdf", "Beitragsordnung … für das Sommersemester 2024 und Wintersemester 2024/2025", ""},
		{"https://bravors.brandenburg.de/gesetze/bbghg", "Brandenburgisches Hochschulgesetz vom 23. September 2020", ""},
	} {
		if got := GazetteIssue(c.url, c.title); got != c.want {
			t.Errorf("GazetteIssue(%s, %q) = %q, want %q", c.url, c.title, got, c.want)
		}
	}
}

func TestGeneralTopic(t *testing.T) {
	for _, c := range []struct{ section, title, want string }{
		{"Landesrechtliche Grundlagen", "Brandenburgisches Hochschulgesetz vom 23. September 2020", TopicHigherEducationAct},
		{"National legal basis", "Brandenburg Higher Education Act of 23 September 2020", TopicHigherEducationAct},
		{"Rahmenordnung für Bachelor-Studiengänge", "Erste Änderungssatzung der Rahmenordnung für Bachelor-Studiengänge", TopicFrameworkBachelor},
		{"Framework regulations for Bachelor´s degree programs", "General Examination and Study Regulations", TopicFrameworkBachelor},
		{"Rahmenordnung für Master-Studiengänge", "Allgemeine Prüfungs- und Studienordnung für Master-Studiengänge", TopicFrameworkMaster},
		{"Framework regulations for Master´s degree programs", "Sixth Amendment Ordinance", TopicFrameworkMaster},
		{"Immatrikulationsordnung", "Aussetzung der Anwendung des § 16 Abs. 2 und Abs. 3 der Immatrikulationsordnung", TopicEnrolment},
		{"Matriculation Regulations", "Enrolment Regulations of the BTU Cottbus-Senftenberg", TopicEnrolment},
		{"Sonstige Ordnungen und Richtlinien", "Ordnung zum Fachübergreifenden Studium", TopicFUES},
		{"Other regulations and guidelines", "Regulations for General Studies", TopicFUES},
		// The other documents of the page are about nothing Folia names.
		{"Rahmenordnung für strukturierte Promotionsprogramme", "Rahmenordnung für strukturierte Promotionsprogramme", ""},
		{"Framework regulations for doctoral procedures", "Framework regulations for doctoral procedures", ""},
		{"Sonstige Ordnungen und Richtlinien", "Rahmenordnung der ehemaligen Hochschule Lausitz für Bachelor- und Masterabschlüsse", ""},
		{"Ordnungen zur Studienvorbereitung", "Rahmenordnung für die Durchführung von Vorbereitungskursen", ""},
		{"Grundordnung", "Grundordnung für die Brandenburgische Technische Universität", ""},
	} {
		if got := GeneralTopic(c.section, c.title); got != c.want {
			t.Errorf("GeneralTopic(%q, %q) = %q, want %q", c.section, c.title, got, c.want)
		}
	}
	for fold, want := range map[string]string{"": "statute", "Änderungssatzungen": "amendment", "Amendment statutes": "amendment", "Lesefassungen": "other"} {
		if got := GeneralKind(fold); got != want {
			t.Errorf("GeneralKind(%q) = %q, want %q", fold, got, want)
		}
	}
}
