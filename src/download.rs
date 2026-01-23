use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tar::Archive;

/// Download a file from a URL to a local path with progress reporting
pub async fn download_file_with_progress(
    client: &Client,
    url: &str,
    path: &Path,
) -> Result<(), String> {
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to GET from '{}': {}", url, e))?;

    if !res.status().is_success() {
        return Err(format!(
            "HTTP error {} when downloading from '{}'",
            res.status(),
            url
        ));
    }

    let total_size = res.content_length();

    // Create progress bar
    let pb = if let Some(size) = total_size {
        let bar = ProgressBar::new(size);
        bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        bar.set_message(format!("Downloading {}", url));
        Some(bar)
    } else {
        // No content length, use spinner
        let bar = ProgressBar::new_spinner();
        bar.set_message(format!("Downloading {} (size unknown)", url));
        Some(bar)
    };

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory '{}': {}", parent.display(), e))?;
    }

    let mut file = File::create(path)
        .map_err(|e| format!("Failed to create file '{}': {}", path.display(), e))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| format!("Error while downloading file: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Error while writing to file: {}", e))?;

        downloaded += chunk.len() as u64;

        if let Some(ref bar) = pb {
            if let Some(size) = total_size {
                let new = min(downloaded, size);
                bar.set_position(new);
            } else {
                bar.set_message(format!("Downloading {} ({} bytes)", url, downloaded));
            }
        }
    }

    if let Some(bar) = pb {
        bar.finish_with_message(format!("Downloaded {} to {}", url, path.display()));
    }

    Ok(())
}

/// Download a file and verify its SHA256 checksum
pub async fn download_and_verify(
    client: &Client,
    url: &str,
    path: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    // Download the file
    download_file_with_progress(client, url, path).await?;

    // Verify checksum
    verify_sha256(path, expected_sha256)?;

    Ok(())
}

/// Verify SHA256 checksum of a file
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| {
        format!(
            "Failed to open file for verification '{}': {}",
            path.display(),
            e
        )
    })?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| {
        format!(
            "Failed to read file for verification '{}': {}",
            path.display(),
            e
        )
    })?;
    let hash = hasher.finalize();
    let hash_hex = hex::encode(hash);

    if hash_hex.to_lowercase() != expected.to_lowercase() {
        return Err(format!(
            "SHA256 verification failed for '{}'\n  Expected: {}\n  Got: {}",
            path.display(),
            expected,
            hash_hex
        ));
    }

    Ok(())
}

/// Calculate SHA256 hash of a URL for cache path generation (matching Homebrew)
pub fn sha256_url(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

/// Extract a tar.gz archive to a destination directory
pub fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|e| format!("Failed to open archive '{}': {}", archive_path.display(), e))?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // Create destination directory if it doesn't exist
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create directory '{}': {}", dest_dir.display(), e))?;

    archive.unpack(dest_dir).map_err(|e| {
        format!(
            "Failed to extract archive to '{}': {}",
            dest_dir.display(),
            e
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_url() {
        // Test URL hashing matches Homebrew's format
        let url = "https://ghcr.io/v2/homebrew/core/wget/blobs/sha256:8cbb5d277cd207e543c9b2e75953e89c7cc89105b2322f3ce652616c5d0f62fe";
        let hash = sha256_url(url);
        // Just verify it's a valid 64-char hex string
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
