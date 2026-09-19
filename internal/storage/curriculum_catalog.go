package storage

import "github.com/jakob/btu-scraper/internal/model"

// HasValidatedCurriculum excludes catalog membership and unverified AI rows.
func (s *Storage) HasValidatedCurriculum(programID string) (bool, error) {
	var exists bool
	err := s.db.QueryRow("SELECT EXISTS(SELECT 1 FROM validated_curriculum_plans WHERE program_id = ?)", programID).Scan(&exists)
	return exists, err
}

// GetCurriculumCatalog loads only fields needed for identity and plausibility checks.
func (s *Storage) GetCurriculumCatalog() ([]model.CurriculumCatalogModule, error) {
	rows, err := s.db.Query(`SELECT id, COALESCE(code,''), COALESCE(title_de,''), COALESCE(title_en,''), COALESCE(turnus,''), COALESCE(duration,''), COALESCE(credits,0) FROM modules ORDER BY id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []model.CurriculumCatalogModule
	for rows.Next() {
		var m model.CurriculumCatalogModule
		if err := rows.Scan(&m.ID, &m.Code, &m.TitleDE, &m.TitleEN, &m.Turnus, &m.Duration, &m.Credits); err != nil {
			return nil, err
		}
		result = append(result, m)
	}
	return result, rows.Err()
}
