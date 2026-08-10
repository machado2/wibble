use std::{env, fs, path::Path};

use dotenvy::dotenv;

use wibble::s3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let images_dir = env::var("IMAGES_DIR")?;
    let uploaded_dir = env::var("UPLOADED_DIR").unwrap_or_else(|_| {
        Path::new(&images_dir)
            .join("uploaded")
            .to_string_lossy()
            .into_owned()
    });
    fs::create_dir_all(&uploaded_dir)?;
    for entry in fs::read_dir(&images_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jpg") {
            let id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => {
                    eprintln!("Skipping file with invalid name: {:?}", path);
                    continue;
                }
            };
            match fs::read(&path) {
                Ok(data) => match s3::upload_image(id, data).await {
                    Ok(_) => {
                        println!("Uploaded {}", id);
                        if let Some(filename) = path.file_name() {
                            if let Err(e) =
                                fs::rename(&path, Path::new(&uploaded_dir).join(filename))
                            {
                                eprintln!("Failed to move {}: {}", id, e);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to upload {}: {}", id, e),
                },
                Err(e) => eprintln!("Failed to read {:?}: {}", path, e),
            }
        }
    }
    Ok(())
}
