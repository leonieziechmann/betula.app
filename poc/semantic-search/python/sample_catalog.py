"""A made-up catalog of 30 modules with the columns of `v_module` that `embed_catalog.py` reads,
so the demo runs without a Betula snapshot. None of it is BTU data.

    python sample_catalog.py sample.db
"""

import sqlite3
import sys

MODULES = [
    ("Einführung in die Programmierung", "Introduction to Programming", "Variablen, Kontrollstrukturen, Funktionen und Klassen in Python; erste eigene Programme."),
    ("Algorithmen und Datenstrukturen", "Algorithms and Data Structures", "Sortieren, Suchen, Bäume, Graphen, Laufzeitanalyse."),
    ("Datenbanksysteme", "Database Systems", "Relationales Modell, SQL, Normalformen, Transaktionen und Indexstrukturen."),
    ("Maschinelles Lernen", "Machine Learning", "Regression, Klassifikation, neuronale Netze, Überanpassung und Validierung."),
    ("Rechnernetze", "Computer Networks", "Schichtenmodell, TCP/IP, Routing, Sicherheit im Netz."),
    ("IT-Sicherheit", "IT Security", "Kryptographie, Authentifizierung, Angriffe auf Webanwendungen und ihre Abwehr."),
    ("Softwaretechnik", "Software Engineering", "Anforderungen, Entwurfsmuster, Tests, agile Vorgehensmodelle im Team."),
    ("Lineare Algebra", "Linear Algebra", "Vektorräume, Matrizen, Determinanten, Eigenwerte."),
    ("Analysis I", "Calculus I", "Folgen, Reihen, Stetigkeit, Differential- und Integralrechnung einer Variablen."),
    ("Wahrscheinlichkeitsrechnung und Statistik", "Probability and Statistics", "Zufallsvariablen, Verteilungen, Schätzer, Hypothesentests."),
    ("Technische Mechanik I – Statik", "Engineering Mechanics I – Statics", "Kräfte, Momente, Gleichgewicht, Lagerreaktionen an Tragwerken."),
    ("Thermodynamik", "Thermodynamics", "Hauptsätze, Zustandsgrößen, Kreisprozesse, Wärmekraftmaschinen."),
    ("Wärme- und Stoffübertragung", "Heat and Mass Transfer", "Wärmeleitung, Konvektion, Strahlung, Diffusion."),
    ("Strömungsmechanik", "Fluid Mechanics", "Hydrostatik, Bernoulli-Gleichung, Rohrströmung, Grenzschichten."),
    ("Regelungstechnik", "Control Engineering", "Regelkreise, Übertragungsfunktionen, Stabilität, PID-Regler."),
    ("Grundlagen der Elektrotechnik", "Fundamentals of Electrical Engineering", "Gleich- und Wechselstrom, Netzwerke, elektrische und magnetische Felder."),
    ("Photovoltaik und Solarthermie", "Photovoltaics and Solar Thermal Energy", "Solarzellen, Wirkungsgrade, Anlagenplanung, solare Wärme."),
    ("Windenergie", "Wind Energy", "Aerodynamik von Rotoren, Anlagentechnik, Netzintegration."),
    ("Energiewirtschaft", "Energy Economics", "Strommärkte, Preisbildung, Energiewende und Regulierung."),
    ("Baustoffkunde", "Building Materials", "Beton, Stahl, Holz und Mauerwerk: Eigenschaften und Prüfung."),
    ("Stahlbetonbau", "Reinforced Concrete Structures", "Bemessung von Balken, Platten und Stützen nach Eurocode."),
    ("Geotechnik", "Geotechnical Engineering", "Bodenmechanik, Grundbau, Setzungen und Standsicherheit von Böschungen."),
    ("Architekturgeschichte des 20. Jahrhunderts", "History of 20th-Century Architecture", "Moderne, Bauhaus, Nachkriegsarchitektur und Postmoderne."),
    ("Städtebau und Stadtplanung", "Urban Design and Planning", "Stadtstrukturen, Bauleitplanung, öffentlicher Raum, nachhaltige Quartiere."),
    ("Umweltrecht", "Environmental Law", "Immissionsschutz, Wasserrecht, Naturschutz und Umweltprüfungen."),
    ("Ökologie", "Ecology", "Populationen, Ökosysteme, Stoffkreisläufe und Biodiversität."),
    ("Klimawandel und Anpassung", "Climate Change and Adaptation", "Klimamodelle, Folgen des Klimawandels, Anpassungsstrategien für Regionen."),
    ("Betriebswirtschaftslehre", "Business Administration", "Unternehmensführung, Rechnungswesen, Marketing und Finanzierung."),
    ("Projektmanagement", "Project Management", "Planung, Zeit- und Kostenmanagement, Risiken und Teamführung."),
    ("Wissenschaftliches Schreiben", "Academic Writing", "Recherche, Zitieren, Aufbau einer Abschlussarbeit, Präsentieren."),
]


def main():
    con = sqlite3.connect(sys.argv[1])
    con.execute("DROP TABLE IF EXISTS v_module")
    con.execute("CREATE TABLE v_module (id INTEGER, title TEXT, title_de TEXT, title_en TEXT, contents TEXT, learning_outcomes TEXT)")
    con.executemany("INSERT INTO v_module VALUES (?, ?, ?, ?, ?, NULL)",
                    [(i + 1, de, de, en, text) for i, (de, en, text) in enumerate(MODULES)])
    con.commit()
    print(f"{len(MODULES)} modules → {sys.argv[1]}")


if __name__ == "__main__":
    main()
