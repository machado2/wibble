use std::env;

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};
use aws_sdk_s3::config::{Region, Credentials, BehaviorVersion};
use crate::error::Error;

async fn get_client() -> Result<Client, Error> {
    let access_key = env::var("S3_ACCESS_KEY_ID")
        .map_err(|_| Error::Storage("S3_ACCESS_KEY_ID not set".to_string()))?;
    let secret_key = env::var("S3_SECRET_ACCESS_KEY")
        .map_err(|_| Error::Storage("S3_SECRET_ACCESS_KEY not set".to_string()))?;
    let region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let endpoint = env::var("S3_ENDPOINT").ok();

    let credentials = Credentials::new(access_key, secret_key, None, None, "env");
    let region = Region::new(region);

    let mut config_builder = Config::builder()
        .behavior_version(BehaviorVersion::latest())

        .region(region)
        .credentials_provider(credentials);
    if let Some(ep) = endpoint {
        config_builder = config_builder.endpoint_url(ep);
    }
    let config = config_builder.build();
    Ok(Client::from_conf(config))
}

pub async fn upload_image(id: &str, data: Vec<u8>) -> Result<(), Error> {
    let client = get_client().await?;
    let bucket = env::var("S3_BUCKET_NAME")
        .map_err(|_| Error::Storage("S3_BUCKET_NAME not set".to_string()))?;
    client
        .put_object()
        .bucket(bucket)
        .key(format!("{}.jpg", id))
        .body(ByteStream::from(data))
        .send()
        .await
        .map_err(|e| Error::Storage(format!("S3 upload failed: {}", e)))?;
    Ok(())
}

pub async fn download_image(id: &str) -> Result<Vec<u8>, Error> {
    let client = get_client().await?;
    let bucket = env::var("S3_BUCKET_NAME")
        .map_err(|_| Error::Storage("S3_BUCKET_NAME not set".to_string()))?;
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(format!("{}.jpg", id))
        .send()
        .await
        .map_err(|e| Error::Storage(format!("S3 download failed: {}", e)))?;
    let data = resp
        .body
        .collect()
        .await
        .map_err(|e| Error::Storage(format!("S3 read failed: {}", e)))?;
    Ok(data.into_bytes().to_vec())
}
