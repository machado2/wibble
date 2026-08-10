## Wibble Operations Runbook

This document covers the release-time operational playbook for the persisted article and translation job system.

## Primary Admin Surface

- Use `/admin/jobs` as the first stop for queue health.
- Review article-job and translation-job status counts before drilling into individual failures.
- Check requester summaries for noisy anonymous keys or accounts with repeated failures or active queues.
- Check rate-limit hit tables and audit summaries to distinguish abuse pressure from provider failure.

## Job Failures

- Open `/admin/jobs` and inspect the failed article-job or translation-job tables.
- Use the stored error summary to separate prompt-policy rejects, model/runtime failures, and image backlog issues.
- Cancel jobs that are clearly stuck or no longer worth retrying.
- For article jobs in `rendering_images`, cancelling stops the job from keeping users on the wait flow while preserving the underlying article record.

## Provider Outages

- If article generation or editing failures spike together, assume the LLM provider is degraded until proven otherwise.
- If jobs are mostly stuck in image-related phases, assume the image provider or image queue is degraded.
- During provider incidents, prefer cancelling backlog jobs rather than allowing an unbounded pile-up.
- Rate-limit and queue telemetry on `/admin/jobs` should be checked before raising quotas or retrying manually.

## Search Outages

- Research mode remains bounded and must fail closed when search or fetch infrastructure is unavailable.
- If future search-backed jobs start failing broadly, treat that as a feature outage rather than silently falling back to fabricated sourcing.
- Do not disable citation or source-traceability rules to keep jobs moving.

## Abusive Traffic Spikes

- Watch requester summaries and rate-limit hit tables for a small set of keys driving disproportionate load.
- Confirm whether the spike is anonymous, authenticated, or admin traffic before changing limits.
- Use the queue and audit summaries to see whether abuse is concentrated on article generation, translation, or edit surfaces.
- Prefer tightening keyed quotas or cancelling abusive backlogs over broad sitewide changes.
