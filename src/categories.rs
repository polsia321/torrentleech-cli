use crate::error::{ErrorKind, Result, TlError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryGroup {
    pub name: &'static str,
    pub categories: &'static [CatalogCategory],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogCategory {
    pub id: u32,
    pub name: &'static str,
    pub group: &'static str,
    pub aliases: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategorySelection {
    Group {
        alias: String,
        group: &'static str,
        category_ids: Vec<u32>,
    },
    Category {
        alias: String,
        category: CatalogCategory,
    },
}

impl CategorySelection {
    #[must_use]
    pub fn category_ids(&self) -> Vec<u32> {
        match self {
            Self::Group { category_ids, .. } => category_ids.clone(),
            Self::Category { category, .. } => vec![category.id],
        }
    }
}

const MOVIES: &[CatalogCategory] = &[
    category(8, "Cam", "Movies", &[]),
    category(9, "TS/TC", "Movies", &[]),
    category(11, "DVDRip/DVDScreener", "Movies", &[]),
    category(37, "WEBRip", "Movies", &[]),
    category(43, "HDRip", "Movies", &[]),
    category(14, "BlurayRip", "Movies", &[]),
    category(12, "DVD-R", "Movies", &[]),
    category(13, "Bluray", "Movies", &[]),
    category(47, "4K", "Movies", &[]),
    category(15, "Boxsets", "Movies", &[]),
    category(29, "Documentaries", "Movies", &[]),
];
const TV: &[CatalogCategory] = &[
    category(26, "Episodes", "TV", &[]),
    category(32, "Episodes HD", "TV", &[]),
    category(27, "TV Boxsets", "TV", &[]),
];
const GAMES: &[CatalogCategory] = &[
    category(17, "PC", "Games", &[]),
    category(42, "Mac", "Games", &[]),
    category(18, "PS", "Games", &[]),
    category(19, "PS2", "Games", &[]),
    category(40, "PS3", "Games", &[]),
    category(20, "PSP", "Games", &[]),
    category(21, "Xbox", "Games", &[]),
    category(39, "Xbox 360", "Games", &[]),
    category(49, "Xbox One", "Games", &[]),
    category(22, "Wii", "Games", &[]),
    category(28, "Nintendo DS", "Games", &[]),
    category(30, "Nintendo 3DS", "Games", &[]),
    category(48, "Nintendo Switch", "Games", &[]),
];
const APPS: &[CatalogCategory] = &[
    category(23, "PC-ISO", "Apps", &[]),
    category(24, "Mac", "Apps", &[]),
    category(25, "Mobile", "Apps", &[]),
    category(33, "0-day", "Apps", &["0day", "0_day"]),
];
const EDUCATION: &[CatalogCategory] = &[category(38, "Education", "Education", &[])];
const ANIMATION: &[CatalogCategory] = &[
    category(34, "Anime", "Animation", &[]),
    category(35, "Cartoons", "Animation", &[]),
];
const BOOKS: &[CatalogCategory] = &[
    category(45, "Ebooks", "Books", &[]),
    category(46, "Comics", "Books", &[]),
];
const MUSIC: &[CatalogCategory] = &[
    category(31, "Audio", "Music", &[]),
    category(16, "Music Videos", "Music", &[]),
];
const FOREIGN: &[CatalogCategory] = &[
    category(36, "Movies foreign", "Foreign", &[]),
    category(44, "TV Series foreign", "Foreign", &[]),
];

const CATALOG: &[CategoryGroup] = &[
    group("Movies", MOVIES),
    group("TV", TV),
    group("Games", GAMES),
    group("Apps", APPS),
    group("Education", EDUCATION),
    group("Animation", ANIMATION),
    group("Books", BOOKS),
    group("Music", MUSIC),
    group("Foreign", FOREIGN),
];

const fn category(
    id: u32,
    name: &'static str,
    group: &'static str,
    aliases: &'static [&'static str],
) -> CatalogCategory {
    CatalogCategory {
        id,
        name,
        group,
        aliases,
    }
}

const fn group(name: &'static str, categories: &'static [CatalogCategory]) -> CategoryGroup {
    CategoryGroup { name, categories }
}

#[must_use]
pub const fn catalog() -> &'static [CategoryGroup] {
    CATALOG
}

pub fn parse_selection(input: &str) -> Result<CategorySelection> {
    if let Ok(id) = input.parse::<u32>() {
        return Ok(CategorySelection::Category {
            alias: input.to_string(),
            category: lookup_category(id)?,
        });
    }

    let normalized = normalize(input);

    for group in CATALOG {
        if normalize(group.name) == normalized {
            return Ok(CategorySelection::Group {
                alias: input.to_string(),
                group: group.name,
                category_ids: group
                    .categories
                    .iter()
                    .map(|category| category.id)
                    .collect(),
            });
        }
    }

    for category in categories() {
        if normalize(category.name) == normalized
            || category
                .aliases
                .iter()
                .any(|alias| normalize(alias) == normalized)
        {
            return Ok(CategorySelection::Category {
                alias: input.to_string(),
                category,
            });
        }
    }

    Err(invalid_input(format!("unknown category '{input}'")))
}

pub fn lookup_category(id: u32) -> Result<CatalogCategory> {
    categories()
        .find(|category| category.id == id)
        .ok_or_else(|| invalid_input(format!("unknown category id '{id}'")))
}

fn categories() -> impl Iterator<Item = CatalogCategory> {
    CATALOG
        .iter()
        .flat_map(|group| group.categories.iter().copied())
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|character| !matches!(character, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn invalid_input(message: String) -> TlError {
    TlError::new(ErrorKind::InvalidInput, message)
}
