use serde::Serialize;

use crate::app::AppContext;
use crate::categories::{CatalogCategory, CategoryGroup, catalog};
use crate::cli::CategoriesArgs;
use crate::error::Result;

#[derive(Debug, Serialize)]
struct CategoriesDisplay {
    groups: Vec<CategoryGroupDisplay>,
}

#[derive(Debug, Serialize)]
struct CategoryGroupDisplay {
    name: &'static str,
    categories: Vec<CategoryDisplay>,
}

#[derive(Debug, Serialize)]
struct CategoryDisplay {
    id: u32,
    name: &'static str,
    aliases: Vec<&'static str>,
}

pub fn run(_context: &AppContext, args: CategoriesArgs) -> Result<()> {
    let display = CategoriesDisplay::from_catalog(catalog());

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&display).expect("category display serializes")
        );
    } else {
        for group in display.groups {
            let categories = group
                .categories
                .into_iter()
                .map(|category| format!("{} {}", category.id, category.name))
                .collect::<Vec<_>>()
                .join(", ");
            println!("{}: {categories}", group.name);
        }
    }

    Ok(())
}

impl CategoriesDisplay {
    fn from_catalog(catalog: &[CategoryGroup]) -> Self {
        Self {
            groups: catalog
                .iter()
                .map(|group| CategoryGroupDisplay {
                    name: group.name,
                    categories: group
                        .categories
                        .iter()
                        .copied()
                        .map(CategoryDisplay::from)
                        .collect(),
                })
                .collect(),
        }
    }
}

impl From<CatalogCategory> for CategoryDisplay {
    fn from(category: CatalogCategory) -> Self {
        Self {
            id: category.id,
            name: category.name,
            aliases: category.aliases.to_vec(),
        }
    }
}
