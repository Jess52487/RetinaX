# Frontend Contributing Guidelines

Welcome frontend developers! This document outlines how to contribute to the frontend of the project.
The full reference is the [Website Developer Guide](docs/website-dev-guide.md).

## Tech Stack
- Plain HTML, CSS and JavaScript in `website/`, with no framework and no build step
- Reusable components as UMD scripts in `website/components/` (custom elements or DOM enhancers)
- AOS and GSAP for animation, loaded from CDNs in `website/index.html`
- Node's built-in test runner (`node --test`), ESLint and Prettier for tests and checks

## Setup
1. Install Node.js 20 or later.
2. Run `cd website && npm ci` to install the dev tooling.
3. Serve the site over HTTP, e.g. `python3 -m http.server 8080 --directory website` from the repo root.
4. Before pushing, run `npm run format:check`, `npm run lint` and `npm test` in `website/`.

## Guidelines
- Use semantic HTML.
- Ensure components are accessible.
- Follow the existing code style.
- Follow the [component creation pattern](docs/website-dev-guide.md#creating-a-component) and put logic in exported pure functions with `node --test` coverage.
- Add new user-facing strings to every catalog in `website/i18n/`.
