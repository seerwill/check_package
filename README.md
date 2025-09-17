# Check packages

A very simple tool written in 30 seconds that takes two files:

- `data.txt` holding a list of vulnerable packages one per line
- `repos.txt` a list of `yarn.lock`, `package-lock.json` or `Cargo.lock` paths
  (one per line) that should be checked for vulnerable packages

For example, `repos.txt` may contain

```txt
/Users/me/repo1/yarn.lock
/Users/me/repo2/Cargo.lock
```

And vulnerabilities may contain

```txt
vulnerable-package-1
vulnerable-package-2
```

When `cargo run` is called the list of vulnerable packages in `data.txt` will be
compared against each of the lockfiles and the output will show whether the
package was found.

The output may look something like this:

```txt
Checking /Users/me/repo1/yarn.lock
 --> potential vulnerability - vulnerable-package-1@^1.0.4:
 --> potential vulnerability -   resolved "https://registry.yarnpkg.com/vulnerable-package-1/-/vulnerable-package-1-1.0.4.tgz#abcdef1234567890"
 --> REPO IS POTENTIALLY VULNERABLE
Checking /Users/me/repo2/Cargo.lock
 --> Repo doesn't appear to be vulnerable
```

> Note - this is a quick check, not a guarantee of vulnerability / not vulnerability
