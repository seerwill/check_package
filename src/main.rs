use std::collections::HashMap;

use anyhow::{Context, anyhow};
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use xshell::Shell;

use crate::checker::{CargoChecker, Checker, JsChecker};

mod checker;

const DATA_PATH: &'static str = "deps.json";
const REPO_PATH: &'static str = "repos.json";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    JS,
    Rust,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Repo {
    pub folder_path: String,
    pub language: Language,
    #[serde(skip)]
    packages: Vec<Package>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepoList {
    repos: Vec<Repo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub versions: Vec<VersionReq>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageList {
    packages: HashMap<Language, Vec<Package>>,
}

impl Repo {
    pub fn check(&self) -> anyhow::Result<Vec<String>> {
        let sh = Shell::new().context("create new main shell")?;
        sh.change_dir(&self.folder_path);

        let checker: Box<dyn Checker> = match self.language {
            Language::JS => Box::new(JsChecker::default()),
            Language::Rust => Box::new(
                CargoChecker::new(&self.folder_path).context("create CargoChecker from folder")?,
            ),
        };

        Ok(self
            .packages
            .iter()
            .flat_map(|pkg| {
                let package = &pkg.name;

                pkg.versions
                    .iter()
                    .filter_map(|version| {
                        let result = checker
                            .check(&sh, package, version)
                            .context("Running repo Check")
                            .expect("Error running query");

                        if result {
                            Some(format!("{package}@{version}"))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect())
    }
}

fn main() -> anyhow::Result<()> {
    let repo_list = serde_json::from_str::<RepoList>(
        &std::fs::read_to_string(REPO_PATH).context("Find repo file")?,
    )
    .context("load repo file")?;
    let package_list = serde_json::from_str::<PackageList>(
        &std::fs::read_to_string(DATA_PATH).context("Find data file")?,
    )
    .context("load package list")?;

    let mut found_bad = false;

    for repo in repo_list.repos {
        println!("Checking {}", &repo.folder_path);

        // lots of cloning but who cares its a throwaway CLI
        let mut r = repo.clone();
        r.packages = package_list
            .packages
            .get(&r.language)
            .expect(&format!(
                "Missing language {:?} in packages.json",
                r.language
            ))
            .clone();

        let vulnerable_packages = r.check().context(format!(
            "check for vulnerable packages in {}",
            r.folder_path
        ))?;
        if vulnerable_packages.is_empty() {
            println!("    no vulnerable package versions found");
        } else {
            found_bad = true;

            println!("    VULNERABLE PACKAGES FOUND");
            for msg in vulnerable_packages {
                println!("    {}", msg);
            }
        }
    }

    if !found_bad {
        Ok(())
    } else {
        Err(anyhow!("Found bad packages"))
    }
}
