use anyhow::{Context, anyhow};
use regex::Regex;
use semver::{Version, VersionReq};
use xshell::{Shell, cmd};

pub trait Checker {
    fn check(&self, shell: &Shell, package: &String, version: &VersionReq) -> anyhow::Result<bool>;
}

#[derive(Default)]
pub struct JsChecker;

impl JsChecker {
    fn find_versions(&self, stdout: &str, package: &str) -> anyhow::Result<Vec<(String, Version)>> {
        let regex = Regex::new(
            r#"^=> Found "([^"\n]+)@([0-9]+(?:\.[0-9]+){2}(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)"$"#,
        )
        .context("parse JsChecker regex")?;

        stdout
            .lines()
            .flat_map(|line| line.split('\r'))
            .map(str::trim_start)
            .filter_map(|line| regex.captures(line))
            .filter(|captures| {
                let found_package = &captures[1];
                found_package == package
                    || found_package
                        .rsplit('#')
                        .next()
                        .is_some_and(|leaf_package| leaf_package == package)
            })
            .map(|captures| {
                Ok((
                    captures[1].to_string(),
                    Version::parse(&captures[2])
                        .context("parse version from capture in JsCheck")?,
                ))
            })
            .collect()
    }
}

impl Checker for JsChecker {
    fn check(&self, shell: &Shell, package: &String, version: &VersionReq) -> anyhow::Result<bool> {
        let command = cmd!(shell, "yarn why {package}");

        match command.output() {
            Ok(result) => {
                let stdout = String::from_utf8(result.stdout)
                    .context("parse JsChecker output from stdout")?;
                let stderr = String::from_utf8(result.stderr)
                    .context("parse JsChecker output from stderr")?;

                if stderr.contains("couldn't find a match") {
                    Ok(false)
                } else {
                    let matches = self
                        .find_versions(&stdout, package)
                        .context("search for package@version in JsChecker")?;

                    if matches.is_empty() {
                        Err(anyhow!(format!(
                            "Couldn't regex matches although package was found by yarn why!\nSTDOUT:\n{stdout}\n\nSTDERR:\n{stderr}"
                        )))
                    } else {
                        Ok(matches
                            .iter()
                            .any(|(_, found_version)| version.matches(found_version)))
                    }
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[derive(Debug)]
pub struct CargoChecker {
    tree: String,
}

impl CargoChecker {
    pub fn new(folder_path: &String) -> anyhow::Result<Self> {
        let sh = Shell::new().context("creating new shell in CargoChecker")?;
        sh.change_dir(folder_path);
        let tree = cmd!(sh, "cargo tree")
            .read()
            .context("running cargo tree")?;

        Ok(Self { tree })
    }

    fn find_versions(&self, package: &str) -> anyhow::Result<Vec<Version>> {
        let pattern = format!(
            r#"(?m)^[^\n]*\b{} v([0-9]+(?:\.[0-9]+){{2}}(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)(?:\s|\(|$)"#,
            regex::escape(package)
        );
        let regex = Regex::new(&pattern).context("parse cargo checker regex")?;

        self.tree
            .lines()
            .filter_map(|line| regex.captures(line))
            .map(|captures| Version::parse(&captures[1]).map_err(Into::into))
            .collect()
    }
}

impl Checker for CargoChecker {
    fn check(
        &self,
        _shell: &Shell,
        package: &String,
        version: &VersionReq,
    ) -> anyhow::Result<bool> {
        Ok(self
            .find_versions(package)
            .context("find_versions in CargoChecker")?
            .iter()
            .any(|found_version| version.matches(found_version)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YARN_WHY_OUTPUT: &str = r#"yarn why v1.22.22
[1/4] 🤔  Why do we have the module "form-data"...?
[2/4] 🚚  Initialising dependency graph...
warning Resolution field "canvas@3.2.2" is incompatible with requested version "canvas@^2.11.2"
warning Resolution field "parse-url@8.1.0" is incompatible with requested version "parse-url@^6.0.0"
[3/4] 🔍  Finding dependency...
[4/4] 🚡  Calculating file sizes...
=> Found "form-data@3.0.4"
info Has been hoisted to "form-data"
=> Found "axios#form-data@4.0.5"
info This module exists because "@seermedical#seer-test-reports#axios" depends on it.
=> Found "@cypress/request#form-data@2.3.3"
info This module exists because "cypress#@cypress#request" depends on it.
✨  Done in 0.67s.
"#;

    const CARGO_TREE_OUTPUT: &str = r#"check_package v0.1.0 (/Users/willhart/src/check_package)
├── anyhow v1.0.99
├── regex v1.12.3
│   ├── aho-corasick v1.1.4
│   │   └── memchr v2.8.0
│   ├── memchr v2.8.0
│   ├── regex-automata v0.4.14
│   │   ├── aho-corasick v1.1.4 (*)
│   │   ├── memchr v2.8.0
│   │   └── regex-syntax v0.8.10
│   └── regex-syntax v0.8.10
├── semver v1.0.28
│   └── serde_core v1.0.228
├── serde v1.0.228
│   ├── serde_core v1.0.228
│   └── serde_derive v1.0.228 (proc-macro)
│       ├── proc-macro2 v1.0.106
│       │   └── unicode-ident v1.0.24
│       ├── quote v1.0.45
│       │   └── proc-macro2 v1.0.106 (*)
│       └── syn v2.0.117
│           ├── proc-macro2 v1.0.106 (*)
│           ├── quote v1.0.45 (*)
│           └── unicode-ident v1.0.24
├── serde_json v1.0.149
│   ├── itoa v1.0.18
│   ├── memchr v2.8.0
│   ├── serde_core v1.0.228
│   └── zmij v1.0.21
└── xshell v0.2.7
    └── xshell-macros v0.2.7 (proc-macro)
"#;

    #[test]
    fn finds_all_matching_package_versions_from_yarn_why_output() {
        let matches = JsChecker
            .find_versions(YARN_WHY_OUTPUT, "form-data")
            .unwrap();

        assert_eq!(
            matches,
            vec![
                ("form-data".to_string(), Version::parse("3.0.4").unwrap()),
                (
                    "axios#form-data".to_string(),
                    Version::parse("4.0.5").unwrap(),
                ),
                (
                    "@cypress/request#form-data".to_string(),
                    Version::parse("2.3.3").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn finds_versions_for_scoped_packages() {
        let stdout = r#"
=> Found "@types/node@24.10.0"
=> Found "vite#@types/node@20.19.1"
"#;

        let matches = JsChecker.find_versions(stdout, "@types/node").unwrap();

        assert_eq!(
            matches,
            vec![
                (
                    "@types/node".to_string(),
                    Version::parse("24.10.0").unwrap(),
                ),
                (
                    "vite#@types/node".to_string(),
                    Version::parse("20.19.1").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn supports_prerelease_and_build_versions() {
        let stdout = r#"
=> Found "pkg@1.2.3-beta.4"
=> Found "parent#pkg@1.2.3+build.7"
=> Found "other#pkg@1.2.3-rc.1+sha.123"
"#;

        let matches = JsChecker.find_versions(stdout, "pkg").unwrap();

        assert_eq!(
            matches,
            vec![
                ("pkg".to_string(), Version::parse("1.2.3-beta.4").unwrap(),),
                (
                    "parent#pkg".to_string(),
                    Version::parse("1.2.3+build.7").unwrap(),
                ),
                (
                    "other#pkg".to_string(),
                    Version::parse("1.2.3-rc.1+sha.123").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn ignores_non_matching_packages_and_similar_names() {
        let stdout = r#"
=> Found "form-data-extra@9.9.9"
=> Found "other#form-data-extra@8.8.8"
=> Found "data-form@7.7.7"
"#;

        let matches = JsChecker.find_versions(stdout, "form-data").unwrap();

        assert!(matches.is_empty());
    }

    #[test]
    fn escapes_package_names_when_building_regex() {
        let stdout = r#"
=> Found "@scope/pkg@1.0.0"
=> Found "parent#@scope/pkg@1.1.0"
=> Found "@scope/pkg-extra@1.2.0"
"#;

        let matches = JsChecker.find_versions(stdout, "@scope/pkg").unwrap();

        assert_eq!(
            matches,
            vec![
                ("@scope/pkg".to_string(), Version::parse("1.0.0").unwrap(),),
                (
                    "parent#@scope/pkg".to_string(),
                    Version::parse("1.1.0").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn reports_true_when_any_found_version_matches_requirement() {
        let matches = JsChecker
            .find_versions(YARN_WHY_OUTPUT, "form-data")
            .unwrap();
        let requirement = VersionReq::parse("^4.0").unwrap();

        assert!(
            matches
                .iter()
                .any(|(_, found_version)| requirement.matches(found_version))
        );
    }

    #[test]
    fn reports_false_when_no_found_version_matches_requirement() {
        let matches = JsChecker
            .find_versions(YARN_WHY_OUTPUT, "form-data")
            .unwrap();
        let requirement = VersionReq::parse(">=5.0.0").unwrap();

        assert!(
            !matches
                .iter()
                .any(|(_, found_version)| requirement.matches(found_version))
        );
    }

    #[test]
    fn returns_error_for_invalid_semver_in_matching_line() {
        let stdout = r#"
=> Found "pkg@1.2.3-01"
"#;

        assert!(JsChecker.find_versions(stdout, "pkg").is_err());
    }

    #[test]
    fn finds_versions_from_plain_yarn_why_output() {
        let stdout = r#"yarn why v1.22.22
    [1/4] Why do we have the module "form-data"...?
    [2/4] Initialising dependency graph...
    [3/4] Finding dependency...
    [4/4] Calculating file sizes...
=> Found "form-data@4.0.0"
    info Reasons this module exists
       - "jest-environment-jsdom#jsdom" depends on it
       - Hoisted from "jest-environment-jsdom#jsdom#form-data"
    info Disk size without dependencies: "64KB"
    info Disk size with unique dependencies: "212KB"
    info Disk size with transitive dependencies: "456KB"
    info Number of shared dependencies: 4
=> Found "graphql-request#form-data@3.0.1"
    info This module exists because "@graphql-codegen#cli#@graphql-tools#prisma-loader#graphql-request" depends on it.
    info Disk size without dependencies: "64KB"
    info Disk size with unique dependencies: "212KB"
    info Disk size with transitive dependencies: "456KB"
    info Number of shared dependencies: 4
    Done in 0.28s.
"#;

        let matches = JsChecker.find_versions(stdout, "form-data").unwrap();

        assert_eq!(
            matches,
            vec![
                ("form-data".to_string(), Version::parse("4.0.0").unwrap()),
                (
                    "graphql-request#form-data".to_string(),
                    Version::parse("3.0.1").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn finds_versions_when_yarn_why_uses_carriage_returns() {
        let stdout = "yarn why v1.22.22\ninfo \r=> Found \"form-data@4.0.0\"\ninfo \r=> Found \"graphql-request#form-data@3.0.1\"\n";

        let matches = JsChecker.find_versions(stdout, "form-data").unwrap();

        assert_eq!(
            matches,
            vec![
                ("form-data".to_string(), Version::parse("4.0.0").unwrap()),
                (
                    "graphql-request#form-data".to_string(),
                    Version::parse("3.0.1").unwrap(),
                ),
            ]
        );
    }

    #[test]
    fn cargo_checker_finds_versions_in_tree_output() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        let matches = checker.find_versions("regex").unwrap();

        assert_eq!(matches, vec![Version::parse("1.12.3").unwrap()]);
    }

    #[test]
    fn cargo_checker_matches_root_package_version() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        let matches = checker.find_versions("check_package").unwrap();

        assert_eq!(matches, vec![Version::parse("0.1.0").unwrap()]);
    }

    #[test]
    fn cargo_checker_collects_multiple_occurrences_of_the_same_package() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        let matches = checker.find_versions("memchr").unwrap();

        assert_eq!(
            matches,
            vec![
                Version::parse("2.8.0").unwrap(),
                Version::parse("2.8.0").unwrap(),
                Version::parse("2.8.0").unwrap(),
                Version::parse("2.8.0").unwrap(),
            ]
        );
    }

    #[test]
    fn cargo_checker_ignores_similar_package_names() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        let matches = checker.find_versions("shell").unwrap();

        assert!(matches.is_empty());
    }

    #[test]
    fn cargo_checker_handles_suffixes_after_versions() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        let proc_macro_matches = checker.find_versions("serde_derive").unwrap();
        let repeated_matches = checker.find_versions("aho-corasick").unwrap();

        assert_eq!(proc_macro_matches, vec![Version::parse("1.0.228").unwrap()]);
        assert_eq!(
            repeated_matches,
            vec![
                Version::parse("1.1.4").unwrap(),
                Version::parse("1.1.4").unwrap(),
            ]
        );
    }

    #[test]
    fn cargo_checker_reports_true_when_requirement_matches() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        assert!(
            checker
                .check(
                    &Shell::new().unwrap(),
                    &"regex".to_string(),
                    &VersionReq::parse("^1.12").unwrap()
                )
                .unwrap()
        );
    }

    #[test]
    fn cargo_checker_reports_false_when_requirement_does_not_match() {
        let checker = CargoChecker {
            tree: CARGO_TREE_OUTPUT.to_string(),
        };

        assert!(
            !checker
                .check(
                    &Shell::new().unwrap(),
                    &"regex".to_string(),
                    &VersionReq::parse(">=2.0.0").unwrap()
                )
                .unwrap()
        );
    }
}
