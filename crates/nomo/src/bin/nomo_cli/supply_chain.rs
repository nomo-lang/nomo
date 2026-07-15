use nomo_supply_chain::{
    CachedTreeHead, PublisherKey, SignedReleaseEnvelope, TransparencyBundle, decode_hex,
    publisher_key_id, sha256_digest, verify_release_envelope, verify_transparency_bundle,
};
use std::fs;
use std::path::PathBuf;

const USAGE: &str = "usage: nomo verify <archive> --envelope <file> --key <ed25519-public-key-hex> [--provenance <file>] [--transparency <file> --log-key <ed25519-public-key-hex>] [--cached-head <file>]";

pub(super) fn run_verify_command(args: Vec<String>) -> Result<(), String> {
    let options = parse_verify_args(args)?;
    let archive = fs::read(&options.archive).map_err(|err| {
        format!(
            "failed to read package archive {}: {err}",
            options.archive.display()
        )
    })?;
    let envelope: SignedReleaseEnvelope = read_json(&options.envelope, "release envelope")?;
    let actual_archive_checksum = sha256_digest(&archive);
    if envelope.subject.archive_checksum != actual_archive_checksum {
        return Err(format!(
            "package archive checksum does not match signed release: expected {}, found {actual_archive_checksum}",
            envelope.subject.archive_checksum
        ));
    }
    let public_key = decode_hex(&options.public_key)?;
    if public_key.len() != 32 {
        return Err("ed25519 publisher public key must contain 32 bytes".to_string());
    }
    verify_release_envelope(
        &envelope,
        &envelope.subject,
        &[PublisherKey {
            key_id: publisher_key_id(&public_key),
            public_key: options.public_key.to_ascii_lowercase(),
        }],
    )?;
    if let Some(expected) = envelope.subject.provenance_digest.as_deref() {
        let path = options.provenance.as_ref().ok_or_else(|| {
            "signed release includes provenance; pass --provenance <file> to verify it".to_string()
        })?;
        let bytes = fs::read(path)
            .map_err(|err| format!("failed to read provenance {}: {err}", path.display()))?;
        let actual = sha256_digest(&bytes);
        if actual != expected {
            return Err(format!(
                "provenance digest does not match signed release: expected {expected}, found {actual}"
            ));
        }
    }
    if let Some(path) = &options.transparency {
        let bundle: TransparencyBundle = read_json(path, "transparency bundle")?;
        let cached = options
            .cached_head
            .as_ref()
            .map(|path| read_json(path, "cached transparency head"))
            .transpose()?;
        if options.log_keys.is_empty() {
            return Err(
                "transparency verification requires a trusted --log-key <ed25519-public-key-hex>"
                    .to_string(),
            );
        }
        let head =
            verify_transparency_bundle(&bundle, &envelope, cached.as_ref(), &options.log_keys)?;
        if let Some(path) = &options.cached_head {
            write_json(path, &head)?;
        }
        println!("transparency {} {}", head.tree_size, head.root_hash);
    }
    println!(
        "verified {} {} {}",
        envelope.subject.package, envelope.subject.version, envelope.signature.key_id
    );
    Ok(())
}

struct VerifyArgs {
    archive: PathBuf,
    envelope: PathBuf,
    public_key: String,
    provenance: Option<PathBuf>,
    transparency: Option<PathBuf>,
    cached_head: Option<PathBuf>,
    log_keys: Vec<String>,
}

fn parse_verify_args(args: Vec<String>) -> Result<VerifyArgs, String> {
    let mut archive = None;
    let mut envelope = None;
    let mut public_key = None;
    let mut provenance = None;
    let mut transparency = None;
    let mut cached_head = None;
    let mut log_keys = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        let target = match arg.as_str() {
            "--envelope" => Some("envelope"),
            "--key" => Some("key"),
            "--provenance" => Some("provenance"),
            "--transparency" => Some("transparency"),
            "--cached-head" => Some("cached-head"),
            "--log-key" => Some("log-key"),
            _ => None,
        };
        if let Some(target) = target {
            index += 1;
            let value = args.get(index).ok_or_else(|| USAGE.to_string())?;
            match target {
                "envelope" => envelope = Some(PathBuf::from(value)),
                "key" => public_key = Some(value.clone()),
                "provenance" => provenance = Some(PathBuf::from(value)),
                "transparency" => transparency = Some(PathBuf::from(value)),
                "cached-head" => cached_head = Some(PathBuf::from(value)),
                "log-key" => log_keys.push(value.to_ascii_lowercase()),
                _ => unreachable!(),
            }
        } else if arg.starts_with('-') || archive.is_some() {
            return Err(USAGE.to_string());
        } else {
            archive = Some(PathBuf::from(arg));
        }
        index += 1;
    }
    Ok(VerifyArgs {
        archive: archive.ok_or_else(|| USAGE.to_string())?,
        envelope: envelope.ok_or_else(|| USAGE.to_string())?,
        public_key: public_key.ok_or_else(|| USAGE.to_string())?,
        provenance,
        transparency,
        cached_head,
        log_keys,
    })
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf, label: &str) -> Result<T, String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read {label} {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("invalid {label} at {}: {err}", path.display()))
}

fn write_json(path: &PathBuf, value: &CachedTreeHead) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let mut rendered = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    rendered.push('\n');
    fs::write(path, rendered)
        .map_err(|err| format!("failed to write cached head {}: {err}", path.display()))
}
