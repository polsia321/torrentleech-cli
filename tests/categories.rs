use torrentleech_cli::categories::{CategorySelection, catalog, lookup_category, parse_selection};
use torrentleech_cli::error::ErrorKind;

#[test]
fn zero_day_aliases_resolve_to_category_id() {
    for alias in ["0-day", "0day", "0_day"] {
        assert_eq!(parse_selection(alias).unwrap().category_ids(), &[33]);
    }
}

#[test]
fn documented_groups_resolve_to_site_ordered_ids() {
    let groups = [
        ("Movies", &[8, 9, 11, 37, 43, 14, 12, 13, 47, 15, 29][..]),
        ("TV", &[26, 32, 27][..]),
        (
            "Games",
            &[17, 42, 18, 19, 40, 20, 21, 39, 49, 22, 28, 30, 48][..],
        ),
        ("Apps", &[23, 24, 25, 33][..]),
        ("Education", &[38][..]),
        ("Animation", &[34, 35][..]),
        ("Books", &[45, 46][..]),
        ("Music", &[31, 16][..]),
        ("Foreign", &[36, 44][..]),
    ];

    for (alias, ids) in groups {
        assert_eq!(parse_selection(alias).unwrap().category_ids(), ids);
    }
}

#[test]
fn documented_advanced_categories_resolve_to_ids() {
    let categories = [
        ("Cam", 8),
        ("TS/TC", 9),
        ("DVDRip/DVDScreener", 11),
        ("WEBRip", 37),
        ("HDRip", 43),
        ("BlurayRip", 14),
        ("DVD-R", 12),
        ("Bluray", 13),
        ("4K", 47),
        ("Boxsets", 15),
        ("Documentaries", 29),
        ("Episodes", 26),
        ("Episodes HD", 32),
        ("TV Boxsets", 27),
        ("PC-ISO", 23),
        ("Audio", 31),
        ("0-day", 33),
        ("Education", 38),
        ("Anime", 34),
        ("Movies foreign", 36),
        ("TV Series foreign", 44),
    ];

    for (alias, id) in categories {
        assert_eq!(parse_selection(alias).unwrap().category_ids(), &[id]);
    }
}

#[test]
fn normalization_ignores_case_spaces_hyphens_and_underscores() {
    for alias in ["episodes hd", "Episodes-HD", "EPISODES_HD"] {
        assert_eq!(parse_selection(alias).unwrap().category_ids(), &[32]);
    }

    for alias in ["tv boxsets", "TV-Boxsets", "tv_boxsets"] {
        assert_eq!(parse_selection(alias).unwrap().category_ids(), &[27]);
    }
}

#[test]
fn numeric_built_in_category_ids_parse_to_single_categories() {
    assert_eq!(parse_selection("33").unwrap().category_ids(), &[33]);
}

#[test]
fn parsed_selection_retains_selection_kind() {
    assert_eq!(
        parse_selection("apps").unwrap(),
        CategorySelection::Group {
            alias: "apps".to_string(),
            group: "Apps",
            category_ids: vec![23, 24, 25, 33],
        }
    );
    assert_eq!(
        parse_selection("0-day").unwrap(),
        CategorySelection::Category {
            alias: "0-day".to_string(),
            category: lookup_category(33).unwrap(),
        }
    );
}

#[test]
fn lookup_returns_canonical_category_and_group_names() {
    let pc_iso = lookup_category(23).unwrap();
    assert_eq!(pc_iso.id, 23);
    assert_eq!(pc_iso.name, "PC-ISO");
    assert_eq!(pc_iso.group, "Apps");

    let zero_day = lookup_category(33).unwrap();
    assert_eq!(zero_day.id, 33);
    assert_eq!(zero_day.name, "0-day");
    assert_eq!(zero_day.group, "Apps");
}

#[test]
fn catalog_exposes_groups_in_site_order() {
    let group_names: Vec<_> = catalog().iter().map(|group| group.name).collect();
    assert_eq!(
        group_names,
        [
            "Movies",
            "TV",
            "Games",
            "Apps",
            "Education",
            "Animation",
            "Books",
            "Music",
            "Foreign",
        ]
    );
}

#[test]
fn unknown_alias_and_id_return_invalid_input_errors() {
    let alias_error = parse_selection("not a category").unwrap_err();
    assert_eq!(alias_error.kind(), ErrorKind::InvalidInput);
    assert_eq!(alias_error.exit_code(), 2);

    let id_error = lookup_category(999).unwrap_err();
    assert_eq!(id_error.kind(), ErrorKind::InvalidInput);
    assert_eq!(id_error.exit_code(), 2);
}
