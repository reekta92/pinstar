# Changelog

All notable changes to pinstar are documented in this file.

## [1.1.2] - 2026-10-21

### Added

- Let embedding hosts guard Canvas saves and retain failures

### CI

- Report verified release CI to branch protection
- Adopt identified crates.io publication checks
- Adopt verified packaged policy discovery
- Include Unicode-safe consumer compatibility checks
- Check Clin adapters against library candidates

### Documentation

- Align registry embedding and release credentials with maintained workflows

### Fixed

- Use release archive verification proven with current Cargo
- Carry maintained Mermaid backend to registry consumers

### Miscellaneous

- Bump yaml-rust2 from 0.10.4 to 0.13.0
- Bump uuid from 1.26.1 to 1.28.0
- Bump image from 0.25.9 to 0.25.10
- Bump merman-core from 0.6.0 to 0.9.0
## [1.1.1] - 2026-10-19

### Release

- V1.1.1
## [1.1.0] - 2026-10-19

### Added

- Add bounded read-only Mermaid flowchart previews
- Allow disabling internal hints via Settings
- Align right-click marquee style with graf

### Fixed

- Tokenize YAML safely and bound raw parser work
- Bound Mermaid expansion before semantic parsing
- Inherit host colors in embedded diagram rows
- Preserve local directions and sanitize shape labels
- Retain complete subgraph titles in terminal previews
- Preserve grapheme clusters in Mermaid labels
- Added show_hints field to Settings initializer

### Miscellaneous

- Bump lru from 0.18.4 to 0.18.5
- Bump ratatui-image from 11.0.8 to 11.1.0
- Version bump
- Version bump
- Bump uuid from 1.26.0 to 1.26.1
- Added API.md and updated README.md
- Updated README.md

### Styling

- Cargo fmt

### Build

- Lock non-expanding YAML scanner dependencies
- Audit Mermaid parser dependency licenses

### Release

- V1.1.0
## [1.0.0] - 2026-09-06

### CI

- Fixed release workflow smoke test
- False positive deny checks fixed
- Small behaviour fixes

### Fixed

- Fixed images not rendering

### Styling

- Cargo fmt

### Release

- V1.0.0
- V1.0.0
- V1.0.0
## [0.6.0] - 2026-09-05

### Added

- Extract lib API for host embedding (clin-rs canvas view)

### CI

- Fix permissions for ci workflow
- Use central workflows from .github repo
- General ci fixes for migration
- Routed all ci/cd channels to the central repo

### Fixed

- Marquee drag threshold is strict Manhattan distance (clin parity)

### Miscellaneous

- Add beta labeler workflow caller

### Styling

- Cargo fmt
## [0.5.0] - 2026-05-22

### Fixed

- Fixed footer hints, small EXT fixes
- Fixed footer hints, small EXT fixes

### Miscellaneous

- Update readme and help pages
- Update readme and help pages

### Add

- More QOL for flowcharts
## [0.3.3] - 2026-05-12

### Fixed

- Fixed footer hints, small EXT fixes
## [0.3.2] - 2026-05-12

