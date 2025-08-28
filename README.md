# Wibble News

[Wibble News](https://wibble.news) is a site where one can generate articles with images using LLM and Stable Diffusion or Dalle. Used mostly for satire, where the LLM being wrong doesn't harm

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