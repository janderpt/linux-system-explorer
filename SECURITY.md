# Security

The project is currently experimental. There is no supported stable release or formal
security-update commitment yet. File operations run with the permissions of the user;
the native package is not a sandbox and should not be run as root.

Report vulnerabilities privately through GitHub Security Advisories if private vulnerability reporting is enabled in the repository. Otherwise, contact the
maintainer through an established private channel. Do not disclose personal paths,
file contents, credentials or exploitable details in a public issue.

Before each public binary release, maintainers must review RustSec advisories, native
runtime dependencies, licensing reports and file-operation regressions. A successful
unit-test run or generated dependency inventory is not a security or license audit.
