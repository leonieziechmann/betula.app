#[cfg(test)]
mod tests {
    use crate::slug::*;

    #[test]
    fn test_slug_generation_and_resolution() {
        let programs = vec![
            ProgramOption {
                id: "stg_1".into(),
                program_name: "Informatik".into(),
                degree: Some("Bachelor (universitär)".into()),
                po_version: Some("2008 - 2. SÄ 2024".into()),
            },
            ProgramOption {
                id: "stg_2".into(),
                program_name: "Informatik".into(),
                degree: Some("Master (universitär)".into()),
                po_version: Some("2008".into()),
            },
        ];

        let slug1 = program_slug("stg_1", &programs);
        assert_eq!(slug1, "bsc-informatik-2008");
        assert_eq!(resolve_program("bsc-informatik-2008", &programs), "stg_1");

        let slug2 = program_slug("stg_2", &programs);
        assert_eq!(slug2, "msc-informatik-2008");
        assert_eq!(resolve_program("msc-informatik-2008", &programs), "stg_2");
    }

    #[test]
    fn test_format_degree_short() {
        assert_eq!(format_degree_short(Some("Bachelor of Science")), "B.Sc.");
        assert_eq!(format_degree_short(Some("Master of Science")), "M.Sc.");
        assert_eq!(format_degree_short(Some("Bachelor of Arts")), "B.A.");
        assert_eq!(format_degree_short(Some("Master of Arts")), "M.A.");
        assert_eq!(format_degree_short(Some("Bachelor of Engineering")), "B.Eng.");
        assert_eq!(format_degree_short(Some("keine Abschlussprüfung möglich")), "");
    }
}
