## 1) Core Features
- Article generation
  - Accept a free-form prompt from the user to generate a news-style article.
  - Use an LLM to produce a structured article with a title and multiple paragraphs.
  - Derive multiple image prompts from the article; generate images for the article.
  - Persist the article content, metadata, and generated images.
- Article browsing and viewing
  - List recent articles with title, description (first paragraph), date, and cover image.
  - Paginate article listings with optional search and sorting modes (newest, hot, most viewed) and time filters (week, month).
  - View an individual article rendered from markdown to HTML.
- Image gallery and details
  - List image thumbnails, paginated.
  - View image details including parameters, associated article, and creation date.
- Background task tracking
  - Fire-and-forget job for article generation with a unique ID.
  - Polling/wait page that refreshes until the article is ready or fails.
- Static assets and theming
  - Serve static assets (CSS/JS/images) and allow style (theme) selection via query parameter.

## 2) User Interface Requirements
- Global layout
  - Base layout with navigation bar including a “Create new article” CTA.
  - SEO/OG tags in head; per-page title and description fields.
  - Themeable stylesheet link with cache-busting support.
- Homepage (index)
  - Grid/list of cards for articles:
    - Thumbnail image, title (links to article), date, short description.
  - “Load More Articles” button; respects pagination using afterId query param.
  - Optional filters:
    - Search term (applies to slug, title, description, full content).
    - Time filter (week or month).
    - Sort (newest default; hot; most_viewed).
- Create page (form)
  - Textarea for prompt input; Submit posts to create endpoint.
- Wait page
  - Shows “Generating article...” and auto-refreshes every 5 seconds.
- Article page
  - Displays title, date, and rendered article body (markdown transformed to HTML).
  - Images in the body are clickable and navigate to the image info page.
- Images page (gallery)
  - Grid of image thumbnails, each linking to the image details page.
  - “Next Page” button using last_id pagination.
- Image info page
  - Large image preview.
  - JSON-like pretty-printed parameters if available; fallback to raw text.
  - Link back to associated article and shows article title.
- Error views
  - Nice error UI for 404 and 500 with a themed image and friendly message.

## 3) Data Management
- Primary entities
  - Content (Article)
    - Fields include: id (UUID), slug (unique), content/markdown, created_at, generating flags/timestamps, flagged, LLM model used, prompt version, fail count, description (first paragraph), image_id (cover image), title, user_input (instructions), view_count, votes, hot_score, generation_time_ms, optional external IDs, long/umami view counters.
  - ContentImage
    - Fields: id, content_id (FK), prompt_hash (optional), prompt (text), alt_text, created_at, flags (flagged, regenerate), fail_count, generator, model, seed, parameters (JSON string optional), view_count.
  - Examples (used for LLM few-shot examples)
    - Stores previous prompts/contents/titles; random sampling by numeric surrogate key.
- Derived/presentation data
  - Headline projection for listings: id, slug, created_at, description, image_id, title; date formatted as YYYY-MM-DD.
- Storage
  - Images stored either:
    - Local filesystem at IMAGES_DIR as {id}.jpg, or
    - S3-compatible bucket with key {id}.jpg.
- Data flow
  - On create: generate ID; spawn async generation task; once completed, insert content and images transactionally; update content with cover image (first image’s id).
  - Listing queries apply filters/sorting and cursor-like pagination via afterId.

## 4) Business Logic
- Article generation workflow
  - Select LLM model from configured list, retry up to 3 attempts; may include example pairs on retries or via a USE_EXAMPLES flag.
  - Two modes:
    - Placeholders mode: LLM returns text with <GeneratedImage prompt="" alt=""/> tags which are replaced with image markdown entries after generating images.
    - Regular mode: LLM returns full article; then generate image prompts via a separate “illustrator” system message; split article into paragraphs; interleave generated images across paragraphs; any remaining images appended after text.
  - Title extraction:
    - Parse first non-empty line, strip leading formatting like #, “Title:”.
- Markdown rendering adjustments
  - Convert markdown to HTML with a custom renderer.
  - Wrap image nodes in anchor links to the image info page.
- Pagination rules
  - Page size default 20 (max <100).
  - Time filters: last 7 days (week) or ~30 days (month).
  - Sorting:
    - newest (created_at desc),
    - hot (hot_score desc),
    - most_viewed (view_count desc).
  - Cursor behavior:
    - afterId points to a reference row; listing then returns items with sort_key < ref.sort_key OR tie-broken by id.
- Error handling
  - If image generator censors or rate-limits, surface meaningful error states.
  - Task result states: Processing, Success, Error; wait page behavior based on state.
- Slug handling
  - Slug generated from title; if collision, fallback to UUID.
- Image parameters formatting
  - If parameters parse as JSON, pretty-print; otherwise display raw text.

## 5) Integration Points
- LLM (text generation)
  - OpenRouter API for chat completions.
  - Inputs: messages (system/user/assistant), temperature, penalties, stop tokens.
  - Outputs: assistant content; may include tool/function call arguments for specific flows.
  - Configurable models via environment variable with comma-separated list.
- Image generation
  - Pluggable providers, selected via IMAGE_MODE:
    - Replicate
    - Stability (SD)
    - AI Horde
    - Hugging Face
    - Fallback and retry wrappers included in composition.
  - For each ImageToCreate, request image creation; collect bytes and parameter metadata.
- Storage services
  - Local filesystem reads/writes under IMAGES_DIR.
  - S3-compatible object storage:
    - Put/Get object {id}.jpg in configured bucket, region, endpoint; credentials from env.
  - Batch uploader utility to migrate local images to S3.
- Database
  - Relational database accessible via configured DATABASE_URL; CRUD for content/images/examples; transactional writes for article and its images.
- Static file serving
  - Serve templates and static assets directory; cache-busted stylesheet resolution via a bust-dir fingerprint.

## 6) Authentication & Authorization
- None required for the current application:
  - All main routes are public (create, list, content, images, image info).
  - No user accounts or roles observed.
  - If reimplementing with auth, treat as future extension (not required).

## 7) Configuration & Settings
- Environment variables
  - DATABASE_URL: connection string to relational DB.
  - LANGUAGE_MODEL: comma-separated list of LLM model identifiers.
  - OPENROUTER_API_KEY: API key for LLM.
  - IMAGE_MODE: selects image provider chain [sd3 | horde | huggingface | default(replica+retry)].
  - STORAGE_TYPE: local | s3.
  - IMAGES_DIR: local directory for image files (local mode).
  - S3_ENDPOINT (optional), S3_BUCKET_NAME, S3_ACCESS_KEY_ID, S3_SECRET_ACCESS_KEY, S3_REGION (default us-east-1).
  - PORT: server port, default 8000.
  - USE_EXAMPLES: "true" to enable example-augmented prompts; otherwise only on retries.
  - USE_PLACEHOLDERS: "true" to use placeholder-driven image insertion flow.
  - UPLOADED_DIR: destination for uploaded images after S3 migration utility runs (defaults to IMAGES_DIR/uploaded).
- Templating and theming
  - Templates loaded from templates/**/*.
  - Theme selected via query parameter “theme” mapping to a CSS path (e.g., /style.css).
  - Cache busting for style path via static directory hashing.

## 8) Performance Requirements
- Async processing and non-blocking I/O
  - Background task for article generation; UI polling with lightweight wait endpoint.
  - Concurrent image generation with per-image async tasks and join/wait pattern to collect results.
- Pagination limits
  - Page sizes enforced (<100) to keep list queries efficient.
- Caching/minification
  - Not explicitly required beyond static asset serving and cache-busting; can be considered an enhancement.
- Observability
  - Logging/tracing of LLM token usage and finish reasons; basic diagnostics sufficient for monitoring.

## 9) Error Handling
- HTTP-level responses
  - 404 and 500 return friendly HTML pages with themed images and messages.
  - Other errors default to 500 with error text; rate-limited maps to 429.
- Domain errors
  - Database errors captured with human-readable messages.
  - LLM errors (missing content, parse failures, generation shortfalls) surfaced as internal errors.
  - Image generator errors:
    - Censorship -> ImageCensored.
    - Rate limiting -> RateLimited.
  - Storage errors when S3/local IO operations fail, with descriptive messages.
- Resilience
  - LLM retries across configured models (up to 3 attempts).
  - Image generation provider fallback/retry modes depending on IMAGE_MODE.

## 10) Deployment & Infrastructure
- Web server
  - HTTP server exposing routes:
    - GET / -> article list (accepts form/query parameters: afterId, pageSize, search, t, sort)
    - GET /content/{slug} -> article page
    - GET /image/{id} -> returns image/jpeg
    - GET /image_info/{id} -> image info page
    - GET /images -> image gallery with pagination
    - GET/POST /create -> form and submission for new article
    - GET /wait/{id} -> wait/poll endpoint returning either redirect to content, HTML wait page, or 404/500 status
  - Serve static files from a static directory; cache busting supported by a hash map over filenames.
- Database
  - Requires a Postgres-compatible URL (by SeaORM/sqlx feature flags); schema aligns with provided entity structs.
- External services
  - Outbound HTTPS to OpenRouter and the selected image provider API(s).
  - Optional S3-compatible object storage endpoint.
- CLI utility
  - upload_images: migrates local images in IMAGES_DIR to S3 and moves uploaded files into UPLOADED_DIR.
- Environment management
  - .env support; variables loaded at startup.

