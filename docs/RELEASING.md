# Release checklist

The current 0.1.0 source is experimental and has no public stable release. Building
packages does not publish anything. Use GitHub prereleases for initial native testing.

## Publishing source updates

The source repository already exists at `janderpt/linux-system-explorer`. Publishing source commits to `main` is separate from publishing native binaries or submitting to a store.

- Update the Portuguese and English READMEs, dependency instructions, changelog and known limitations for the actual behavior.
- Review tracked files for credentials, private paths, real network addresses and unintended data. Keep `target/`, `build/` and `dist/` ignored; keep test fixtures synthetic.
- Run the checks in CONTRIBUTING.md, including the declared preview runtimes and Python tests. Use disposable files and an isolated Trash.
- Verify the SSH remote and fetch before pushing. Use an ordinary fast-forward push: `git push origin main`; resolve concurrent work without force-pushing shared history.
- Check the GitHub Actions run for the exact published commit and fix any failing checks.
- Do not create stable version tags, releases or upload binaries merely because source updates were published. Those use the additional checks below.
- Store credentials belong in appropriate CI secrets, never in source or package artifacts.

## Before each binary prerelease

- Run formatting, Clippy, all tests and metadata validation.
- Review RustSec advisories and native runtime dependencies; document remaining issues.
- Review third-party licenses, workspace notices missing from crates, MPL source
  obligations and licenses of embedded fonts. Keep the exact source revision available.
- Build through the packaging workflow or `cargo build --locked --release` and
  `python3 scripts/package.py build`; verify `SHA256SUMS`.
- Test extraction, installation, removal and upgrade on a clean supported distribution.
- Test GNOME and KDE, X11 and Wayland; default app opening, file selection, cut/copy/paste,
  drag and drop, Trash/restore/permanent deletion, mounted shares and preferences.
- Use only disposable files and an isolated Trash during destructive validation.
- Update CHANGELOG, document known limitations, test the actual release executable and
  review English/Portuguese text. Add screenshots made from that release.
- Confirm Cargo version/tag match and commit the reviewed source. Create an annotated
  version tag and a GitHub prerelease only after these checks.
- Attach reviewed artifacts, checksums and notices. CI currently uploads draft workflow
  artifacts only; it does not create GitHub releases or publish to stores.

## Stable releases and stores

Complete the channel-specific gates in DISTRIBUTION.md. Do not mark this build stable
or claim store compatibility on the basis of unit tests. Each package must pass real
installation and native-integration checks. Set an update/security support policy.

AppStream currently labels releases as development and has no screenshot URLs. A human
maintainer must review the OARS declaration, screenshots, public URLs, translations,
release history, identity and metadata before submission.

Flatpak manifests and Flathub submission/review interactions must follow the human-only
parts of Flathub's current AI policy. Acceptance is the store's decision.
