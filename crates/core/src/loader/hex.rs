use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use prost::Message;
use ring::digest;
use ring::signature;
use x509_parser::prelude::FromDer;

const HEX_REPOSITORY_BASE: &str = "https://repo.hex.pm";
const CACHE_SCHEMA_VERSION: u32 = 1;

const HEXPM_PUBLIC_KEY: &[u8] = b"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApqREcFDt5vV21JVe2QNB
Edvzk6w36aNFhVGWN5toNJRjRJ6m4hIuG4KaXtDWVLjnvct6MYMfqhC79HAGwyF+
IqR6Q6a5bbFSsImgBJwz1oadoVKD6ZNetAuCIK84cjMrEFRkELtEIPNHblCzUkkM
3rS9+DPlnfG8hBvGi6tvQIuZmXGCxF/73hU0/MyGhbmEjIKRtG6b0sJYKelRLTPW
XgK7s5pESgiwf2YC/2MGDXjAJfpfCd0RpLdvd4eRiXtVlE9qO9bND94E7PgQ/xqZ
J1i2xWFndWa6nfFnRxZmCStCOZWYYPlaxr+FZceFbpMwzTNs4g3d4tLNUcbKAIH4
0wIDAQAB
-----END PUBLIC KEY-----
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexPackageRequest {
    pub name: String,
    pub version: String,
    pub outer_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedTarball {
    pub package: HexPackageRequest,
    pub path: PathBuf,
    pub downloaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPackageMetadata {
    pub name: String,
    pub url: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexCache {
    root: PathBuf,
}

impl HexCache {
    pub fn from_home() -> Result<Self, HexError> {
        let home = dirs::home_dir().ok_or(HexError::MissingHomeDirectory)?;
        Ok(Self::new(home.join(".regulus").join("store")))
    }

    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tarballs_dir(&self) -> PathBuf {
        self.root.join("hex").join("tarballs")
    }

    pub fn tarball_path(&self, outer_checksum: &str) -> PathBuf {
        self.tarballs_dir().join(format!("{outer_checksum}.tar"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexLoader {
    cache: HexCache,
    repository_base: String,
}

impl HexLoader {
    pub fn from_home() -> Result<Self, HexError> {
        Ok(Self::new(HexCache::from_home()?))
    }

    pub fn new(cache: HexCache) -> Self {
        Self { cache, repository_base: HEX_REPOSITORY_BASE.to_string() }
    }

    pub fn with_repository_base(mut self, repository_base: impl Into<String>) -> Self {
        self.repository_base = repository_base.into().trim_end_matches('/').to_string();
        self
    }

    pub fn cache(&self) -> &HexCache {
        &self.cache
    }

    pub fn fetch_package_metadata(&self, name: &str) -> Result<VerifiedPackageMetadata, HexError> {
        validate_package_name(name)?;
        let url = self.package_metadata_url(name);
        let compressed = http_get(&url, "application/octet-stream")?;
        let signed = gunzip(&compressed)?;
        let payload = verify_signed_payload(&signed)?;
        Ok(VerifiedPackageMetadata { name: name.to_string(), url, payload })
    }

    pub fn ensure_tarball_cached(&self, package: HexPackageRequest) -> Result<CachedTarball, HexError> {
        validate_package_name(&package.name)?;
        validate_version(&package.version)?;
        let checksum = parse_sha256(&package.outer_checksum)?;
        let path = self.cache.tarball_path(&package.outer_checksum);

        if path.is_file() {
            let bytes = fs::read(&path).map_err(|source| HexError::ReadFile { path: path.clone(), source })?;
            verify_sha256(&bytes, &checksum)?;
            return Ok(CachedTarball { package, path, downloaded: false });
        }

        fs::create_dir_all(self.cache.tarballs_dir())
            .map_err(|source| HexError::CreateDirectory { path: self.cache.tarballs_dir(), source })?;

        let url = self.package_tarball_url(&package.name, &package.version);
        let bytes = http_get(&url, "application/x-tar")?;
        verify_sha256(&bytes, &checksum)?;

        let tmp_path = path.with_extension("tar.tmp");
        fs::write(&tmp_path, &bytes).map_err(|source| HexError::WriteFile { path: tmp_path.clone(), source })?;
        fs::rename(&tmp_path, &path).map_err(|source| HexError::RenameFile {
            from: tmp_path,
            to: path.clone(),
            source,
        })?;

        Ok(CachedTarball { package, path, downloaded: true })
    }

    pub fn package_metadata_url(&self, name: &str) -> String {
        format!("{}/packages/{name}", self.repository_base)
    }

    pub fn package_tarball_url(&self, name: &str, version: &str) -> String {
        format!("{}/tarballs/{name}-{version}.tar", self.repository_base)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageExtractionStamp {
    pub cache_schema_version: u32,
    pub name: String,
    pub version: String,
    pub outer_checksum: String,
    pub source: String,
}

impl PackageExtractionStamp {
    pub fn hex(name: String, version: String, outer_checksum: String) -> Self {
        Self { cache_schema_version: CACHE_SCHEMA_VERSION, source: "hex".to_string(), name, version, outer_checksum }
    }
}

#[derive(Debug)]
pub enum HexError {
    MissingHomeDirectory,
    InvalidPackageName(String),
    InvalidVersion(String),
    InvalidChecksum(String),
    IncorrectChecksum,
    InvalidMetadata(String),
    InvalidMetadataSignature,
    HttpRequest {
        url: String,
        source: reqwest::Error,
    },
    HttpStatus {
        url: String,
        status: u16,
    },
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
    RenameFile {
        from: PathBuf,
        to: PathBuf,
        source: std::io::Error,
    },
    Io(std::io::Error),
}

impl fmt::Display for HexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HexError::MissingHomeDirectory => write!(f, "could not determine home directory"),
            HexError::InvalidPackageName(name) => write!(f, "invalid Hex package name `{name}`"),
            HexError::InvalidVersion(version) => write!(f, "invalid Hex package version `{version}`"),
            HexError::InvalidChecksum(checksum) => write!(f, "invalid Hex package checksum `{checksum}`"),
            HexError::IncorrectChecksum => write!(f, "Hex package checksum did not match"),
            HexError::InvalidMetadata(error) => write!(f, "invalid Hex metadata: {error}"),
            HexError::InvalidMetadataSignature => write!(f, "Hex metadata signature did not verify"),
            HexError::HttpRequest { url, source } => write!(f, "could not fetch {url}: {source}"),
            HexError::HttpStatus { url, status } => write!(f, "could not fetch {url}: HTTP {status}"),
            HexError::CreateDirectory { path, source } => {
                write!(f, "could not create directory {}: {source}", path.display())
            }
            HexError::ReadFile { path, source } => {
                write!(f, "could not read file {}: {source}", path.display())
            }
            HexError::WriteFile { path, source } => {
                write!(f, "could not write file {}: {source}", path.display())
            }
            HexError::RenameFile { from, to, source } => write!(
                f,
                "could not move file {} to {}: {source}",
                from.display(),
                to.display()
            ),
            HexError::Io(error) => write!(f, "I/O error: {error}"),
        }
    }
}

impl std::error::Error for HexError {}

impl From<std::io::Error> for HexError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Clone, PartialEq, Message)]
struct SignedPayload {
    #[prost(bytes = "vec", required, tag = "1")]
    payload: Vec<u8>,
    #[prost(bytes = "vec", optional, tag = "2")]
    signature: Option<Vec<u8>>,
}

fn http_get(url: &str, accept: &str) -> Result<Vec<u8>, HexError> {
    let response = reqwest::blocking::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, accept)
        .header(reqwest::header::USER_AGENT, "regulus")
        .send()
        .map_err(|source| HexError::HttpRequest { url: url.to_string(), source })?;
    let status = response.status();
    if !status.is_success() {
        return Err(HexError::HttpStatus { url: url.to_string(), status: status.as_u16() });
    }

    response
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|source| HexError::HttpRequest { url: url.to_string(), source })
}

fn gunzip(bytes: &[u8]) -> Result<Vec<u8>, HexError> {
    let mut decoder = GzDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|error| HexError::InvalidMetadata(error.to_string()))?;
    Ok(output)
}

fn verify_signed_payload(bytes: &[u8]) -> Result<Vec<u8>, HexError> {
    let signed = SignedPayload::decode(bytes).map_err(|error| HexError::InvalidMetadata(error.to_string()))?;
    let signature = signed.signature.ok_or(HexError::InvalidMetadataSignature)?;
    let (_, pem) =
        x509_parser::pem::parse_x509_pem(HEXPM_PUBLIC_KEY).map_err(|_| HexError::InvalidMetadataSignature)?;
    let (_, spki) = x509_parser::prelude::SubjectPublicKeyInfo::from_der(&pem.contents)
        .map_err(|_| HexError::InvalidMetadataSignature)?;
    let key = signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA512, spki.subject_public_key.data);
    key.verify(&signed.payload, &signature)
        .map_err(|_| HexError::InvalidMetadataSignature)?;
    Ok(signed.payload)
}

fn parse_sha256(checksum: &str) -> Result<[u8; 32], HexError> {
    let checksum = checksum.trim();
    if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HexError::InvalidChecksum(checksum.to_string()));
    }

    let mut bytes = [0u8; 32];
    for (index, chunk) in checksum.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|_| HexError::InvalidChecksum(checksum.to_string()))?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| HexError::InvalidChecksum(checksum.to_string()))?;
    }
    Ok(bytes)
}

fn verify_sha256(bytes: &[u8], expected: &[u8; 32]) -> Result<(), HexError> {
    let digest = digest::digest(&digest::SHA256, bytes);
    if digest.as_ref() == expected { Ok(()) } else { Err(HexError::IncorrectChecksum) }
}

fn validate_package_name(name: &str) -> Result<(), HexError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-'));
    if valid { Ok(()) } else { Err(HexError::InvalidPackageName(name.to_string())) }
}

fn validate_version(version: &str) -> Result<(), HexError> {
    let valid = !version.is_empty()
        && version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b'_'));
    if valid { Ok(()) } else { Err(HexError::InvalidVersion(version.to_string())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_paths_use_regulus_store_layout() {
        let cache = HexCache::new(PathBuf::from("/home/me/.regulus/store"));

        assert_eq!(
            cache.tarballs_dir(),
            PathBuf::from("/home/me/.regulus/store/hex/tarballs")
        );
        assert_eq!(
            cache.tarball_path("abc123"),
            PathBuf::from("/home/me/.regulus/store/hex/tarballs/abc123.tar")
        );
    }

    #[test]
    fn validates_cached_tarball_checksum() {
        let bytes = b"tarball bytes";
        let checksum = digest::digest(&digest::SHA256, bytes);
        let checksum = checksum
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        let parsed = parse_sha256(&checksum).expect("checksum parsed");

        assert!(verify_sha256(bytes, &parsed).is_ok());
        assert!(matches!(
            verify_sha256(b"other bytes", &parsed),
            Err(HexError::IncorrectChecksum)
        ));
    }

    #[test]
    fn rejects_invalid_package_names_before_building_urls() {
        assert!(validate_package_name("gleam_stdlib").is_ok());
        assert!(validate_package_name("gleam-stdlib2").is_ok());
        assert!(matches!(
            validate_package_name("../gleam_stdlib"),
            Err(HexError::InvalidPackageName(_))
        ));
    }

    #[test]
    fn builds_hex_repository_urls() {
        let loader = HexLoader::new(HexCache::new(PathBuf::from("/cache")));

        assert_eq!(
            loader.package_metadata_url("gleam_stdlib"),
            "https://repo.hex.pm/packages/gleam_stdlib"
        );
        assert_eq!(
            loader.package_tarball_url("gleam_stdlib", "0.50.0"),
            "https://repo.hex.pm/tarballs/gleam_stdlib-0.50.0.tar"
        );
    }
}
