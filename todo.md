# Wibble Agent + Refactor Release Plan

This file is the release-blocking plan for the agent-based generation/editing work and the refactors needed to ship it safely.

No release until every phase below is complete or explicitly removed from scope.

## Current Findings

- [x] **Automatic translation is implemented end-to-end**
  - `src/llm/translate.rs` is wired through a translation service and runtime call sites.
  - `language` / `translation` tables back the persisted translation cache.
  - Article pages detect browser language, support explicit per-article overrides, and serve the source article immediately while background translation work runs.
  - Translation jobs are now persisted and resume-safe, with queue priority, retry/backoff, and edit-triggered invalidation/refresh.
  - Remaining translation release work is about operational hardening and browser QA, not missing runtime plumbing.

- [ ] **Research-mode and edit-agent orchestration are still missing**
  - The default generation path now uses a bounded runtime with structured steps, usage accounting, validation, and policy limits.
  - Create and dead-link recovery jobs use persisted `article_job` rows instead of `src/tasklist.rs`.
  - Research, preview/review editing, and user-input pause/resume still need dedicated orchestration.

- [ ] **Abuse controls still need agent-specific hardening**
  - `src/rate_limit.rs` uses keyed, tiered quotas by capability, and generation jobs now persist per-job execution counters and hard budgets.
  - Remaining gaps are admin-facing abuse summaries, moderation policy, and richer operational visibility.

- [ ] **Key refactor hotspots**
  - `src/content.rs` still needs a final cleanup pass around richer article/job metadata.
  - `src/llm/article_generator.rs` still needs research-mode and edit-agent reuse.
  - `templates/*` and `static/style.css` still need shared patterns for job state, previews, and quota notices.

## Release Principles

- [x] Keep anonymous usage cheap, narrow, and heavily limited.
- [x] Put costly or higher-risk features behind login.
- [x] Prefer bounded workflows over autonomous freeform agents.
- [x] Every agent action must be attributable, auditable, and interruptible.
- [x] Every step that mutates content must support preview before publish.
- [x] Do not ship dormant half-features.

## Phase 0: Freeze Scope and Define Release Contracts

- [x] Confirm release scope:
  - Agent-based article generation
  - Agent-based editing from a change description
  - Login incentives and quota tiers
  - Automatic browser-language translation with background generation, persistence, and graceful resume
- [x] Write explicit product rules for:
  - anonymous generation
  - logged-in generation
  - research mode
  - edit agent
  - publish / draft ownership
  - automatic translation defaults, language whitelist, and fallback behavior
- [x] Define hard budgets:
  - max searches per job
  - max fetched pages per job
  - max model/tool calls per job
  - max runtime per job
  - max concurrent jobs by user tier
- [x] Decide whether dead-link recovery is allowed to use agent workflows or must stay simple.

## Phase 1: Core Refactor Before Agent Work

### 1.1 Route and handler decomposition

- [x] Split `src/main.rs` into focused route/handler modules:
  - `routes/public.rs`
  - `routes/create.rs`
  - `routes/content.rs`
  - `routes/edit.rs`
  - `routes/admin.rs`
  - `routes/auth.rs`
- [x] Move shared permission helpers and audit logging out of `main.rs`.
- [x] Keep `main.rs` as router composition + startup only.

### 1.2 Service / repository decomposition

- [x] Split `src/create.rs` into:
  - input validation
  - wait-page rendering
  - dead-link recovery
  - generation orchestration
- [x] Add a create-flow job service layer so create, recovery, and wait handling no longer manipulate task state directly.
- [x] Split `src/content.rs` into:
  - article query / load service
  - rendering helpers
  - comment query / pagination
  - public interaction policy
- [x] Split `src/repository.rs` into:
  - `repositories/articles.rs`
  - `repositories/images.rs`
  - `repositories/examples.rs`
  - `repositories/translations.rs` if translation ships
- [x] Move slug generation into a dedicated article persistence service.

### 1.3 Startup and runtime state cleanup

- [x] Split `src/app_state.rs` into:
  - DB initialization
  - schema compatibility / migrations bridge
  - provider factory
  - background jobs bootstrap
  - runtime registries
- [x] Replace ad hoc schema mutation at startup with a clearer compatibility layer.
- [x] Keep startup `ALTER TABLE` behavior as a temporary compatibility bridge, to be replaced by proper migrations before release.

## Phase 2: Persistent Job Model for Agents

- [x] Replace the current minimal task result model with persisted job state.
- [x] Add a DB-backed job table for long-running work.
- [x] Add explicit job phases such as:
  - `queued`
  - `planning`
  - `researching`
  - `awaiting_user_input`
  - `writing`
  - `editing`
  - `translating`
  - `rendering_images`
  - `ready_for_review`
  - `completed`
  - `failed`
  - `cancelled`
- [x] Store job metadata:
  - requesting user or anonymous key
  - article id
  - prompt
  - feature type
  - usage counters
  - error summary
  - preview payload / draft output
- [x] Keep the single-instance in-memory “currently active” protections, but layer them over persisted job state rather than replacing it.
- [x] Add resume-safe behavior after restart.

## Phase 3: Identity, Ownership, and Quotas

### 3.1 Ownership model

- [x] Audit all content ownership paths.
- [x] Ensure every created article has a coherent owner model.
- [x] Extend edit rights from admin-only to `admin || author`.
- [x] Decide how anonymous articles are owned:
  - chosen approach: anonymous published items with no edit rights
  - logged-in articles are author-owned drafts that require explicit publish
  - forced login remains reserved for future advanced features

### 3.2 Rate limits and anti-abuse

- [x] Replace global generation limits with keyed limits:
  - per-user for authenticated users
  - per-IP / fingerprint key for anonymous users
- [x] Separate quotas by capability:
  - plain article generation
  - research-enabled generation
  - edit-agent requests
  - background translation requests
  - image regeneration
  - clarifying-question loops
- [x] Add queue priority tiers:
  - anonymous: lowest
  - logged-in: normal
  - admin: elevated
- [x] Add server-side cost accounting per job.
- [x] Add upper bounds for prompt size, fetched content size, and number of agent steps.
- [ ] Add abuse monitoring and audit summaries.

### 3.3 Login incentives

- [x] Decide the logged-in feature bundle:
  - higher quotas
  - research mode
  - saved drafts
  - agent editing
  - article ownership and publish controls
  - richer translation controls if needed beyond the default browser-language behavior
- [x] Update create / wait / edit UI to explain why login unlocks more capability.

## Phase 4: Agent Runtime Architecture

- [x] Add a bounded agent orchestration layer instead of embedding logic directly in handlers.
- [ ] Define structured tools available to the generation agent:
  - article planning
  - limited web search / fetch
  - draft writer
  - image brief planner
  - self-critique / policy check
- [ ] Define structured tools available to the edit agent:
  - load article
  - apply requested change
  - summarize diff
  - propose title / dek / body edits
- [x] Build a strict execution policy:
  - no arbitrary URLs from anonymous users
  - no unbounded loops
  - capped search count
  - capped source count
  - no hidden mutation without preview
- [x] Add structured logs for every step the agent takes.

## Phase 5: Agent-Based Article Generation

### 5.1 Default generation path

- [x] Keep a cheap default path for prompts that do not need browsing.
- [x] Use the current prompt-based generation as the baseline non-research path.
- [x] Refactor `src/llm/article_generator.rs` into:
  - prompt builder
  - planning
  - article draft generation
  - image brief generation
  - output validation

### 5.2 Research mode

- [ ] Add a bounded research mode for prompts involving:
  - current events
  - named public institutions
  - public figures
  - real policies
  - real organizations
- [ ] Define when research mode is:
  - auto-enabled
  - manually requested
  - login-gated
- [ ] Add source handling rules:
  - prioritize primary or reputable sources
  - keep only brief extracted context
  - store citations for internal traceability
- [ ] Add a “source-aware satire” prompt layer so tone remains deadpan instead of turning into summary prose.

### 5.3 Clarifying question flow

- [ ] Allow the agent to ask at most one clarifying question per job unless the user is admin.
- [ ] Only allow clarification when ambiguity materially changes the article.
- [ ] Add a job state and UI for pending questions.
- [ ] Add timeout / fallback behavior if the user does not answer.

### 5.4 Output validation

- [x] Validate generated articles for:
  - title present
  - minimum structure
  - deadpan tone rules
  - image tag count or image brief count
  - no forbidden markup
  - no direct prompt leakage
- [x] Add policy checks for real-person / defamation / high-risk content.

## Phase 6: Agent-Based Editing

- [x] Add a new edit workflow based on a user-supplied change description.
- [x] Support requests like:
  - “make it drier”
  - “shorten by 30%”
  - “replace the third section with a parliamentary reaction”
  - “rewrite in a stricter bulletin tone”
- [x] Edit flow must be:
  - load current article
  - generate revised draft
  - produce summary of changes
  - show preview / diff
  - require explicit apply
- [x] Persist change request, agent summary, and final apply action to `audit_log`.
- [x] Gate agent editing to logged-in owners/admins.
- [x] Consider keeping raw markdown editing for admins as an escape hatch.

## Phase 7: Prompt System Cleanup

- [x] Introduce explicit prompt versioning for:
  - article generation
  - placeholder generation
  - image brief generation
  - translation
- [ ] Extend prompt versioning to:
  - research-enabled generation
  - [x] edit-agent rewriting
- [x] Move prompt assembly rules out of scattered string constants into a prompt module.
- [x] Add tests or fixtures that validate prompt structure and output parsers.
- [x] Store the prompt version used on each generated article.

## Phase 8: Automatic Translation

### 8.1 Product rules

- [x] Define the source-language model for articles:
  - article language is the generated language
  - English is the baseline fallback if browser-language translation is unavailable
- [x] Detect the user's preferred language from the browser request.
- [x] Restrict translation targets to a curated whitelist of languages the model handles well.
- [x] Define fallback rules:
  - if browser language is unsupported, serve English or the source article immediately
  - if translation is missing or in progress, do not block the page; serve the source article immediately
  - once translation is ready, the preferred variant should be served automatically when appropriate

### 8.2 Translation persistence and cache model

- [x] Use `language` / `translation` tables for actual runtime persistence.
- [x] Add a translation service instead of leaving `src/llm/translate.rs` orphaned.
- [x] Persist translations so repeat reads do not re-run the model.
- [x] Cache by:
  - article id
  - source content revision / hash
  - target language
  - translation prompt version
- [x] Invalidate cached translations when the source article changes.
- [x] Decide whether translations are stored as:
  - chosen approach: title + description + markdown fields
  - not full rendered markdown/body copies
  - not a dedicated translation aggregate yet

### 8.3 Background generation and resume behavior

- [x] Translation must happen asynchronously in the background when a requested language variant is missing.
- [x] Add translation jobs with dedicated quotas and queue priority.
- [x] Add resume-safe translation jobs so work can continue gracefully after server crash or stop.
- [x] Ensure half-finished translations do not corrupt the cache or block reads.
- [x] Add idempotent retry rules for failed translations.
- [x] Keep translation state persisted so the server can recover mid-flight work after restart.

### 8.4 Serving behavior

- [x] When a new translation is needed, serve the source article immediately instead of making the user wait.
- [x] Prefer the browser language only when:
  - it is supported
  - the user has not overridden the preference
  - a cached translation exists, or the source-language fallback is acceptable while translation is being generated
- [x] Define precedence between:
  - explicit user toggle choice
  - browser default language
  - article source language
  - English fallback

### 8.5 UI and preference controls

- [x] Add a language toggle control on article pages.
- [x] Decide whether the toggle is:
  - icon-only
  - icon + text
  - compact menu
- [x] Match the control to the deadpan editorial UI rather than making it look like a consumer app language picker.
- [x] Remember non-default language preference in a cookie.
- [x] Do not write a cookie when the current state matches the automatic browser-language default.
- [x] Define whether the preference is article-page only or site-wide.

### 8.6 Safety and abuse controls

- [x] Restrict automatic translation to the supported language whitelist.
- [x] Do not allow user-supplied arbitrary target language strings.
- [x] Rate-limit translation creation separately from article generation.
- [x] Prevent translation spam from anonymous traffic by deduplicating in-flight translation jobs per article/language.

### 8.7 Editing and translation coherence

- [x] Decide how agent edits interact with existing translations.
- [x] Mark translations stale when the source article is edited.
- [x] Re-queue background translation refresh after edits.
- [x] Add auditability for translation generation and invalidation.

## Phase 9: UI and Product Flow

- [ ] Update create UI for mode selection:
  - standard draft
  - research-enabled draft
  - maybe “requires login”
- [ ] Add job-status UI for multi-step agent states.
- [ ] Add question/answer UI for clarifying prompts.
- [x] Add preview/diff UI for agent edits.
- [ ] Add quota messaging and login upsell copy.
- [ ] Add translation toggle UI and fallback messaging that does not interrupt reading.
- [ ] Add article metadata display if research mode used internally and that becomes product-relevant.

## Phase 10: Safety, Moderation, and Editorial Policy

- [x] Define stricter rules for real people and real allegations.
- [ ] Prevent the research agent from confidently fabricating citations or facts.
- [x] Add moderation rules for user prompts that target private individuals.
- [x] Add policy handling for risky contemporary events.
- [x] Add internal guidance for “deadpan but not defamatory”.

## Phase 11: Testing and Verification

### 11.1 Automated tests

- [ ] Expand beyond the current lightweight test coverage.
- [ ] Add unit tests for:
  - keyed quotas
  - ownership checks
  - job state transitions
  - agent output validation
  - prompt version selection
  - edit diff application
  - translation caching and invalidation
  - browser-language selection with whitelist fallback
  - preference cookie behavior
- [ ] Add integration tests for:
  - anonymous generation limits
  - logged-in quota differences
  - research-mode job lifecycle
  - question/answer pause + resume
  - edit-agent preview + apply
  - publish/unpublish after edits
  - translation request -> source fallback -> background completion -> translated serve
  - server restart during translation job
- [ ] Add template parse coverage and browser smoke coverage for new flows.

### 11.2 Manual verification

- [ ] Verify recovery behavior after process restart while jobs are mid-flight.
- [ ] Verify stale DB rows do not strand users on wait pages.
- [ ] Verify article ownership and author editing.
- [ ] Verify rate-limit messaging and login incentives.
- [ ] Verify translation fallback, toggle, cookie persistence, and crash recovery.

## Phase 12: Release Hardening

- [ ] Add metrics and logs for:
  - job counts by state
  - agent cost by feature
  - search usage
  - edit usage
  - rate-limit hits by tier
- [ ] Add admin visibility into queued / failed jobs.
- [ ] Add kill / cancel capability for stuck jobs.
- [ ] Document operational runbooks for:
  - job failures
  - provider outages
  - search outages
  - abusive traffic spikes
- [ ] Re-run design pass once new agent surfaces are in place.

## Refactor Map by File

- [x] `src/main.rs`
  - break apart into route modules and handler modules
  - move permission helpers and audit logging out
  - keep route table + startup only

- [x] `src/create/`
  - split orchestration, wait logic, recovery, and form rendering
- [x] create flow job-oriented service layer

- [ ] `src/content.rs`
  - separate article loading, rendering, comments, and interaction policy
  - prepare for richer article/job metadata

- [x] `src/repository.rs`
  - split by aggregate / concern
  - remove mixed storage + examples + article persistence responsibilities

- [x] `src/app_state.rs`
  - separate startup concerns
  - isolate provider factory and schema compatibility code

- [x] `src/rate_limit.rs`
  - replace global counters with keyed quota service
  - add tier-aware policies

- [x] `src/tasklist.rs`
  - replace with richer persisted job model
  - preserve in-memory active tracking as an optimization only

- [ ] `src/llm/article_generator.rs`
  - [x] split planning, generation, parsing, validation, and image brief generation
  - add support for research mode and edit-agent reuse

- [x] `src/llm/translate.rs`
  - [x] move from orphan helper to a real translation service entry point
  - [x] add structured target-language whitelist and prompt versioning

- [x] `prompts/*`
  - introduce versioning and a prompt registry
  - stop relying on scattered include_str files without metadata

- [ ] `templates/*` and `static/style.css`
  - once the new agent flows exist, extract repeating UI patterns for:
    - job status
    - agent question prompts
    - diffs / previews
    - quota / login notices

## Release Checklist

- [ ] Core refactor completed
- [x] Persisted job system completed
- [x] Keyed quotas completed
- [x] Ownership + author editing completed
- [ ] Agent generation completed
- [x] Agent editing completed
- [x] Translation decision completed
- [x] Automatic translation implementation completed
- [ ] Safety / moderation completed
- [ ] Test coverage completed
- [ ] Operational controls completed
- [ ] Final browser QA completed
