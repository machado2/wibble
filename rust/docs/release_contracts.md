# Wibble Release Contracts

This document freezes the product rules and operating limits for the bounded generation and editing work tracked in `todo.md`.

## Release Scope

- Agent-assisted article generation remains in scope, but only as a bounded workflow.
- Agent-assisted editing from a change description remains in scope, but only for logged-in owners and admins with preview-before-apply.
- Login incentives and quota tiers remain in scope.
- Automatic browser-language translation with background generation, persistence, and graceful resume remains in scope.

## Release Principles

- Anonymous usage stays cheap, narrow, and heavily limited.
- Costly or higher-risk features stay behind login.
- Workflows stay bounded rather than open-ended.
- Every agent action must be attributable, auditable, and interruptible.
- Every content mutation must support preview before publish.
- Dormant half-features should not be exposed.

## Product Rules

### Anonymous generation

- Anonymous users get the standard article-generation path only.
- Anonymous requests must not trigger research browsing or agent editing.
- Successful anonymous articles publish immediately.
- Anonymous articles have no retained owner and no follow-up edit rights.
- Anonymous traffic uses the lowest queue priority and the tightest quotas.

### Logged-in generation

- Logged-in users get higher quotas, private drafts, owner editing, image replacement, and publish controls.
- Logged-in generation defaults to draft creation rather than immediate publication.
- Logged-in users are the minimum tier for future research mode and edit-agent access.

### Research mode

- Research mode is login-gated.
- Research mode is only allowed for prompts involving current events, named institutions, public figures, policies, or organizations.
- Research mode must stay bounded by the hard budgets below and keep source traceability for internal review.
- Research mode output still has to read like deadpan satire rather than a factual digest.

### Edit agent

- Edit-agent access is limited to logged-in article owners and admins.
- The edit agent must load the current article, generate a revised draft, summarize the changes, show a preview/diff, and require explicit apply.
- Preview does not mutate the stored article or translation cache.
- Admin raw-markdown editing remains the escape hatch if the agent path misbehaves.

### Publish and draft ownership

- Logged-in articles are author-owned drafts until explicitly published.
- Authors and admins can edit author-owned drafts and published articles they own.
- Admins can always override publish state.
- Anonymous articles remain public, unowned, and non-editable.

### Automatic translation defaults

- The source article remains authoritative.
- Browser language is used automatically only when it is in the supported whitelist and the user has not overridden the preference.
- If a translation is missing or still generating, the source article is served immediately.
- Explicit article-language overrides beat browser defaults.
- The whitelist remains the curated language set exposed by the prompt registry.

## Hard Budgets

- Max searches per job: `3`
- Max fetched pages per job: `5`
- Max model/tool calls per job: `12`
- Max runtime per job: `90s`
- Max concurrent jobs by user tier:
  - Anonymous: `1`
  - Authenticated: `2`
  - Admin: `4`

These are release contracts, not suggestions. If implementation cannot enforce them, the related surface is not ready to ship.

## Dead-Link Recovery

- Dead-link recovery must stay simple.
- It may reuse the standard bounded article generator, but it must not invoke browsing, question loops, or open-ended agent workflows.
- Recovery should continue to infer a prompt from the slug, create a placeholder row, and resume safely after restart.

## Logged-In Feature Bundle

- Higher keyed quotas.
- Private draft generation.
- Owner editing and image replacement.
- Publish and unpublish controls.
- Saved article-language preference behavior.
- Login-gated access to future research mode and edit-agent features.

## Translation Policy for Agent Edits

- Preview-only edit proposals do not touch translations.
- Applying an agent edit invalidates cached translations for the previous source revision.
- The source-language article remains the only authoritative stored draft.
- Translation refresh happens asynchronously after apply, using the same stale-marking and requeue behavior as manual edits.
