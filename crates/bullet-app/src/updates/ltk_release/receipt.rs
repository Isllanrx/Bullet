use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const RECEIPT_FILE: &str = "receipt.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceStatus {
    SignedAuthenticode,
    UnsignedOfficial,
    Untrusted(String),
}

impl fmt::Display for ProvenanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignedAuthenticode => write!(f, "Signed (Authenticode)"),
            Self::UnsignedOfficial => write!(f, "Unsigned (Official Repository)"),
            Self::Untrusted(reason) => write!(f, "Untrusted ({reason})"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReceipt {
    pub name: String,
    pub size_bytes: usize,
    pub sha256: String,
    pub provenance: ProvenanceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInspectionReceipt {
    pub version: String,
    pub commit_sha: String,
    pub inspected_at_epoch_secs: u64,
    pub artifacts: Vec<ArtifactReceipt>,
}

impl ReleaseInspectionReceipt {
    #[must_use]
    pub fn new(version: String, commit_sha: String, artifacts: Vec<ArtifactReceipt>) -> Self {
        let inspected_at_epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            version,
            commit_sha,
            inspected_at_epoch_secs,
            artifacts,
        }
    }

    pub fn validate_structural(&self) -> Result<(), String> {
        if self.version.trim().is_empty() {
            return Err("receipt version cannot be empty".to_string());
        }
        if self.commit_sha.len() != 40 || !self.commit_sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "receipt commit_sha must be 40 hex characters, got {:?}",
                self.commit_sha
            ));
        }
        if self.artifacts.is_empty() {
            return Err("receipt must contain at least one artifact".to_string());
        }
        for artifact in &self.artifacts {
            if artifact.name.trim().is_empty()
                || artifact.name.contains('/')
                || artifact.name.contains('\\')
                || artifact.name.contains("..")
            {
                return Err(format!(
                    "artifact has invalid filename: {:?}",
                    artifact.name
                ));
            }
            if artifact.size_bytes == 0 {
                return Err(format!("artifact {:?} size must be > 0", artifact.name));
            }
            if artifact.sha256.len() != 64
                || !artifact.sha256.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(format!(
                    "artifact {:?} sha256 must be 64 hex characters, got {:?}",
                    artifact.name, artifact.sha256
                ));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn is_trusted(&self) -> bool {
        if self.validate_structural().is_err() {
            return false;
        }
        !self.artifacts.is_empty()
            && self.artifacts.iter().all(|a| {
                matches!(
                    a.provenance,
                    ProvenanceStatus::SignedAuthenticode | ProvenanceStatus::UnsignedOfficial
                )
            })
    }

    #[must_use]
    pub fn artifact(&self, name: &str) -> Option<&ArtifactReceipt> {
        self.artifacts.iter().find(|a| a.name == name)
    }

    pub fn verify_bytes(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.validate_structural()?;
        let expected = self
            .artifact(name)
            .ok_or_else(|| format!("artifact {name:?} not present in receipt"))?;
        if bytes.len() != expected.size_bytes {
            return Err(format!(
                "artifact {name} size mismatch: expected {} bytes, got {} bytes",
                expected.size_bytes,
                bytes.len()
            ));
        }
        let actual_sha256 = bullet_inject::dll_validator::compute_sha256(bytes);
        if !actual_sha256.eq_ignore_ascii_case(&expected.sha256) {
            return Err(format!(
                "artifact {name} sha256 mismatch: expected {}, got {actual_sha256}",
                expected.sha256
            ));
        }
        Ok(())
    }

    pub fn verify_staged_files(&self, files: &[(&str, &[u8])]) -> Result<(), String> {
        self.validate_structural()?;
        for (name, bytes) in files {
            self.verify_bytes(name, bytes)?;
        }
        Ok(())
    }

    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        self.validate_structural()?;
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let receipt: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        receipt.validate_structural()?;
        Ok(receipt)
    }
}
