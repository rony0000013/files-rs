use directories::UserDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use windows::core::{GUID, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KNOWN_FOLDER_FLAG};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FilesPath(PathBuf);

// GUIDs for known folders (Windows API)
pub mod known_folders {
    use super::*;
    pub const FOLDERID_DOCUMENTS: GUID = GUID::from_u128(0x4BED8C30_C1B7_4B89_9173_F9260C476B70);
    pub const FOLDERID_PICTURES: GUID = GUID::from_u128(0x33E28130_C1B7_4B89_9173_F9260C476B70);
    pub const FOLDERID_MUSIC: GUID = GUID::from_u128(0x18DE5000_C1B7_4B89_9173_F9260C476B70);
    pub const FOLDERID_VIDEOS: GUID = GUID::from_u128(0x8863A180_C1B7_4B89_9173_F9260C476B70);
    pub const FOLDERID_RECYCLE_BIN: GUID = GUID::from_u128(0xBF797060_1F7D_47F2_A63F_8E9A765A4455);
    pub const FOLDERID_NETWORK_HOOD: GUID = GUID::from_u128(0x590329B8_8283_4439_B7EF_FB5FC3F26D1);
    pub const FOLDERID_BUILDS: GUID = GUID::from_u128(0x0);
}

impl FilesPath {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self(path.as_ref().to_path_buf())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    #[allow(dead_code)]
    pub fn canonicalize(&self) -> anyhow::Result<Self> {
        let path = dunce::canonicalize(self.as_path())?;
        Ok(Self(path))
    }

    pub fn resolve_known_folder(folder_id: &GUID) -> anyhow::Result<Self> {
        unsafe {
            let mut path_ptr: PWSTR = PWSTR::null();
            // In windows-rs 0.52, SHGetKnownFolderPath's 3rd arg is an htoken.
            // To get the path, we pass the token (None for current user).
            // The return value is the PWSTR.
            let path_ptr = SHGetKnownFolderPath(folder_id, KNOWN_FOLDER_FLAG(0), None)?;

            let path_str = path_ptr.to_string()?;
            let path = PathBuf::from(path_str);

            windows::Win32::System::Com::CoTaskMemFree(Some(path_ptr.as_ptr() as *const _));

            Ok(Self::new(path))
        }
    }

    #[allow(dead_code)]
    pub fn user_dirs() -> Option<UserDirs> {
        UserDirs::new()
    }

    pub fn ancestors(&self) -> Vec<Self> {
        let mut res = Vec::new();
        let mut current = Some(self.as_path());
        while let Some(path) = current {
            let path_str = path.to_string_lossy();
            // Skip empty paths (e.g., parent of Windows root drive C:\)
            if !path_str.is_empty() {
                res.push(Self::new(path));
            }
            current = path.parent();
        }
        res.reverse();
        res
    }

    pub fn parent(&self) -> Option<Self> {
        self.0.parent().map(Self::new)
    }

    pub fn file_name(&self) -> Option<Self> {
        self.0.file_name().map(Self::new)
    }

    pub fn to_string_lossy(&self) -> std::borrow::Cow<'_, str> {
        self.0.to_string_lossy()
    }

    pub fn is_namespace_path(&self) -> bool {
        let s = self.to_string_lossy();
        s.contains(':') && !s.contains(r":\")
    }

    pub fn resolve_shell_path(&self) -> anyhow::Result<Self> {
        let s = self.to_string_lossy();
        if !s.starts_with("shell:") {
            return Err(anyhow::anyhow!("Not a shell namespace path"));
        }

        let folder_name = &s[6..];
        let folder_id = match folder_name {
            "RecycleBinFolder" => &known_folders::FOLDERID_RECYCLE_BIN,
            "MyComputerFolder" => &known_folders::FOLDERID_BUILDS,
            "NetworkPlacesFolder" => &known_folders::FOLDERID_NETWORK_HOOD,
            "Documents" => &known_folders::FOLDERID_DOCUMENTS,
            "Pictures" => &known_folders::FOLDERID_PICTURES,
            "Music" => &known_folders::FOLDERID_MUSIC,
            "Videos" => &known_folders::FOLDERID_VIDEOS,
            _ => return Err(anyhow::anyhow!("Unsupported shell folder: {}", folder_name)),
        };

        Self::resolve_known_folder(folder_id)
    }
}

impl From<PathBuf> for FilesPath {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<&str> for FilesPath {
    fn from(s: &str) -> Self {
        Self(PathBuf::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn test_new_path() {
        let p = FilesPath::new("test/path");
        assert_eq!(p.as_path(), Path::new("test/path"));
    }

    #[test]
    fn test_is_namespace() {
        let p = FilesPath::new("shell:MyFolder");
        assert!(p.is_namespace_path());

        let p2 = FilesPath::new(r"C:\Users\Test");
        assert!(!p2.is_namespace_path());
    }

    #[test]
    fn test_canonicalize() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let file_path = dir.path().join("test_file.txt");
        File::create(&file_path)?;

        let fs_path = FilesPath::new(file_path);
        let canonical = fs_path.canonicalize()?;

        assert_eq!(canonical.as_path(), fs_path.as_path());
        Ok(())
    }
}
