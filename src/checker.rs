use anyhow::anyhow;
use xshell::{Shell, cmd};

pub trait Checker {
    fn check(&self, shell: &Shell, package: &String, version: &String) -> anyhow::Result<bool>;
}

#[derive(Default)]
pub struct JsChecker;

impl Checker for JsChecker {
    fn check(&self, shell: &Shell, package: &String, version: &String) -> anyhow::Result<bool> {
        let command = cmd!(shell, "yarn why {package}");

        match command.output() {
            Ok(result) => {
                let stdout = String::from_utf8(result.stdout)?;
                let stderr = String::from_utf8(result.stderr)?;

                if stdout.contains("Found") && stdout.contains(version) {
                    Ok(true)
                } else if stderr.contains("couldn't find a match") {
                    Ok(false)
                } else {
                    Err(anyhow!(stdout))
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
        let sh = Shell::new()?;
        sh.change_dir(folder_path);
        let tree = cmd!(sh, "cargo tree").read()?;

        Ok(Self { tree })
    }
}

impl Checker for CargoChecker {
    fn check(&self, _shell: &Shell, package: &String, version: &String) -> anyhow::Result<bool> {
        Ok(!self
            .tree
            .lines()
            .filter(|l| l.contains(&format!("{package} v")) && l.contains(version))
            .collect::<Vec<_>>()
            .is_empty())
    }
}
