## Wibble Editorial Policy

This document defines the release-time safety rules enforced around article generation and agent-based editing.

## Blocked Request Classes

- Prompts and edit requests must be rejected when they target private individuals.
- Prompts and edit requests must be rejected when they frame real-person allegations.
- Prompts and edit requests must be rejected when they center high-risk contemporary violence or atrocity topics.

These checks are enforced before generation starts and before an edit-agent preview is produced.

## Private Individuals

- Wibble does not generate satire about private citizens, acquaintances, coworkers, classmates, or similar non-public individuals.
- “Private individual” includes prompts framed around personal relationships or local, non-public targets.
- If a request appears to single out a private person, the system should fail closed rather than trying to rewrite around it.

## Real People and Allegations

- Satire about real people must not hinge on unverified criminal, sexual, or reputational allegations.
- The system should reject requests that combine person-identifying language with allegation framing.
- Generated article output is re-checked before persistence so a draft cannot be saved if it crosses this line.

## Risky Contemporary Events

- Requests centered on events such as mass shootings, terrorist attacks, hostage crises, genocide, or ethnic cleansing are blocked.
- Output that drifts into those topics must also be rejected before save/apply.
- This policy is intentionally narrow and fail-closed for clearly high-risk event classes.

## Deadpan but Not Defamatory

- The voice should stay dry and bulletin-like.
- The voice should not rely on defamatory claims, insinuations, or allegation-forward framing about real people.
- If deadpan tone and safety conflict, safety wins and the request should fail rather than stretch the joke.
