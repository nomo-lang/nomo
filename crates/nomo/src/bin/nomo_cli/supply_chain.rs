use nomo_supply_chain::{
    DEFAULT_OFFLINE_PROOF_MAX_AGE_SECONDS, DEFAULT_PROOF_MAX_AGE_SECONDS,
    DEFAULT_PROOF_MAX_FUTURE_SKEW_SECONDS, GossipCheckpoint, ProofFreshnessPolicy, PublisherKey,
    SignedReleaseEnvelope, TransparencyBundle, TransparencyVerificationPolicy,
    current_unix_seconds, decode_hex, publisher_key_id, sha256_digest, verify_release_envelope,
    verify_transparency_bundle_with_policy,
};
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: nomo verify <archive> --envelope <file> --key <ed25519-public-key-hex> [--provenance <file>] [--transparency <file> --log-key <ed25519-public-key-hex>] [--cached-head <file>] [--gossip <file>] [--write-gossip <file>] [--proof-max-age-seconds <seconds>] [--offline-proof-max-age-seconds <seconds>] [--max-future-skew-seconds <seconds>] [--offline]";

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
        let mut gossip_checkpoints = Vec::new();
        for path in &options.gossip {
            gossip_checkpoints.extend(read_gossip_checkpoints(path)?);
        }
        let policy = TransparencyVerificationPolicy {
            trusted_log_keys: options.log_keys.clone(),
            freshness: ProofFreshnessPolicy {
                max_age_seconds: options.proof_max_age_seconds,
                offline_max_age_seconds: options.offline_proof_max_age_seconds,
                max_future_skew_seconds: options.max_future_skew_seconds,
            },
            gossip_checkpoints,
        };
        let verified = verify_transparency_bundle_with_policy(
            &bundle,
            &envelope,
            cached.as_ref(),
            &policy,
            options.offline,
            current_unix_seconds(),
        )?;
        if let Some(path) = &options.cached_head {
            write_json(path, &verified.cached_head)?;
        }
        if let Some(path) = &options.write_gossip {
            write_json(path, &verified.gossip_checkpoint)?;
        }
        println!(
            "transparency {} {} {}",
            verified.cached_head.tree_size,
            verified.cached_head.root_hash,
            verified.cached_head.key_id
        );
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
    gossip: Vec<PathBuf>,
    write_gossip: Option<PathBuf>,
    proof_max_age_seconds: u64,
    offline_proof_max_age_seconds: u64,
    max_future_skew_seconds: u64,
    offline: bool,
}

fn parse_verify_args(args: Vec<String>) -> Result<VerifyArgs, String> {
    let mut archive = None;
    let mut envelope = None;
    let mut public_key = None;
    let mut provenance = None;
    let mut transparency = None;
    let mut cached_head = None;
    let mut log_keys = Vec::new();
    let mut gossip = Vec::new();
    let mut write_gossip = None;
    let mut proof_max_age_seconds = DEFAULT_PROOF_MAX_AGE_SECONDS;
    let mut offline_proof_max_age_seconds = DEFAULT_OFFLINE_PROOF_MAX_AGE_SECONDS;
    let mut max_future_skew_seconds = DEFAULT_PROOF_MAX_FUTURE_SKEW_SECONDS;
    let mut offline = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--offline" {
            if offline {
                return Err("--offline may only be specified once".to_string());
            }
            offline = true;
            index += 1;
            continue;
        }
        let target = match arg.as_str() {
            "--envelope" => Some("envelope"),
            "--key" => Some("key"),
            "--provenance" => Some("provenance"),
            "--transparency" => Some("transparency"),
            "--cached-head" => Some("cached-head"),
            "--log-key" => Some("log-key"),
            "--gossip" => Some("gossip"),
            "--write-gossip" => Some("write-gossip"),
            "--proof-max-age-seconds" => Some("proof-max-age-seconds"),
            "--offline-proof-max-age-seconds" => Some("offline-proof-max-age-seconds"),
            "--max-future-skew-seconds" => Some("max-future-skew-seconds"),
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
                "gossip" => gossip.push(PathBuf::from(value)),
                "write-gossip" => write_gossip = Some(PathBuf::from(value)),
                "proof-max-age-seconds" => {
                    proof_max_age_seconds = parse_positive_seconds(value, target)?
                }
                "offline-proof-max-age-seconds" => {
                    offline_proof_max_age_seconds = parse_positive_seconds(value, target)?
                }
                "max-future-skew-seconds" => {
                    max_future_skew_seconds = parse_seconds(value, target)?
                }
                _ => unreachable!(),
            }
        } else if arg.starts_with('-') || archive.is_some() {
            return Err(USAGE.to_string());
        } else {
            archive = Some(PathBuf::from(arg));
        }
        index += 1;
    }
    if offline_proof_max_age_seconds < proof_max_age_seconds {
        return Err(
            "--offline-proof-max-age-seconds must be at least --proof-max-age-seconds".to_string(),
        );
    }
    Ok(VerifyArgs {
        archive: archive.ok_or_else(|| USAGE.to_string())?,
        envelope: envelope.ok_or_else(|| USAGE.to_string())?,
        public_key: public_key.ok_or_else(|| USAGE.to_string())?,
        provenance,
        transparency,
        cached_head,
        log_keys,
        gossip,
        write_gossip,
        proof_max_age_seconds,
        offline_proof_max_age_seconds,
        max_future_skew_seconds,
        offline,
    })
}

fn parse_seconds(value: &str, option: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("--{option} requires a non-negative integer"))
}

fn parse_positive_seconds(value: &str, option: &str) -> Result<u64, String> {
    let value = parse_seconds(value, option)?;
    if value == 0 {
        Err(format!("--{option} must be positive"))
    } else {
        Ok(value)
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf, label: &str) -> Result<T, String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read {label} {}: {err}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("invalid {label} at {}: {err}", path.display()))
}

fn read_gossip_checkpoints(path: &PathBuf) -> Result<Vec<GossipCheckpoint>, String> {
    let bytes = fs::read(path)
        .map_err(|err| format!("failed to read gossip checkpoint {}: {err}", path.display()))?;
    if let Ok(checkpoint) = serde_json::from_slice::<GossipCheckpoint>(&bytes) {
        return Ok(vec![checkpoint]);
    }
    serde_json::from_slice::<Vec<GossipCheckpoint>>(&bytes)
        .map_err(|err| format!("invalid gossip checkpoint at {}: {err}", path.display()))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    create_output_parent(path)?;
    let mut rendered = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    rendered.push('\n');
    let temporary = path.with_extension(
        path.extension()
            .map(|extension| format!("{}.tmp", extension.to_string_lossy()))
            .unwrap_or_else(|| "tmp".to_string()),
    );
    fs::write(&temporary, rendered).map_err(|err| {
        format!(
            "failed to write temporary JSON output {}: {err}",
            temporary.display()
        )
    })?;
    fs::rename(&temporary, path)
        .map_err(|err| format!("failed to install JSON output {}: {err}", path.display()))
}

fn create_output_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create output directory {}: {err}",
                parent.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_args_parse_gossip_and_freshness_policy() {
        let parsed = parse_verify_args(
            [
                "archive.nomo-package",
                "--envelope",
                "envelope.json",
                "--key",
                "publisher",
                "--transparency",
                "bundle.json",
                "--log-key",
                "log-key",
                "--gossip",
                "peer-a.json",
                "--gossip",
                "peer-b.json",
                "--write-gossip",
                "latest.json",
                "--proof-max-age-seconds",
                "60",
                "--offline-proof-max-age-seconds",
                "600",
                "--max-future-skew-seconds",
                "10",
                "--offline",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        )
        .unwrap();
        assert_eq!(parsed.gossip.len(), 2);
        assert_eq!(parsed.write_gossip, Some(PathBuf::from("latest.json")));
        assert_eq!(parsed.proof_max_age_seconds, 60);
        assert_eq!(parsed.offline_proof_max_age_seconds, 600);
        assert_eq!(parsed.max_future_skew_seconds, 10);
        assert!(parsed.offline);
    }

    #[test]
    fn write_json_supports_a_current_directory_output() {
        create_output_parent(Path::new("checkpoint.json")).unwrap();
        let directory = std::env::temp_dir().join(format!(
            "nomo-verify-json-{}-{}",
            std::process::id(),
            current_unix_seconds()
        ));
        let output = directory.join("checkpoint.json");
        write_json(&output, &vec!["ok"]).unwrap();
        assert_eq!(fs::read_to_string(output).unwrap(), "[\n  \"ok\"\n]\n");
        fs::remove_dir_all(directory).unwrap();
    }
}
