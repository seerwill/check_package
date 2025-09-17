use anyhow::bail;

const DATA_PATH: &'static str = "data.txt";
const REPO_PATH: &'static str = "repos.txt";

/// Takes a line from the data path file and extracts the package
fn parse_line(line: &str) -> anyhow::Result<String> {
    match line.split(',').next() {
        Some(v) => Ok(v.to_string()),
        None => bail!("Invalid line - {line}"),
    }
}

/// Checks the lock file at the given path and prints out whether or not
/// it contains the vulnerable packages specified in the `packages` variable
fn is_repository_vulnerable(lockfile: &str, packages: &[String]) -> anyhow::Result<bool> {
    let mut vulnerable = false;

    for line in lockfile.lines() {
        for package in packages {
            if line.contains(package) {
                println!(" --> potential vulnerability - {line}");
                vulnerable = true;
            }
        }
    }

    Ok(vulnerable)
}

fn main() -> anyhow::Result<()> {
    let packages = std::fs::read_to_string(DATA_PATH)?
        .lines()
        .map(parse_line)
        .collect::<Result<Vec<String>, anyhow::Error>>()?;

    for repo in std::fs::read_to_string(REPO_PATH)?.lines() {
        println!("Checking {repo}");
        let lockfile = std::fs::read_to_string(repo)?;

        if is_repository_vulnerable(&lockfile, &packages)? {
            println!(" --> REPO IS POTENTIALLY VULNERABLE");
        } else {
            println!(" --> Repo doesn't appear to be vulnerable");
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::is_repository_vulnerable;

    #[test]
    fn test_is_vulnerable() -> anyhow::Result<()> {
        assert!(is_repository_vulnerable(
            "knex\nknex-migrate\nis-ansi",
            &["knex".into()]
        )?);
        assert!(!is_repository_vulnerable(
            "knex\nknex-migrate\nis-ansi",
            &["knexo".into()]
        )?);
        assert!(!is_repository_vulnerable(
            "knex\nknex-migrate\nis-ansi",
            &["my-test-repo".into()]
        )?);

        Ok(())
    }
}
