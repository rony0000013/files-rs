use crate::core::path_util::FilesPath;
use crate::storage::storage::{FileMetadata, FileType, StorableItem, StorageService};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use suppaftp::tokio::AsyncFtpStream;
use suppaftp::FtpError;
use tokio::io::AsyncReadExt;

#[allow(dead_code)]
pub struct FtpStorageService {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
}

#[allow(dead_code)]
impl FtpStorageService {
    async fn connect(&self) -> Result<AsyncFtpStream> {
        let mut ftp_stream =
            AsyncFtpStream::connect(format!("{}:{}", self.host, self.port)).await?;
        ftp_stream.login(&self.user, &self.pass).await?;
        Ok(ftp_stream)
    }
}

#[async_trait]
impl StorageService for FtpStorageService {
    async fn list_directory(&self, path: &FilesPath) -> Result<Vec<StorableItem>> {
        let mut stream = self.connect().await?;
        let path_str = path.as_path().to_string_lossy();

        if !path_str.is_empty() {
            stream.cwd(&path_str).await?;
        }

        let list = stream.list(None).await?;
        let mut items = Vec::new();

        for line in list {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let name = parts.last().unwrap_or(&"unknown").to_string();
            let size: u64 = parts[4].parse().unwrap_or(0);
            let is_dir = line.starts_with('d');

            items.push(StorableItem {
                path: FilesPath::new(path.as_path().join(name)),
                metadata: FileMetadata {
                    size,
                    created: None,
                    modified: None,
                    file_type: if is_dir {
                        FileType::Directory
                    } else {
                        FileType::File
                    },
                    is_hidden: false,
                    is_system: false,
                },
            });
        }

        Ok(items)
    }

    async fn get_metadata(&self, path: &FilesPath) -> Result<StorableItem> {
        let items = self.list_directory(path).await?;
        items
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("File not found on FTP server"))
    }

    async fn read_file(&self, path: &FilesPath) -> Result<Vec<u8>> {
        let mut stream = self.connect().await?;

        // Use retr_with_callback and handle the error properly
        let bytes: Vec<u8> = stream
            .retr(path.as_path().to_string_lossy().as_ref(), |reader| {
                Box::pin(async move {
                    let mut reader = reader;
                    let mut buf = Vec::new();
                    reader
                        .read_to_end(&mut buf)
                        .await
                        .map_err(|e| FtpError::ConnectionError(e))?;
                    Ok((buf, reader))
                })
            })
            .await
            .map_err(|e| anyhow!("FTP read error: {:?}", e))?;

        Ok(bytes)
    }

    async fn write_file(&self, path: &FilesPath, data: &[u8]) -> Result<()> {
        let mut stream = self.connect().await?;

        let path_str = path.as_path().to_string_lossy().to_string();

        // Use put_file to upload data
        let mut reader = data;
        stream
            .put_file(&path_str, &mut reader)
            .await
            .map_err(|e| anyhow!("FTP write error: {:?}", e))?;

        Ok(())
    }
}
