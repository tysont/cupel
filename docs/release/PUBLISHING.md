<!-- ABOUTME: The owner's one-time steps that turn on publishing to npm, PyPI, and the Homebrew tap from release.yml (ADR-235), -->
<!-- ABOUTME: how to check the first publish after a tag, and how to stop or undo one. -->
# Publishing

`.github/workflows/release.yml` publishes on every `v*` tag, after the GitHub Release exists. Its `publishers` job opens one gate per registry; a closed gate skips that job with a `skipped: …` notice and the run still succeeds ([ADR-235](../adr/CUPEL_DECISION_235.md)).

| Job | Registry | Credential | Gate (on `tysont/cupel`) | GitHub environment |
| --- | --- | --- | --- | --- |
| `publish-npm` | npm `cupel-cli` | OIDC trusted publisher | variable `NPM_TRUSTED_PUBLISHING` = `true` | `npm` |
| `pypi-build`, `publish-pypi` | PyPI `cupel-cli` | OIDC trusted publisher | variable `PYPI_TRUSTED_PUBLISHING` = `true` | `pypi` |
| `publish-homebrew` | `tysont/homebrew-cupel` | SSH deploy key | secret `HOMEBREW_TAP_DEPLOY_KEY` set | none |
| `publish-crates` | crates.io | — | stays skipped ([ADR-146](../adr/CUPEL_DECISION_146.md)) | none |

Values every registry form asks for: owner **`tysont`**, repository **`cupel`**, workflow file **`release.yml`** (the file name only), package/project **`cupel-cli`**, environment **`npm`** on npm and **`pypi`** on PyPI.

Set a gate only after its registry is configured: an open gate whose registry does not trust the workflow fails the run.

## Order

1. GitHub: the environments (below). Any time.
2. PyPI: the pending publisher, then its variable. Any time.
3. Homebrew: the deploy key and its secret. Any time.
4. npm: the first version by hand any time; the trusted publisher and its variable **within 2 days before the next tag** (npm expires an unused configuration after 2 days).
5. Push the next tag.

## 1. GitHub environments

Create `npm` and `pypi`, each limited to `v*` tags:

```sh
for env in npm pypi; do
  gh api -X PUT "repos/tysont/cupel/environments/$env" --input - <<'JSON'
{"deployment_branch_policy": {"protected_branches": false, "custom_branch_policies": true}}
JSON
  gh api -X POST "repos/tysont/cupel/environments/$env/deployment-branch-policies" -f name='v*' -f type=tag
done
gh api repos/tysont/cupel/environments --jq '.environments[].name'   # lists npm and pypi
```

## 2. PyPI (no project needed: a pending publisher creates it)

1. Sign in at https://pypi.org and open https://pypi.org/manage/account/publishing/ (account sidebar → Publishing).
2. Under "Add a new pending publisher", choose **GitHub** and enter:
   - PyPI Project Name: `cupel-cli`
   - Owner: `tysont`
   - Repository name: `cupel`
   - Workflow name: `release.yml`
   - Environment name: `pypi`
3. Click **Add**.
4. `gh variable set PYPI_TRUSTED_PUBLISHING --repo tysont/cupel --body true`

A pending publisher does not reserve the name; it becomes the project's publisher on the first upload.

## 3. Homebrew tap deploy key

The tap is empty; the first run creates `main` with `Formula/cupel.rb` and a `README.md`.

```sh
dir="$(mktemp -d)"
ssh-keygen -t ed25519 -N '' -C 'tysont/cupel release.yml' -f "$dir/tap"
gh repo deploy-key add "$dir/tap.pub" --repo tysont/homebrew-cupel --title 'tysont/cupel release.yml' --allow-write
gh secret set HOMEBREW_TAP_DEPLOY_KEY --repo tysont/cupel < "$dir/tap"
rm -rf "$dir"
```

A key added by `gh` is removed if that `gh` login's token is ever de-authorized. To avoid that, replace the `gh repo deploy-key add` line with `cat "$dir/tap.pub"` and paste its output at https://github.com/tysont/homebrew-cupel/settings/keys/new (Title `tysont/cupel release.yml`, tick **Allow write access**).

## 4. npm

npm cannot trust a workflow for a package that does not exist, so the first version is published by hand once, with a short-lived `npm login` session (no stored token). It must be a released version: the wrapper downloads its own version's binary.

1. Turn on 2FA for the npm account `tysont` if it is not on (https://www.npmjs.com/settings/~/tfa).
2. Publish `cupel-cli@0.2.9` from the `v0.2.9` tag (npm 11.5.1 or later):
   ```sh
   git clone --depth 1 --branch v0.2.9 https://github.com/tysont/cupel.git "$TMPDIR/cupel-v0.2.9"
   cd "$TMPDIR/cupel-v0.2.9"
   npm login
   npm publish ./dist/npm --access public
   npm logout
   ```
3. Within 2 days before the next tag, add the trusted publisher at https://www.npmjs.com/package/cupel-cli/access → **Trusted Publisher** → **GitHub Actions**:
   - Organization or user: `tysont`
   - Repository: `cupel`
   - Workflow filename: `release.yml`
   - Environment name: `npm`
   - Allowed actions: tick **npm publish**

   Or with npm 11.15.0 or later: `npm trust github cupel-cli --file release.yml --repository tysont/cupel --environment npm --allow-publish`
4. On the same page, under **Publishing access**, choose **Require two-factor authentication and disallow tokens** (trusted publishing keeps working).
5. `gh variable set NPM_TRUSTED_PUBLISHING --repo tysont/cupel --body true`

If the tag is more than 2 days away, wait to do step 3. An expired configuration cannot be edited: delete it and add it again.

## Verify after the next tag

```sh
run="$(gh run list --repo tysont/cupel --workflow release.yml --event push --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run view "$run" --repo tysont/cupel   # publish-npm, pypi-build, publish-pypi, publish-homebrew green; one notice, publish-crates skipped
v=0.3.0   # the tag's version
npm view "cupel-cli@$v" version dist.attestations.provenance.predicateType   # the version and an SLSA provenance type
curl -s "https://pypi.org/pypi/cupel-cli/$v/json" | jq -r '.info.version'
gh api repos/tysont/homebrew-cupel/commits/main --jq '.commit.message'       # "cupel <version>"
npx -y "cupel-cli@$v" --version                                              # cupel <version>
pipx run --spec "cupel-cli==$v" cupel --version                              # cupel <version>
brew install tysont/cupel/cupel && cupel --version                           # Homebrew 7 may first ask: brew trust tysont/cupel
```

A gate still closed shows as `skipped: variable … is not true` or `skipped: secret … not set` in the run's notices and summary.

## Stop or undo

- **Stop a route** before the next tag: `gh variable delete NPM_TRUSTED_PUBLISHING --repo tysont/cupel`, `gh variable delete PYPI_TRUSTED_PUBLISHING --repo tysont/cupel`, or `gh secret delete HOMEBREW_TAP_DEPLOY_KEY --repo tysont/cupel`.
- **Revoke trust**: npm, https://www.npmjs.com/package/cupel-cli/access → Trusted Publisher → delete (or `npm trust list cupel-cli`, then `npm trust revoke --id <id> cupel-cli`); PyPI, https://pypi.org/manage/project/cupel-cli/settings/publishing/ → Remove (a pending one: https://pypi.org/manage/account/publishing/); the tap, `gh repo deploy-key list --repo tysont/homebrew-cupel`, then `gh repo deploy-key delete <id> --repo tysont/homebrew-cupel`.
- **A bad version**: npm, `npm deprecate cupel-cli@<version> "<reason>"` (`npm unpublish cupel-cli@<version>` only within 72 hours, and the version number can never be reused); PyPI, https://pypi.org/manage/project/cupel-cli/releases/ → the release → Options → **Yank**; the tap, revert its last commit:
  ```sh
  git clone git@github.com:tysont/homebrew-cupel.git "$TMPDIR/tap" && cd "$TMPDIR/tap"
  git revert --no-edit HEAD && git push origin main
  ```
