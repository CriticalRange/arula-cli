//! Android filesystem implementation using scoped storage

use crate::platform::android::AndroidContext;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::fs;

/// Android filesystem backend with scoped storage support
pub struct AndroidFileSystem {
    ctx: AndroidContext,
    base_path: Arc<Mutex<PathBuf>>,
}

impl AndroidFileSystem {
    pub fn new(ctx: AndroidContext) -> Self {
        Self {
            ctx,
            base_path: Arc::new(Mutex::new(PathBuf::from("/storage/emulated/0"))),
        }
    }

    /// Set the base storage directory
    pub async fn set_base_path<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut base_path = self.base_path.lock().await;
        *base_path = path.as_ref().to_path_buf();
        Ok(())
    }

    /// Read file content
    pub async fn read_file<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        let content = fs::read_to_string(&full_path).await
            .map_err(|e| anyhow::anyhow!("Failed to read file {:?}: {}", full_path, e))?;

        Ok(content)
    }

    /// Write file content
    pub async fn write_file<P: AsRef<Path>>(&self, path: P, content: &str) -> Result<()> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| anyhow::anyhow!("Failed to create directory {:?}: {}", parent, e))?;
        }

        fs::write(&full_path, content).await
            .map_err(|e| anyhow::anyhow!("Failed to write file {:?}: {}", full_path, e))?;

        Ok(())
    }

    /// List directory contents
    pub async fn list_dir<P: AsRef<Path>>(&self, path: P) -> Result<Vec<String>> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        let mut entries = Vec::new();
        let mut dir = fs::read_dir(&full_path).await
            .map_err(|e| anyhow::anyhow!("Failed to read directory {:?}: {}", full_path, e))?;

        while let Some(entry) = dir.next_entry().await.map_err(|e| {
            anyhow::anyhow!("Error reading directory entry: {}", e)
        })? {
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(name);
        }

        entries.sort();
        Ok(entries)
    }

    /// Check if path exists
    pub async fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);
        tokio::fs::metadata(full_path).await.is_ok()
    }

    /// Get file metadata
    pub async fn metadata<P: AsRef<Path>>(&self, path: P) -> Result<FileMetadata> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        let meta = fs::metadata(&full_path).await
            .map_err(|e| anyhow::anyhow!("Failed to get metadata for {:?}: {}", full_path, e))?;

        Ok(FileMetadata {
            is_dir: meta.is_dir(),
            is_file: meta.is_file(),
            size: meta.len(),
            modified: meta.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs()),
        })
    }

    /// Create directory
    pub async fn create_dir<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        fs::create_dir_all(&full_path).await
            .map_err(|e| anyhow::anyhow!("Failed to create directory {:?}: {}", full_path, e))?;

        Ok(())
    }

    /// Delete file or directory
    pub async fn delete<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let base_path = self.base_path.lock().await;
        let full_path = base_path.join(path);

        let meta = fs::metadata(&full_path).await;

        if meta.is_ok() {
            if meta.unwrap().is_dir() {
                fs::remove_dir_all(&full_path).await
                    .map_err(|e| anyhow::anyhow!("Failed to remove directory {:?}: {}", full_path, e))?;
            } else {
                fs::remove_file(&full_path).await
                    .map_err(|e| anyhow::anyhow!("Failed to remove file {:?}: {}", full_path, e))?;
            }
        }

        Ok(())
    }

    /// Get Android app-specific storage path
    pub fn app_storage_path() -> PathBuf {
        // In a real implementation, this would get from Context
        PathBuf::from("/data/data/com.arula.terminal/files")
    }

    /// Get Android external storage path
    pub fn external_storage_path() -> PathBuf {
        PathBuf::from("/storage/emulated/0")
    }

    /// Get Download directory
    pub fn downloads_path() -> PathBuf {
        Self::external_storage_path().join("Download")
    }

    /// Get Documents directory
    pub fn documents_path() -> PathBuf {
        Self::external_storage_path().join("Documents")
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
    pub modified: Option<u64>,
}