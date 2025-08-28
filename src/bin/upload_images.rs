use std::{env, fs};

use dotenvy::dotenv;

#[path = "../s3.rs"]
mod s3;
#[path = "../error.rs"]
mod error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let images_dir = env::var("IMAGES_DIR")?;
    for entry in fs::read_dir(&images_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("jpg") {
            let id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => {
                    eprintln!("Skipping file with invalid name: {:?}", path);
                    continue;
                }
            };
            match fs::read(&path) {
                Ok(data) => match s3::upload_image(id, data).await {
                    Ok(_) => println!("Uploaded {}", id),
                    Err(e) => eprintln!("Failed to upload {}: {}", id, e),
                },
                Err(e) => eprintln!("Failed to read {:?}: {}", path, e),
            }
        }
    }
    Ok(())
}

