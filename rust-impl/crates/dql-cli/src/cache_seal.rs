//! AES-256-GCM for the table cache and the query-result cache.
//!
//! A sealed file is a version byte, a 12-byte nonce, then ciphertext and the
//! 16-byte tag. Byte `0x7b` (`{`) is plaintext JSON from before sealing and is
//! rewritten in place. Authentication failure is a cache miss.
//!
//! The 32-byte key is per OS user. It is not the API token and it does not live
//! next to the cache files.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
const MIN_LEN: usize = 1 + NONCE_LEN + TAG_LEN;
const KEY_LEN: usize = 32;
const STORE_TIMEOUT: Duration = Duration::from_secs(3);

const SECRET_SERVICE: &str = "dqlrs";
const SECRET_KIND: &str = "cache-key";
const KEYCHAIN_SERVICE: &str = "dqlrs-cache-key";
const KEYCHAIN_ACCOUNT: &str = "dqlrs";

static KEY: Mutex<Option<[u8; KEY_LEN]>> = Mutex::new(None);
static KEY_ERROR_LOGGED: AtomicBool = AtomicBool::new(false);
static FILE_KEY: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Store {
    File,
    SecretService,
    Keychain,
    Dpapi,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

/// The process key. The first call loads or creates it. Later calls reuse it.
pub(crate) fn cache_key() -> Result<[u8; KEY_LEN], String> {
    let mut slot = lock(&KEY);
    if let Some(key) = *slot {
        return Ok(key);
    }
    match load_platform_key() {
        Ok(key) => {
            *slot = Some(key);
            Ok(key)
        }
        Err(err) => {
            if !KEY_ERROR_LOGGED.swap(true, Ordering::Relaxed) {
                eprintln!("dqlrs cache key: {err}");
            }
            Err(err)
        }
    }
}

fn load_platform_key() -> Result<[u8; KEY_LEN], String> {
    if let Some(path) = std::env::var_os("DQLRS_CACHE_KEY_FILE") {
        if !path.is_empty() {
            return load_or_create_file(Path::new(&path));
        }
    }
    if cfg!(test) {
        return load_or_create_file(&test_key_path());
    }
    load_os_key()
}

fn test_key_path() -> PathBuf {
    std::env::temp_dir()
        .join(format!("dqlrs-test-key-{}", std::process::id()))
        .join("cache.key")
}

fn load_os_key() -> Result<[u8; KEY_LEN], String> {
    let store = select_store(
        current_os(),
        is_wsl(),
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
        command_on_path("secret-tool"),
    );
    match store {
        Store::File => load_or_create_file(&default_key_file()),
        Store::SecretService => load_secret_service_key(),
        Store::Keychain => load_keychain_key(),
        Store::Dpapi => load_dpapi_key(),
    }
}

fn current_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

fn select_store(os: &str, wsl: bool, dbus: bool, secret_tool: bool) -> Store {
    match os {
        "macos" => Store::Keychain,
        "windows" => Store::Dpapi,
        "linux" if !wsl && dbus && secret_tool => Store::SecretService,
        _ => Store::File,
    }
}

fn default_key_file() -> PathBuf {
    let xdg = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").ok();
    key_file_from(xdg.as_deref(), home.as_deref())
}

fn key_file_from(xdg: Option<&str>, home: Option<&str>) -> PathBuf {
    if let Some(dir) = xdg {
        if !dir.is_empty() {
            return PathBuf::from(dir).join("dqlrs").join("cache.key");
        }
    }
    if let Some(dir) = home {
        if !dir.is_empty() {
            return PathBuf::from(dir)
                .join(".config")
                .join("dqlrs")
                .join("cache.key");
        }
    }
    PathBuf::from(".config").join("dqlrs").join("cache.key")
}

/// Load the raw 32-byte key at `path`, or create it once.
///
/// A file whose length is not 32 bytes is an error. A new key is not written
/// over it.
pub(crate) fn load_or_create_file(path: &Path) -> Result<[u8; KEY_LEN], String> {
    let _guard = lock(&FILE_KEY);
    if path.is_file() {
        let bytes = fs::read(path).map_err(|err| err.to_string())?;
        return key_from_exact(&bytes, &path.display().to_string());
    }
    let key = random_key()?;
    write_private(path, &key).map_err(|err| err.to_string())?;
    announce(&path.display().to_string());
    Ok(key)
}

fn key_from_exact(bytes: &[u8], place: &str) -> Result<[u8; KEY_LEN], String> {
    if bytes.len() != KEY_LEN {
        return Err(format!("cache key in {place} is not 32 bytes"));
    }
    let mut key = [0u8; KEY_LEN];
    key.copy_from_slice(bytes);
    Ok(key)
}

fn load_secret_service_key() -> Result<[u8; KEY_LEN], String> {
    match secret_lookup(SECRET_KIND)? {
        Some(key) => Ok(key),
        None => {
            let key = random_key()?;
            if let Err(err) = secret_store(SECRET_KIND, &key) {
                eprintln!(
                    "dqlrs cache key: secret service store failed ({err}); using the file fallback"
                );
                return load_or_create_file(&default_key_file());
            }
            announce("the desktop secret service");
            Ok(key)
        }
    }
}

fn load_keychain_key() -> Result<[u8; KEY_LEN], String> {
    match keychain_lookup()? {
        Some(key) => Ok(key),
        None => {
            let key = random_key()?;
            keychain_store(&key)?;
            announce("the login keychain");
            Ok(key)
        }
    }
}

fn load_dpapi_key() -> Result<[u8; KEY_LEN], String> {
    #[cfg(target_os = "windows")]
    {
        load_dpapi_key_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("DPAPI cache keys are only available on Windows".to_string())
    }
}

fn announce(place: &str) {
    eprintln!(
        "saved a new cache key in {place}. Losing it makes cached table definitions and query history unreadable."
    );
}

fn random_key() -> Result<[u8; KEY_LEN], String> {
    let mut key = [0u8; KEY_LEN];
    getrandom::getrandom(&mut key).map_err(|err| err.to_string())?;
    Ok(key)
}

pub(crate) fn seal(key: &[u8; KEY_LEN], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|err| err.to_string())?;
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "cache key must be 32 bytes".to_string())?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| "cache encrypt failed".to_string())?;
    let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
    out.push(VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub(crate) fn open(key: &[u8; KEY_LEN], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.first() != Some(&VERSION) || blob.len() < MIN_LEN {
        return Err("not a sealed cache file".to_string());
    }
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| "cache key must be 32 bytes".to_string())?;
    cipher
        .decrypt(
            Nonce::from_slice(&blob[1..1 + NONCE_LEN]),
            &blob[1 + NONCE_LEN..],
        )
        .map_err(|_| "authentication failed".to_string())
}

/// Write `plaintext` as a sealed file. The parent directory is created `0700`
/// and the file is `0600`. Callers that own a further parent (`.dqlrs-web-cache`)
/// tighten that directory themselves. This does not walk up to the workspace
/// or to `~/.config`.
pub(crate) fn write_sealed(path: &Path, plaintext: &[u8]) -> Result<(), String> {
    let key = cache_key()?;
    let sealed = seal(&key, plaintext)?;
    write_private(path, &sealed).map_err(|err| err.to_string())
}

/// Read a cache file. Plaintext JSON is sealed in place and returned. A bad
/// tag, a short file, or a missing key is `None`.
pub(crate) fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    let bytes = fs::read(path).ok()?;
    if bytes.first() == Some(&b'{') && serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
        let _ = write_sealed(path, &bytes);
        return Some(bytes);
    }
    if bytes.first() != Some(&VERSION) || bytes.len() < MIN_LEN {
        return None;
    }
    let key = cache_key().ok()?;
    open(&key, &bytes).ok()
}

/// Create `dir` and set it to `0700`. Ancestors created along the way keep
/// the process umask. Only `dir` itself is tightened.
pub(crate) fn ensure_owner_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    set_mode(dir, 0o700)
}

/// Set an existing directory to `0700`. Missing paths are left alone.
pub(crate) fn restrict_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        set_mode(dir, 0o700)?;
    }
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_owner_dir(parent)?;
        }
    }
    let tmp = temp_path(path)?;
    {
        let mut file = open_private(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    replace_file(&tmp, path)?;
    set_mode(path, 0o600)?;
    sync_parent(path);
    Ok(())
}

fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "cache file name"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(".tmp");
    Ok(path.with_file_name(tmp_name))
}

fn open_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    set_mode(path, 0o600)?;
    Ok(file)
}

fn replace_file(tmp: &Path, path: &Path) -> io::Result<()> {
    if fs::rename(tmp, path).is_err() {
        let _ = fs::remove_file(path);
        fs::rename(tmp, path)?;
    }
    Ok(())
}

fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

fn is_wsl() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some() {
        return true;
    }
    fs::read_to_string("/proc/sys/kernel/osrelease").is_ok_and(|text| wsl_release(&text))
}

fn wsl_release(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("microsoft") || lower.contains("wsl")
}

fn command_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

fn secret_lookup(kind: &str) -> Result<Option<[u8; KEY_LEN]>, String> {
    let mut cmd = Command::new("secret-tool");
    cmd.args(["lookup", "service", SECRET_SERVICE, "kind", kind]);
    let output = run_command(&mut cmd, None)?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        return decode_hex(&text).map(Some);
    }
    if output.status.code() == Some(1) && output.stderr.is_empty() {
        return Ok(None);
    }
    let err = String::from_utf8_lossy(&output.stderr);
    let err = err.trim();
    if output.status.code() == Some(1) && not_found_message(err) {
        return Ok(None);
    }
    Err(format!("secret-tool lookup failed: {err}"))
}

fn secret_store(kind: &str, key: &[u8; KEY_LEN]) -> Result<(), String> {
    let hex = encode_hex(key);
    let mut cmd = Command::new("secret-tool");
    cmd.args([
        "store",
        "--label=dqlrs cache key",
        "service",
        SECRET_SERVICE,
        "kind",
        kind,
    ]);
    let output = run_command(&mut cmd, Some(hex.as_bytes()))?;
    if output.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&output.stderr);
    Err(format!("secret-tool store failed: {}", err.trim()))
}

#[cfg(test)]
fn secret_clear(kind: &str) -> Result<(), String> {
    let mut cmd = Command::new("secret-tool");
    cmd.args(["clear", "service", SECRET_SERVICE, "kind", kind]);
    let output = run_command(&mut cmd, None)?;
    if output.status.success() {
        Ok(())
    } else {
        Err("secret-tool clear failed".to_string())
    }
}

fn not_found_message(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("not found") || lower.contains("no such") || lower.contains("could not be found")
}

fn keychain_lookup() -> Result<Option<[u8; KEY_LEN]>, String> {
    let mut cmd = Command::new("security");
    cmd.args([
        "find-generic-password",
        "-s",
        KEYCHAIN_SERVICE,
        "-a",
        KEYCHAIN_ACCOUNT,
        "-w",
    ]);
    let output = run_command(&mut cmd, None)?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        return decode_hex(&text).map(Some);
    }
    let err = String::from_utf8_lossy(&output.stderr);
    let err = err.trim();
    if output.status.code() == Some(44) || not_found_message(err) {
        return Ok(None);
    }
    Err(format!("keychain lookup failed: {err}"))
}

fn keychain_store(key: &[u8; KEY_LEN]) -> Result<(), String> {
    let hex = encode_hex(key);
    let mut cmd = Command::new("security");
    cmd.args([
        "add-generic-password",
        "-U",
        "-s",
        KEYCHAIN_SERVICE,
        "-a",
        KEYCHAIN_ACCOUNT,
        "-w",
        &hex,
    ]);
    let output = run_command(&mut cmd, None)?;
    if output.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&output.stderr);
        Err(format!("keychain store failed: {}", err.trim()))
    }
}

fn run_command(
    cmd: &mut Command,
    stdin_bytes: Option<&[u8]>,
) -> Result<std::process::Output, String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if stdin_bytes.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    let mut child = cmd.spawn().map_err(|err| err.to_string())?;
    if let Some(bytes) = stdin_bytes {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(bytes);
        }
    }
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|err| err.to_string()),
            Ok(None) if started.elapsed() >= STORE_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("cache key store timed out".to_string());
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err.to_string());
            }
        }
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(text: &str) -> Result<[u8; KEY_LEN], String> {
    let text = text.trim();
    if text.len() != KEY_LEN * 2 {
        return Err("cache key is not 32 bytes".to_string());
    }
    let bytes = text.as_bytes();
    let mut out = [0u8; KEY_LEN];
    for index in 0..KEY_LEN {
        let hi = hex_val(bytes[index * 2])?;
        let lo = hex_val(bytes[index * 2 + 1])?;
        out[index] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_val(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("cache key is not hex".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn load_dpapi_key_windows() -> Result<[u8; KEY_LEN], String> {
    let path = dpapi_key_path();
    if path.is_file() {
        let blob = fs::read(&path).map_err(|err| err.to_string())?;
        let raw = dpapi_unprotect(&blob)?;
        return key_from_exact(&raw, &path.display().to_string());
    }
    let key = random_key()?;
    let blob = dpapi_protect(&key)?;
    write_private(&path, &blob).map_err(|err| err.to_string())?;
    announce(&path.display().to_string());
    Ok(key)
}

#[cfg(target_os = "windows")]
fn dpapi_key_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("dqlrs").join("cache.key")
}

#[cfg(target_os = "windows")]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(data.len()).map_err(|_| "cache key is too long".to_string())?,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return Err(format!("CryptProtectData failed ({})", GetLastError()));
        }
        if output.pbData.is_null() || output.cbData == 0 {
            return Err("CryptProtectData returned an empty blob".to_string());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(bytes)
    }
}

#[cfg(target_os = "windows")]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::{GetLastError, LocalFree};
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    unsafe {
        let mut input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(data.len())
                .map_err(|_| "cache key blob is too long".to_string())?,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 {
            return Err(format!("CryptUnprotectData failed ({})", GetLastError()));
        }
        if output.pbData.is_null() || output.cbData == 0 {
            return Err("CryptUnprotectData returned an empty blob".to_string());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        Ok(bytes)
    }
}

#[cfg(all(unix, test))]
pub(crate) fn mode_bits(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|meta| meta.permissions().mode() & 0o777)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hides(bytes: &[u8], secret: &[u8]) -> bool {
        secret.is_empty() || !bytes.windows(secret.len()).any(|window| window == secret)
    }

    #[test]
    fn seal_round_trips_and_hides_the_plaintext() {
        let key = random_key().unwrap();
        let plain = b"SELECT marker_row FROM posts";
        let sealed = seal(&key, plain).unwrap();
        let again = seal(&key, plain).unwrap();
        assert_ne!(sealed, again);
        assert_eq!(sealed[0], VERSION);
        assert!(sealed.len() >= MIN_LEN);
        assert!(hides(&sealed, b"marker_row"));
        assert!(hides(&sealed, b"posts"));
        assert_eq!(open(&key, &sealed).unwrap(), plain);
        let mut wrong = key;
        wrong[0] ^= 0xff;
        assert!(open(&wrong, &sealed).is_err());
        assert!(open(&key, &[VERSION, 1, 2, 3]).is_err());
    }

    #[test]
    fn file_key_reloads_after_the_copy_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dqlrs").join("cache.key");
        #[cfg(unix)]
        let before = mode_bits(dir.path());
        let sealed = {
            let created = load_or_create_file(&path).unwrap();
            seal(&created, b"SELECT marker_row;").unwrap()
        };
        let loaded = load_or_create_file(&path).unwrap();
        assert_eq!(open(&loaded, &sealed).unwrap(), b"SELECT marker_row;");
        assert_eq!(fs::read(&path).unwrap().len(), KEY_LEN);
        #[cfg(unix)]
        {
            assert_eq!(mode_bits(&path), 0o600);
            assert_eq!(mode_bits(path.parent().unwrap()), 0o700);
            assert_eq!(mode_bits(dir.path()), before);
        }
    }

    #[test]
    fn a_short_key_file_is_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.key");
        fs::write(&path, [1u8, 2, 3]).unwrap();
        let err = load_or_create_file(&path).unwrap_err();
        assert!(err.contains("not 32 bytes"));
        assert_eq!(fs::read(&path).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn write_sealed_is_owner_only_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let before = mode_bits(dir.path());
        let path = dir.path().join("definitions").join("item.json");
        let plain = b"SELECT alpha FROM posts";
        write_sealed(&path, plain).unwrap();
        let stored = fs::read(&path).unwrap();
        assert_eq!(stored[0], VERSION);
        assert!(hides(&stored, b"alpha"));
        assert!(hides(&stored, b"posts"));
        assert_eq!(read_bytes(&path).unwrap(), plain);
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["item.json".to_string()]);
        #[cfg(unix)]
        {
            assert_eq!(mode_bits(&path), 0o600);
            assert_eq!(mode_bits(path.parent().unwrap()), 0o700);
            assert_eq!(mode_bits(dir.path()), before);
        }
    }

    #[test]
    fn read_bytes_migrates_plaintext_json_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.json");
        let plain = br#"{"saved_at_ms":1,"names":["marker_table"]}"#;
        fs::write(&path, plain).unwrap();
        assert_eq!(read_bytes(&path).unwrap(), plain);
        let stored = fs::read(&path).unwrap();
        assert_eq!(stored[0], VERSION);
        assert!(hides(&stored, b"marker_table"));
        assert_eq!(read_bytes(&path).unwrap(), plain);
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("list.json"))
            .collect();
        assert_eq!(names, vec!["list.json".to_string()]);
    }

    #[test]
    fn authentication_failure_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("list.json");
        let mut key = cache_key().unwrap();
        key[0] ^= 0xff;
        fs::write(&path, seal(&key, b"{\"names\":[\"secret\"]}").unwrap()).unwrap();
        assert!(read_bytes(&path).is_none());
        fs::write(&path, br#"{"not":"json""#).unwrap();
        assert!(read_bytes(&path).is_none());
    }

    #[test]
    fn key_file_path_prefers_xdg_then_home() {
        assert_eq!(
            key_file_from(Some("/xdg"), Some("/home/me")),
            PathBuf::from("/xdg/dqlrs/cache.key")
        );
        assert_eq!(
            key_file_from(Some(""), Some("/home/me")),
            PathBuf::from("/home/me/.config/dqlrs/cache.key")
        );
        assert_eq!(
            key_file_from(None, None),
            PathBuf::from(".config/dqlrs/cache.key")
        );
    }

    #[test]
    fn store_selection_matches_the_platform_table() {
        assert_eq!(select_store("macos", false, false, false), Store::Keychain);
        assert_eq!(select_store("windows", false, false, false), Store::Dpapi);
        assert_eq!(
            select_store("linux", false, true, true),
            Store::SecretService
        );
        assert_eq!(select_store("linux", true, true, true), Store::File);
        assert_eq!(select_store("linux", false, false, true), Store::File);
        assert_eq!(select_store("linux", false, true, false), Store::File);
        assert_eq!(select_store("other", false, true, true), Store::File);
        assert!(wsl_release("5.15.0-microsoft-standard-WSL2"));
        assert!(!wsl_release("6.8.0-generic"));
    }

    #[test]
    fn hex_round_trip_rejects_a_short_secret() {
        let key = [0xabu8; KEY_LEN];
        assert_eq!(decode_hex(&encode_hex(&key)).unwrap(), key);
        assert!(decode_hex("abcd").is_err());
        assert!(decode_hex(&format!("{}\n", encode_hex(&key))).is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn secret_service_round_trip_when_present() {
        if is_wsl()
            || std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
            || !command_on_path("secret-tool")
        {
            return;
        }
        if secret_lookup("cache-key-absent-probe").is_err() {
            return;
        }
        let kind = format!("cache-key-test-{}", std::process::id());
        let key = random_key().unwrap();
        struct Clear(String);
        impl Drop for Clear {
            fn drop(&mut self) {
                let _ = secret_clear(&self.0);
            }
        }
        let _clear = Clear(kind.clone());
        secret_store(&kind, &key).unwrap();
        assert_eq!(secret_lookup(&kind).unwrap().unwrap(), key);
    }
}
