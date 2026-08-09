# Wibble News

[Wibble News](https://wibble.news) is a site where one can generate articles with images using LLM and Stable Diffusion or Dalle. Used mostly for satire, where the LLM being wrong doesn't harm

## Language model

To use the current OpenAI Responses API directly, configure:

```dotenv
OPENAI_API_KEY=your-api-key
OPENAI_MODEL=gpt-5.6-luna
```

`OPENAI_API_URL` is optional and defaults to
`https://api.openai.com/v1/responses` when `OPENAI_API_KEY` is set.
`LANGUAGE_MODEL` can be used instead of `OPENAI_MODEL`, including a
comma-separated fallback list.

OpenRouter-compatible deployments remain supported through
`OPENROUTER_API_KEY`, `OPENROUTER_API_URL`, and `OPENROUTER_MODEL`. When both
provider configurations are present, the OpenAI variables take precedence.
OpenRouter and OpenCode-compatible endpoints continue to use Chat Completions.

## Authentication

Wibble uses `sso.fbmac.net` as an OpenID Connect provider with Authorization
Code and PKCE. Configure the client registered in the SSO with:

```dotenv
SSO_ISSUER_URL=https://sso.fbmac.net/api/auth
SSO_CLIENT_ID=your-client-id
SSO_CLIENT_SECRET=your-client-secret
```

The registered callback URI must be exactly
`https://wibble.fbmac.net/auth/callback`. The application root
`https://wibble.fbmac.net/` must be registered as a post-logout redirect URI.

## Image generation

Replicate is the default image provider. Configure it with:

```dotenv
IMAGE_MODE=replicate
REPLICATE_API_TOKEN=your-replicate-token
```

The default model endpoint is FLUX Schnell:
`https://api.replicate.com/v1/models/black-forest-labs/flux-schnell/predictions`.
Override it with `REPLICATE_API_URL` to use another compatible Replicate model.

The main optional concurrency settings are:

```dotenv
IMAGE_MAX_PARALLEL_PER_ARTICLE=2
REPLICATE_MAX_CONCURRENT_REQUESTS=2
```

`REPLICATE_API_KEY` is not read by the application; the variable name must be
`REPLICATE_API_TOKEN`.

## Image storage

Images are stored on the local filesystem by default. Set `IMAGES_DIR` to
point to the directory where files should be written.

To store images in an S3 compatible bucket instead, configure the following
environment variables and set `STORAGE_TYPE=s3`:

- `S3_ENDPOINT` – optional custom endpoint (e.g. for MinIO)
- `S3_BUCKET_NAME` – target bucket name
- `S3_ACCESS_KEY_ID` – access key ID
- `S3_SECRET_ACCESS_KEY` – secret access key
- `S3_REGION` – optional region, defaults to `us-east-1`

When `STORAGE_TYPE` is unset or set to `local`, the application continues to
write and read images from the local `IMAGES_DIR`.

To migrate existing images from the local directory to the configured S3
bucket, run the helper binary:

```bash
cargo run --bin upload_images
```

Uploaded files are moved to `UPLOADED_DIR` (defaults to `IMAGES_DIR/uploaded`)
so rerunning the command continues from the remaining images.
