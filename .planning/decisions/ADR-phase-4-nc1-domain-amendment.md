---
id: ADR-phase-4-nc1-domain-amendment
title: "NC-1 Amendment: Deployed domain changed to deal-lang.org"
status: accepted
date: 2026-06-06
deciders: ["David Dunnock"]
phase: 04-ecosystem
plan: 04-07
decision_type: amendment
amends: NC-1
---

# NC-1 Amendment: Deployed Domain Changed to deal-lang.org

## Context

The original planning artifact NC-1 specified a `.dev` domain for the DEAL documentation site. This was an aspirational choice at planning time. At execution time, the user confirmed:

1. The `.org` TLD is already owned by the project (`deal-lang.org`).
2. The `.dev` TLD was not owned, making that candidate domain unreliable.
3. `deal-lang.org` is the authoritative project domain (D-64).

## Decision

The deployed domain for the `deal-lang.org` docs site is **`deal-lang.org`**.

Specifically:
- `astro.config.mjs` sets `site: 'https://deal-lang.org'`
- `public/CNAME` contains `deal-lang.org`
- GitHub Pages custom domain is configured to `deal-lang.org`

## Consequences

### Sibling Repository Directory Name

The sibling repository directory is `deal-lang.org/`, matching the owned domain and the deployed site URL.

This means: the repo lives at `../deal-lang.org/` on disk, but the site is served at `https://deal-lang.org/`.

### DNS Setup (User-Side Manual Step)

Setting up the DNS CNAME record is a manual step performed by the domain owner (David Dunnock). The required configuration is:

```
Type:   CNAME
Name:   @  (or www, depending on registrar)
Value:  deal-lang.github.io  (GitHub Pages canonical hostname)
TTL:    3600
```

Additionally, in the GitHub repository settings for `deal-lang/deal-lang.org`:
- Navigate to Settings → Pages
- Set Source to "GitHub Actions"
- Set Custom domain to `deal-lang.org`
- Enable "Enforce HTTPS"

This step is captured in the `user_setup` section of `04-07-PLAN.md`.

### No Impact on CI or Build

The `public/CNAME` file is automatically copied by Astro to `dist/CNAME` during `astro build`, so the GitHub Pages deploy workflow requires no extra step to preserve the custom domain.

## References

- D-64: "Domain is deal-lang.org (NOT .dev). NC-1 amendment ADR required."
- `04-07-PLAN.md` user_setup section: DNS CNAME + GH Pages settings
- `astro.config.mjs` site field (deal-lang.org sibling repo)
- `public/CNAME` (deal-lang.org sibling repo)
