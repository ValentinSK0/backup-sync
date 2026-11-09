# Security policy

## Supported versions

Security fixes target the latest released version. Reports involving the current `main` branch are also welcome. Include the release tag or commit when reporting a problem.

## Reporting a vulnerability

Ask the maintainer for a private reporting channel through a minimal GitHub issue. Share reproduction details through the agreed private channel. If GitHub displays a **Report a vulnerability** option for this repository, use that private reporting flow.

Keep the initial public request limited to arranging contact. Exclude exploit details, credentials, save files, personal paths, and configuration contents.

Include the following information in the private report.

- Affected release or commit and Windows version
- Steps to reproduce, using disposable files where possible
- Expected and observed behavior
- Potential impact and affected backup or provider operations
- A proposed fix, if available

## Areas covered

Reports can concern local file access, archive extraction, restore paths, backup integrity, credential handling, OAuth, custom cloud endpoints, or remote image handling.

Google OAuth secrets and provider tokens use Windows Credential Manager. Application configuration still contains user paths and provider account labels, so review diagnostic material before sharing it.

Security-related implementation and testing details are documented in the technical guides. [Documentation](../docs/README.md)
