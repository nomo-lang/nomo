pub use nomo_manifest::RegistryTrustPolicy as TrustPolicy;
use nomo_manifest::{validate_package_id, validate_version_like};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub const RELEASE_ENVELOPE_SCHEMA: u32 = 1;
pub const PROVENANCE_SCHEMA: u32 = 1;
pub const TRANSPARENCY_HEAD_SCHEMA: u32 = 2;
pub const LOG_KEY_ROTATION_SCHEMA: u32 = 1;
pub const DEFAULT_PROOF_MAX_AGE_SECONDS: u64 = 24 * 60 * 60;
pub const DEFAULT_OFFLINE_PROOF_MAX_AGE_SECONDS: u64 = 7 * 24 * 60 * 60;
pub const DEFAULT_PROOF_MAX_FUTURE_SKEW_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSubject {
    pub schema: u32,
    pub package: String,
    pub version: String,
    pub archive_checksum: String,
    pub manifest_checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
}

impl ReleaseSubject {
    pub fn new(
        package: String,
        version: String,
        archive_checksum: String,
        manifest_checksum: String,
        provenance_digest: Option<String>,
    ) -> Result<Self, String> {
        let subject = Self {
            schema: RELEASE_ENVELOPE_SCHEMA,
            package,
            version,
            archive_checksum,
            manifest_checksum,
            provenance_digest,
        };
        subject.validate()?;
        Ok(subject)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != RELEASE_ENVELOPE_SCHEMA {
            return Err(format!(
                "unsupported release envelope schema {}",
                self.schema
            ));
        }
        validate_package_id(&self.package)?;
        validate_version_like("signed release version", &self.version)?;
        validate_sha256("archive checksum", &self.archive_checksum)?;
        validate_sha256("manifest checksum", &self.manifest_checksum)?;
        if let Some(digest) = self.provenance_digest.as_deref() {
            validate_sha256("provenance digest", digest)?;
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = b"nomo-release-envelope-v1\n".to_vec();
        canonical_field(&mut out, "package", &self.package);
        canonical_field(&mut out, "version", &self.version);
        canonical_field(&mut out, "archive", &self.archive_checksum);
        canonical_field(&mut out, "manifest", &self.manifest_checksum);
        canonical_field(
            &mut out,
            "provenance",
            self.provenance_digest.as_deref().unwrap_or(""),
        );
        Ok(out)
    }

    pub fn digest(&self) -> Result<String, String> {
        Ok(sha256_digest(&self.canonical_bytes()?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherSignature {
    pub algorithm: String,
    pub key_id: String,
    pub public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedReleaseEnvelope {
    pub subject: ReleaseSubject,
    pub signature: PublisherSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalSignerResponse {
    pub algorithm: String,
    #[serde(default)]
    pub key_id: Option<String>,
    pub public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherKey {
    pub key_id: String,
    pub public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedReleaseEvidence {
    pub key_id: String,
    pub subject_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparency_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseProvenance {
    pub schema: u32,
    pub builder: String,
    pub builder_version: String,
    pub package: String,
    pub version: String,
    pub archive_checksum: String,
    pub manifest_checksum: String,
}

impl ReleaseProvenance {
    pub fn render(&self) -> Result<String, String> {
        if self.schema != PROVENANCE_SCHEMA {
            return Err(format!("unsupported provenance schema {}", self.schema));
        }
        validate_package_id(&self.package)?;
        validate_version_like("provenance version", &self.version)?;
        validate_sha256("provenance archive checksum", &self.archive_checksum)?;
        validate_sha256("provenance manifest checksum", &self.manifest_checksum)?;
        let mut rendered = serde_json::to_string_pretty(self).map_err(|err| err.to_string())?;
        rendered.push('\n');
        Ok(rendered)
    }
}

pub fn envelope_from_signer_response(
    subject: ReleaseSubject,
    response: ExternalSignerResponse,
) -> Result<SignedReleaseEnvelope, String> {
    if response.algorithm != "ed25519" {
        return Err(format!(
            "unsupported signer algorithm `{}`; expected ed25519",
            response.algorithm
        ));
    }
    let public_key = decode_hex(&response.public_key)?;
    if public_key.len() != 32 {
        return Err("ed25519 public key must contain 32 bytes".to_string());
    }
    let derived_key_id = publisher_key_id(&public_key);
    if response
        .key_id
        .as_deref()
        .is_some_and(|key_id| key_id != derived_key_id)
    {
        return Err("external signer key id does not match its public key".to_string());
    }
    let envelope = SignedReleaseEnvelope {
        subject,
        signature: PublisherSignature {
            algorithm: response.algorithm,
            key_id: derived_key_id,
            public_key: encode_hex(&public_key),
            signature: response.signature.to_ascii_lowercase(),
        },
    };
    verify_signature_bytes(&envelope)?;
    Ok(envelope)
}

pub fn verify_release_envelope(
    envelope: &SignedReleaseEnvelope,
    expected_subject: &ReleaseSubject,
    authorized_keys: &[PublisherKey],
) -> Result<(), String> {
    if &envelope.subject != expected_subject {
        return Err(
            "signed release subject does not match the requested package artifact".to_string(),
        );
    }
    verify_signature_bytes(envelope)?;
    let authorized = authorized_keys.iter().any(|key| {
        key.key_id == envelope.signature.key_id
            && key
                .public_key
                .eq_ignore_ascii_case(&envelope.signature.public_key)
    });
    if !authorized {
        return Err(format!(
            "publisher key `{}` is not authorized for package `{}`",
            envelope.signature.key_id, envelope.subject.package
        ));
    }
    Ok(())
}

fn verify_signature_bytes(envelope: &SignedReleaseEnvelope) -> Result<(), String> {
    envelope.subject.validate()?;
    if envelope.signature.algorithm != "ed25519" {
        return Err(format!(
            "unsupported release signature algorithm `{}`",
            envelope.signature.algorithm
        ));
    }
    let public_key = decode_hex(&envelope.signature.public_key)?;
    if public_key.len() != 32 {
        return Err("ed25519 public key must contain 32 bytes".to_string());
    }
    let key_id = publisher_key_id(&public_key);
    if envelope.signature.key_id != key_id {
        return Err("release signature key id does not match its public key".to_string());
    }
    let signature = decode_hex(&envelope.signature.signature)?;
    if signature.len() != 64 {
        return Err("ed25519 signature must contain 64 bytes".to_string());
    }
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&envelope.subject.canonical_bytes()?, &signature)
        .map_err(|_| "release signature verification failed".to_string())
}

pub fn publisher_key_id(public_key: &[u8]) -> String {
    sha256_digest(public_key)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TransparencyEventKind {
    KeyRegistered {
        package: String,
        key_id: String,
        public_key: String,
    },
    KeyRevoked {
        package: String,
        key_id: String,
    },
    Release {
        package: String,
        version: String,
        subject_digest: String,
        key_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransparencyEvent {
    pub sequence: u64,
    #[serde(flatten)]
    pub kind: TransparencyEventKind,
}

impl TransparencyEvent {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = b"nomo-transparency-event-v1\n".to_vec();
        canonical_field(&mut out, "sequence", &self.sequence.to_string());
        match &self.kind {
            TransparencyEventKind::KeyRegistered {
                package,
                key_id,
                public_key,
            } => {
                canonical_field(&mut out, "type", "key-registered");
                canonical_field(&mut out, "package", package);
                canonical_field(&mut out, "key-id", key_id);
                canonical_field(&mut out, "public-key", public_key);
            }
            TransparencyEventKind::KeyRevoked { package, key_id } => {
                canonical_field(&mut out, "type", "key-revoked");
                canonical_field(&mut out, "package", package);
                canonical_field(&mut out, "key-id", key_id);
            }
            TransparencyEventKind::Release {
                package,
                version,
                subject_digest,
                key_id,
            } => {
                canonical_field(&mut out, "type", "release");
                canonical_field(&mut out, "package", package);
                canonical_field(&mut out, "version", version);
                canonical_field(&mut out, "subject", subject_digest);
                canonical_field(&mut out, "key-id", key_id);
            }
        }
        Ok(out)
    }

    fn validate(&self) -> Result<(), String> {
        match &self.kind {
            TransparencyEventKind::KeyRegistered {
                package,
                key_id,
                public_key,
            } => {
                validate_package_id(package)?;
                let public_key_bytes = decode_hex(public_key)?;
                if public_key_bytes.len() != 32 || publisher_key_id(&public_key_bytes) != *key_id {
                    return Err(
                        "transparency key registration has an invalid key identity".to_string()
                    );
                }
            }
            TransparencyEventKind::KeyRevoked { package, key_id } => {
                validate_package_id(package)?;
                validate_sha256("revoked publisher key id", key_id)?;
            }
            TransparencyEventKind::Release {
                package,
                version,
                subject_digest,
                key_id,
            } => {
                validate_package_id(package)?;
                validate_version_like("transparency release version", version)?;
                validate_sha256("release subject digest", subject_digest)?;
                validate_sha256("release publisher key id", key_id)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProofSide {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofStep {
    pub side: ProofSide,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InclusionProof {
    pub leaf_index: u64,
    pub tree_size: u64,
    pub siblings: Vec<ProofStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedTreeHead {
    pub schema: u32,
    pub log_id: String,
    pub tree_size: u64,
    pub root_hash: String,
    pub issued_at_unix_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_tree_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_root_hash: Option<String>,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

impl SignedTreeHead {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = b"nomo-transparency-head-v2\n".to_vec();
        canonical_field(&mut out, "schema", &self.schema.to_string());
        canonical_field(&mut out, "log-id", &self.log_id);
        canonical_field(&mut out, "tree-size", &self.tree_size.to_string());
        validate_sha256("transparency root hash", &self.root_hash)?;
        canonical_field(&mut out, "root-hash", &self.root_hash);
        canonical_field(
            &mut out,
            "issued-at",
            &self.issued_at_unix_seconds.to_string(),
        );
        canonical_field(
            &mut out,
            "previous-size",
            &self
                .previous_tree_size
                .map(|value| value.to_string())
                .unwrap_or_default(),
        );
        canonical_field(
            &mut out,
            "previous-root",
            self.previous_root_hash.as_deref().unwrap_or(""),
        );
        canonical_field(&mut out, "algorithm", &self.algorithm);
        canonical_field(&mut out, "key-id", &self.key_id);
        Ok(out)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != TRANSPARENCY_HEAD_SCHEMA {
            return Err(format!(
                "unsupported transparency tree-head schema {}",
                self.schema
            ));
        }
        if self.log_id.trim().is_empty() {
            return Err("transparency tree head must include a non-empty log id".to_string());
        }
        if self.tree_size == 0 {
            return Err("transparency tree head must contain at least one event".to_string());
        }
        validate_sha256("transparency root hash", &self.root_hash)?;
        if self.issued_at_unix_seconds == 0 {
            return Err("transparency tree head must include an issuance timestamp".to_string());
        }
        match (&self.previous_tree_size, &self.previous_root_hash) {
            (None, None) => {}
            (Some(size), Some(root)) if *size < self.tree_size => {
                validate_sha256("previous transparency root hash", root)?;
            }
            (Some(_), Some(_)) => {
                return Err(
                    "previous transparency tree size must be smaller than the current size"
                        .to_string(),
                );
            }
            _ => {
                return Err(
                    "previous transparency tree size and root hash must be provided together"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogKeyRotation {
    pub schema: u32,
    pub log_id: String,
    pub activate_at_tree_size: u64,
    pub old_key_id: String,
    pub old_public_key: String,
    pub new_key_id: String,
    pub new_public_key: String,
    pub old_signature: String,
    pub new_signature: String,
}

impl LogKeyRotation {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_identity()?;
        let mut out = b"nomo-transparency-log-key-rotation-v1\n".to_vec();
        canonical_field(&mut out, "schema", &self.schema.to_string());
        canonical_field(&mut out, "log-id", &self.log_id);
        canonical_field(
            &mut out,
            "activate-at-tree-size",
            &self.activate_at_tree_size.to_string(),
        );
        canonical_field(&mut out, "old-key-id", &self.old_key_id);
        canonical_field(&mut out, "old-public-key", &self.old_public_key);
        canonical_field(&mut out, "new-key-id", &self.new_key_id);
        canonical_field(&mut out, "new-public-key", &self.new_public_key);
        Ok(out)
    }

    fn validate_identity(&self) -> Result<(), String> {
        if self.schema != LOG_KEY_ROTATION_SCHEMA {
            return Err(format!(
                "unsupported transparency log-key rotation schema {}",
                self.schema
            ));
        }
        if self.log_id.trim().is_empty() {
            return Err("transparency log-key rotation must include a log id".to_string());
        }
        if self.activate_at_tree_size == 0 {
            return Err(
                "transparency log-key rotation activation size must be positive".to_string(),
            );
        }
        validate_log_key_identity("old", &self.old_key_id, &self.old_public_key)?;
        validate_log_key_identity("new", &self.new_key_id, &self.new_public_key)?;
        if self.old_key_id == self.new_key_id {
            return Err("transparency log-key rotation must change the key".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogInclusion {
    pub event: TransparencyEvent,
    pub proof: InclusionProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransparencyBundle {
    pub head: SignedTreeHead,
    pub log_public_key: String,
    #[serde(default)]
    pub log_key_rotations: Vec<LogKeyRotation>,
    #[serde(default)]
    pub head_history: Vec<SignedTreeHead>,
    pub release: LogInclusion,
    pub key_events: Vec<LogInclusion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedTreeHead {
    pub tree_size: u64,
    pub root_hash: String,
    #[serde(default)]
    pub issued_at_unix_seconds: u64,
    #[serde(default)]
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GossipCheckpoint {
    pub observed_at_unix_seconds: u64,
    pub head: SignedTreeHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofFreshnessPolicy {
    pub max_age_seconds: u64,
    pub offline_max_age_seconds: u64,
    pub max_future_skew_seconds: u64,
}

impl Default for ProofFreshnessPolicy {
    fn default() -> Self {
        Self {
            max_age_seconds: DEFAULT_PROOF_MAX_AGE_SECONDS,
            offline_max_age_seconds: DEFAULT_OFFLINE_PROOF_MAX_AGE_SECONDS,
            max_future_skew_seconds: DEFAULT_PROOF_MAX_FUTURE_SKEW_SECONDS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransparencyVerificationPolicy {
    pub trusted_log_keys: Vec<String>,
    pub freshness: ProofFreshnessPolicy,
    pub gossip_checkpoints: Vec<GossipCheckpoint>,
}

impl TransparencyVerificationPolicy {
    pub fn pinned(trusted_log_keys: Vec<String>) -> Self {
        Self {
            trusted_log_keys,
            freshness: ProofFreshnessPolicy::default(),
            gossip_checkpoints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTransparency {
    pub cached_head: CachedTreeHead,
    pub gossip_checkpoint: GossipCheckpoint,
}

#[derive(Debug, Clone)]
pub struct TransparencyLog {
    events: Vec<TransparencyEvent>,
}

impl TransparencyLog {
    pub fn new(events: Vec<TransparencyEvent>) -> Result<Self, String> {
        if events.is_empty() {
            return Err("transparency log must contain at least one event".to_string());
        }
        for (index, event) in events.iter().enumerate() {
            if event.sequence != index as u64 {
                return Err(format!(
                    "transparency event sequence {} does not match index {index}",
                    event.sequence
                ));
            }
            event.validate()?;
        }
        Ok(Self { events })
    }

    pub fn root_hash(&self) -> Result<String, String> {
        Ok(format!(
            "sha256:{}",
            encode_hex(&merkle_root(&self.leaf_hashes()?))
        ))
    }

    pub fn inclusion(&self, index: usize) -> Result<LogInclusion, String> {
        let event = self
            .events
            .get(index)
            .cloned()
            .ok_or_else(|| format!("transparency event index {index} is out of range"))?;
        let mut level = self.leaf_hashes()?;
        let mut current = index;
        let mut siblings = Vec::new();
        while level.len() > 1 {
            if current % 2 == 1 {
                siblings.push(ProofStep {
                    side: ProofSide::Left,
                    hash: encode_hex(&level[current - 1]),
                });
            } else if current + 1 < level.len() {
                siblings.push(ProofStep {
                    side: ProofSide::Right,
                    hash: encode_hex(&level[current + 1]),
                });
            }
            level = next_merkle_level(&level);
            current /= 2;
        }
        Ok(LogInclusion {
            event,
            proof: InclusionProof {
                leaf_index: index as u64,
                tree_size: self.events.len() as u64,
                siblings,
            },
        })
    }

    fn leaf_hashes(&self) -> Result<Vec<[u8; 32]>, String> {
        self.events
            .iter()
            .map(|event| event.canonical_bytes().map(|bytes| leaf_hash(&bytes)))
            .collect()
    }
}

pub fn verify_transparency_bundle(
    bundle: &TransparencyBundle,
    envelope: &SignedReleaseEnvelope,
    cached_head: Option<&CachedTreeHead>,
    trusted_log_keys: &[String],
) -> Result<CachedTreeHead, String> {
    let policy = TransparencyVerificationPolicy::pinned(trusted_log_keys.to_vec());
    Ok(verify_transparency_bundle_with_policy(
        bundle,
        envelope,
        cached_head,
        &policy,
        false,
        current_unix_seconds(),
    )?
    .cached_head)
}

pub fn verify_transparency_bundle_with_policy(
    bundle: &TransparencyBundle,
    envelope: &SignedReleaseEnvelope,
    cached_head: Option<&CachedTreeHead>,
    policy: &TransparencyVerificationPolicy,
    offline: bool,
    now_unix_seconds: u64,
) -> Result<VerifiedTransparency, String> {
    validate_transparency_policy(policy)?;
    let timeline = build_log_key_timeline(bundle, &policy.trusted_log_keys)?;
    verify_tree_head_with_timeline(&bundle.head, &timeline)?;
    for head in &bundle.head_history {
        verify_tree_head_with_timeline(head, &timeline)?;
    }
    verify_head_freshness(&bundle.head, policy.freshness, offline, now_unix_seconds)?;
    let head_history = indexed_head_history(bundle)?;
    if let Some(cached) = cached_head {
        validate_sha256("cached transparency root hash", &cached.root_hash)?;
        verify_head_extends_anchor(
            &bundle.head,
            &head_history,
            cached.tree_size,
            &cached.root_hash,
            "cached tree head",
        )?;
    }
    for gossip in &policy.gossip_checkpoints {
        verify_gossip_checkpoint(
            gossip,
            &bundle.head,
            &head_history,
            &timeline,
            policy.freshness.max_future_skew_seconds,
            now_unix_seconds,
        )?;
    }
    verify_log_inclusion(&bundle.release, &bundle.head)?;
    for event in &bundle.key_events {
        verify_log_inclusion(event, &bundle.head)?;
    }

    let release_sequence = bundle.release.event.sequence;
    let (release_package, release_version, subject_digest, release_key_id) =
        match &bundle.release.event.kind {
            TransparencyEventKind::Release {
                package,
                version,
                subject_digest,
                key_id,
            } => (package, version, subject_digest, key_id),
            _ => return Err("transparency bundle release entry is not a release event".to_string()),
        };
    if release_package != &envelope.subject.package
        || release_version != &envelope.subject.version
        || subject_digest != &envelope.subject.digest()?
        || release_key_id != &envelope.signature.key_id
    {
        return Err("transparency release event does not match the signed release".to_string());
    }

    let mut key_state = BTreeMap::<String, Option<String>>::new();
    let mut key_events = bundle.key_events.iter().collect::<Vec<_>>();
    key_events.sort_by_key(|event| event.event.sequence);
    for inclusion in key_events {
        if inclusion.event.sequence >= release_sequence {
            continue;
        }
        match &inclusion.event.kind {
            TransparencyEventKind::KeyRegistered {
                package,
                key_id,
                public_key,
            } if package == release_package => {
                key_state.insert(key_id.clone(), Some(public_key.clone()));
            }
            TransparencyEventKind::KeyRevoked { package, key_id } if package == release_package => {
                key_state.insert(key_id.clone(), None);
            }
            _ => {}
        }
    }
    let active_public_key = key_state
        .get(release_key_id)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            format!("publisher key `{release_key_id}` was not active when the release was logged")
        })?;
    verify_release_envelope(
        envelope,
        &envelope.subject,
        &[PublisherKey {
            key_id: release_key_id.clone(),
            public_key: active_public_key.clone(),
        }],
    )?;

    let cached_head = CachedTreeHead {
        tree_size: bundle.head.tree_size,
        root_hash: bundle.head.root_hash.clone(),
        issued_at_unix_seconds: bundle.head.issued_at_unix_seconds,
        key_id: bundle.head.key_id.clone(),
    };
    Ok(VerifiedTransparency {
        cached_head,
        gossip_checkpoint: GossipCheckpoint {
            observed_at_unix_seconds: now_unix_seconds,
            head: bundle.head.clone(),
        },
    })
}

#[derive(Debug, Clone)]
struct LogKeyTimeline {
    log_id: String,
    initial_public_key: String,
    rotations: Vec<LogKeyRotation>,
}

impl LogKeyTimeline {
    fn public_key_at(&self, tree_size: u64) -> &str {
        let mut public_key = self.initial_public_key.as_str();
        for rotation in &self.rotations {
            if tree_size < rotation.activate_at_tree_size {
                break;
            }
            public_key = &rotation.new_public_key;
        }
        public_key
    }
}

fn build_log_key_timeline(
    bundle: &TransparencyBundle,
    trusted_log_keys: &[String],
) -> Result<LogKeyTimeline, String> {
    validate_log_key_identity("current", &bundle.head.key_id, &bundle.log_public_key)?;
    if bundle.log_key_rotations.is_empty() {
        if !trusted_log_keys
            .iter()
            .any(|key| key.eq_ignore_ascii_case(&bundle.log_public_key))
        {
            return Err("transparency log public key is not trusted by policy".to_string());
        }
        return Ok(LogKeyTimeline {
            log_id: bundle.head.log_id.clone(),
            initial_public_key: bundle.log_public_key.to_ascii_lowercase(),
            rotations: Vec::new(),
        });
    }

    let first = &bundle.log_key_rotations[0];
    if !trusted_log_keys
        .iter()
        .any(|key| key.eq_ignore_ascii_case(&first.old_public_key))
    {
        return Err(
            "transparency log-key rotation chain does not start at a policy-pinned key".to_string(),
        );
    }
    let mut expected_public_key = first.old_public_key.to_ascii_lowercase();
    let mut previous_activation = 0;
    for rotation in &bundle.log_key_rotations {
        rotation.validate_identity()?;
        if rotation.log_id != bundle.head.log_id {
            return Err("transparency log-key rotation targets a different log id".to_string());
        }
        if !rotation
            .old_public_key
            .eq_ignore_ascii_case(&expected_public_key)
        {
            return Err("transparency log-key rotation chain is discontinuous".to_string());
        }
        if rotation.activate_at_tree_size <= previous_activation
            || rotation.activate_at_tree_size > bundle.head.tree_size
        {
            return Err(
                "transparency log-key rotation activation sizes must increase within the verified tree"
                    .to_string(),
            );
        }
        verify_log_key_rotation(rotation)?;
        previous_activation = rotation.activate_at_tree_size;
        expected_public_key = rotation.new_public_key.to_ascii_lowercase();
    }
    if !expected_public_key.eq_ignore_ascii_case(&bundle.log_public_key) {
        return Err(
            "transparency log-key rotation chain does not reach the current tree-head key"
                .to_string(),
        );
    }
    Ok(LogKeyTimeline {
        log_id: bundle.head.log_id.clone(),
        initial_public_key: first.old_public_key.to_ascii_lowercase(),
        rotations: bundle.log_key_rotations.clone(),
    })
}

fn verify_log_key_rotation(rotation: &LogKeyRotation) -> Result<(), String> {
    let canonical = rotation.canonical_bytes()?;
    verify_ed25519_signature(
        "old transparency log key",
        &rotation.old_public_key,
        &rotation.old_signature,
        &canonical,
    )?;
    verify_ed25519_signature(
        "new transparency log key",
        &rotation.new_public_key,
        &rotation.new_signature,
        &canonical,
    )
}

fn validate_log_key_identity(label: &str, key_id: &str, public_key: &str) -> Result<(), String> {
    let public_key = decode_hex(public_key)?;
    if public_key.len() != 32 || publisher_key_id(&public_key) != key_id {
        return Err(format!("{label} transparency log key identity is invalid"));
    }
    Ok(())
}

fn verify_ed25519_signature(
    label: &str,
    public_key: &str,
    signature: &str,
    message: &[u8],
) -> Result<(), String> {
    let public_key = decode_hex(public_key)?;
    if public_key.len() != 32 {
        return Err(format!("{label} public key must contain 32 bytes"));
    }
    let signature = decode_hex(signature)?;
    if signature.len() != 64 {
        return Err(format!("{label} signature must contain 64 bytes"));
    }
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(message, &signature)
        .map_err(|_| format!("{label} signature verification failed"))
}

fn verify_tree_head_with_timeline(
    head: &SignedTreeHead,
    timeline: &LogKeyTimeline,
) -> Result<(), String> {
    if head.log_id != timeline.log_id {
        return Err("transparency tree head targets a different log id".to_string());
    }
    verify_tree_head(head, timeline.public_key_at(head.tree_size))
}

fn validate_transparency_policy(policy: &TransparencyVerificationPolicy) -> Result<(), String> {
    if policy.trusted_log_keys.is_empty() {
        return Err("transparency policy must pin at least one log key".to_string());
    }
    for key in &policy.trusted_log_keys {
        let bytes = decode_hex(key)?;
        if bytes.len() != 32 {
            return Err("transparency policy log keys must contain 32 bytes".to_string());
        }
    }
    if policy.freshness.max_age_seconds == 0
        || policy.freshness.offline_max_age_seconds < policy.freshness.max_age_seconds
    {
        return Err(
            "offline transparency proof age must be at least the positive online proof age"
                .to_string(),
        );
    }
    Ok(())
}

fn verify_head_freshness(
    head: &SignedTreeHead,
    policy: ProofFreshnessPolicy,
    offline: bool,
    now_unix_seconds: u64,
) -> Result<(), String> {
    if now_unix_seconds == 0 {
        return Err("transparency verification time must be positive".to_string());
    }
    if head.issued_at_unix_seconds > now_unix_seconds.saturating_add(policy.max_future_skew_seconds)
    {
        return Err("transparency proof tree head is too far in the future".to_string());
    }
    let age = now_unix_seconds.saturating_sub(head.issued_at_unix_seconds);
    let maximum = if offline {
        policy.offline_max_age_seconds
    } else {
        policy.max_age_seconds
    };
    if age > maximum {
        return Err(format!(
            "transparency proof is stale: tree head age {age}s exceeds {maximum}s {} limit",
            if offline { "offline" } else { "online" }
        ));
    }
    Ok(())
}

fn indexed_head_history(
    bundle: &TransparencyBundle,
) -> Result<BTreeMap<u64, &SignedTreeHead>, String> {
    let mut heads = BTreeMap::new();
    for head in bundle
        .head_history
        .iter()
        .chain(std::iter::once(&bundle.head))
    {
        if let Some(existing) = heads.insert(head.tree_size, head) {
            if existing.root_hash != head.root_hash {
                return Err(
                    "transparency head history contains equivocation at one tree size".to_string(),
                );
            }
            return Err("transparency head history contains a duplicate tree size".to_string());
        }
    }
    Ok(heads)
}

fn verify_head_extends_anchor(
    current: &SignedTreeHead,
    history: &BTreeMap<u64, &SignedTreeHead>,
    anchor_size: u64,
    anchor_root: &str,
    anchor_label: &str,
) -> Result<(), String> {
    if current.tree_size < anchor_size {
        return Err(format!(
            "transparency log rollback detected: {anchor_label} size {anchor_size}, received {}",
            current.tree_size
        ));
    }
    if current.tree_size == anchor_size {
        return if current.root_hash == anchor_root {
            Ok(())
        } else {
            Err(format!(
                "transparency log equivocation detected against {anchor_label}"
            ))
        };
    }

    let mut cursor = current;
    let mut visited = BTreeMap::new();
    loop {
        if visited.insert(cursor.tree_size, ()).is_some() {
            return Err("transparency head history contains a cycle".to_string());
        }
        let (previous_size, previous_root) = cursor
            .previous_tree_size
            .zip(cursor.previous_root_hash.as_deref())
            .ok_or_else(|| {
                format!(
                    "transparency consistency chain does not reach {anchor_label} size {anchor_size}"
                )
            })?;
        if previous_size == anchor_size {
            return if previous_root == anchor_root {
                Ok(())
            } else {
                Err(format!(
                    "transparency log equivocation detected against {anchor_label}"
                ))
            };
        }
        if previous_size < anchor_size {
            return Err(format!(
                "transparency consistency chain skips {anchor_label} size {anchor_size}"
            ));
        }
        cursor = history.get(&previous_size).copied().ok_or_else(|| {
            format!(
                "transparency consistency chain is missing tree head {previous_size} required by {anchor_label}"
            )
        })?;
        if cursor.root_hash != previous_root {
            return Err("transparency head history predecessor root does not match".to_string());
        }
    }
}

fn verify_gossip_checkpoint(
    gossip: &GossipCheckpoint,
    current: &SignedTreeHead,
    history: &BTreeMap<u64, &SignedTreeHead>,
    timeline: &LogKeyTimeline,
    max_future_skew_seconds: u64,
    now_unix_seconds: u64,
) -> Result<(), String> {
    verify_tree_head_with_timeline(&gossip.head, timeline)?;
    if gossip.observed_at_unix_seconds < gossip.head.issued_at_unix_seconds {
        return Err("gossip checkpoint was observed before its tree head was issued".to_string());
    }
    if gossip.observed_at_unix_seconds > now_unix_seconds.saturating_add(max_future_skew_seconds) {
        return Err("gossip checkpoint observation time is too far in the future".to_string());
    }
    verify_head_extends_anchor(
        current,
        history,
        gossip.head.tree_size,
        &gossip.head.root_hash,
        "gossip checkpoint",
    )
}

pub fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn verify_tree_head(head: &SignedTreeHead, public_key: &str) -> Result<(), String> {
    head.validate()?;
    if head.algorithm != "ed25519" {
        return Err(format!(
            "unsupported transparency signature algorithm `{}`",
            head.algorithm
        ));
    }
    let public_key = decode_hex(public_key)?;
    if public_key.len() != 32 || publisher_key_id(&public_key) != head.key_id {
        return Err("transparency tree-head key identity is invalid".to_string());
    }
    let signature = decode_hex(&head.signature)?;
    if signature.len() != 64 {
        return Err("transparency tree-head signature must contain 64 bytes".to_string());
    }
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&head.canonical_bytes()?, &signature)
        .map_err(|_| "transparency tree-head signature verification failed".to_string())
}

pub fn verify_log_inclusion(inclusion: &LogInclusion, head: &SignedTreeHead) -> Result<(), String> {
    if inclusion.proof.tree_size != head.tree_size
        || inclusion.proof.leaf_index >= inclusion.proof.tree_size
        || inclusion.proof.leaf_index != inclusion.event.sequence
    {
        return Err("transparency inclusion proof indices are inconsistent".to_string());
    }
    let mut hash = leaf_hash(&inclusion.event.canonical_bytes()?);
    for step in &inclusion.proof.siblings {
        let sibling = decode_hash(&step.hash)?;
        hash = match step.side {
            ProofSide::Left => node_hash(&sibling, &hash),
            ProofSide::Right => node_hash(&hash, &sibling),
        };
    }
    if format!("sha256:{}", encode_hex(&hash)) == head.root_hash {
        Ok(())
    } else {
        Err("transparency inclusion proof does not match the signed tree head".to_string())
    }
}

pub fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("hex value must contain an even number of hexadecimal digits".to_string());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            let text = std::str::from_utf8(chunk).expect("hex bytes are ASCII");
            u8::from_str_radix(text, 16).map_err(|err| err.to_string())
        })
        .collect()
}

fn validate_sha256(label: &str, digest: &str) -> Result<(), String> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(format!("{label} must use sha256:<hex>"));
    };
    if hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{label} must contain 64 hexadecimal digits"))
    }
}

fn canonical_field(out: &mut Vec<u8>, name: &str, value: &str) {
    out.extend_from_slice(name.as_bytes());
    out.push(b':');
    out.extend_from_slice(value.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(value.as_bytes());
    out.push(b'\n');
}

fn leaf_hash(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update([0]);
    digest.update(bytes);
    digest.finalize().into()
}

fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update([1]);
    digest.update(left);
    digest.update(right);
    digest.finalize().into()
}

fn next_merkle_level(level: &[[u8; 32]]) -> Vec<[u8; 32]> {
    level
        .chunks(2)
        .map(|pair| {
            if pair.len() == 2 {
                node_hash(&pair[0], &pair[1])
            } else {
                pair[0]
            }
        })
        .collect()
}

fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        level = next_merkle_level(&level);
    }
    level[0]
}

fn decode_hash(value: &str) -> Result<[u8; 32], String> {
    let bytes = decode_hex(value)?;
    bytes
        .try_into()
        .map_err(|_| "transparency proof hash must contain 32 bytes".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    const SEED_ONE: &str = "0101010101010101010101010101010101010101010101010101010101010101";
    const SEED_TWO: &str = "0202020202020202020202020202020202020202020202020202020202020202";

    fn key(seed: &str) -> Ed25519KeyPair {
        Ed25519KeyPair::from_seed_unchecked(&decode_hex(seed).unwrap()).unwrap()
    }

    fn subject(version: &str) -> ReleaseSubject {
        ReleaseSubject::new(
            "nomo-lang/demo".to_string(),
            version.to_string(),
            sha256_digest(b"archive"),
            sha256_digest(b"manifest"),
            Some(sha256_digest(b"provenance")),
        )
        .unwrap()
    }

    fn sign(subject: ReleaseSubject, pair: &Ed25519KeyPair) -> SignedReleaseEnvelope {
        envelope_from_signer_response(
            subject.clone(),
            ExternalSignerResponse {
                algorithm: "ed25519".to_string(),
                key_id: None,
                public_key: encode_hex(pair.public_key().as_ref()),
                signature: encode_hex(pair.sign(&subject.canonical_bytes().unwrap()).as_ref()),
            },
        )
        .unwrap()
    }

    fn signed_head(log: &TransparencyLog, pair: &Ed25519KeyPair) -> SignedTreeHead {
        signed_head_at(log, pair, current_unix_seconds(), None)
    }

    fn signed_head_at(
        log: &TransparencyLog,
        pair: &Ed25519KeyPair,
        issued_at_unix_seconds: u64,
        previous: Option<&SignedTreeHead>,
    ) -> SignedTreeHead {
        let mut head = SignedTreeHead {
            schema: TRANSPARENCY_HEAD_SCHEMA,
            log_id: "https://registry.example/transparency".to_string(),
            tree_size: log.events.len() as u64,
            root_hash: log.root_hash().unwrap(),
            issued_at_unix_seconds,
            previous_tree_size: previous.map(|head| head.tree_size),
            previous_root_hash: previous.map(|head| head.root_hash.clone()),
            algorithm: "ed25519".to_string(),
            key_id: publisher_key_id(pair.public_key().as_ref()),
            signature: String::new(),
        };
        head.signature = encode_hex(pair.sign(&head.canonical_bytes().unwrap()).as_ref());
        head
    }

    fn log_key_rotation(
        old: &Ed25519KeyPair,
        new: &Ed25519KeyPair,
        activate_at_tree_size: u64,
    ) -> LogKeyRotation {
        let mut rotation = LogKeyRotation {
            schema: LOG_KEY_ROTATION_SCHEMA,
            log_id: "https://registry.example/transparency".to_string(),
            activate_at_tree_size,
            old_key_id: publisher_key_id(old.public_key().as_ref()),
            old_public_key: encode_hex(old.public_key().as_ref()),
            new_key_id: publisher_key_id(new.public_key().as_ref()),
            new_public_key: encode_hex(new.public_key().as_ref()),
            old_signature: String::new(),
            new_signature: String::new(),
        };
        let canonical = rotation.canonical_bytes().unwrap();
        rotation.old_signature = encode_hex(old.sign(&canonical).as_ref());
        rotation.new_signature = encode_hex(new.sign(&canonical).as_ref());
        rotation
    }

    #[test]
    fn verifies_rfc8032_empty_message_vector() {
        let public =
            decode_hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a").unwrap();
        let signature = decode_hex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        )
        .unwrap();
        UnparsedPublicKey::new(&ED25519, public)
            .verify(b"", &signature)
            .unwrap();
    }

    #[test]
    fn release_encoding_and_signature_are_deterministic_and_authorized() {
        let pair = key(SEED_ONE);
        let subject = subject("1.0.0");
        let envelope = sign(subject.clone(), &pair);
        let authorized = PublisherKey {
            key_id: publisher_key_id(pair.public_key().as_ref()),
            public_key: encode_hex(pair.public_key().as_ref()),
        };
        verify_release_envelope(&envelope, &subject, &[authorized]).unwrap();
        assert_eq!(
            subject.canonical_bytes().unwrap(),
            subject.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn inclusion_rotation_revocation_and_rollback_are_verified_without_secrets() {
        let publisher_one = key(SEED_ONE);
        let publisher_two = key(SEED_TWO);
        let log_key = key(SEED_TWO);
        let public_one = encode_hex(publisher_one.public_key().as_ref());
        let public_two = encode_hex(publisher_two.public_key().as_ref());
        let key_one = publisher_key_id(publisher_one.public_key().as_ref());
        let key_two = publisher_key_id(publisher_two.public_key().as_ref());
        let release = sign(subject("1.1.0"), &publisher_two);
        let events = vec![
            TransparencyEvent {
                sequence: 0,
                kind: TransparencyEventKind::KeyRegistered {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_one.clone(),
                    public_key: public_one,
                },
            },
            TransparencyEvent {
                sequence: 1,
                kind: TransparencyEventKind::KeyRegistered {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_two.clone(),
                    public_key: public_two,
                },
            },
            TransparencyEvent {
                sequence: 2,
                kind: TransparencyEventKind::KeyRevoked {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_one,
                },
            },
            TransparencyEvent {
                sequence: 3,
                kind: TransparencyEventKind::Release {
                    package: "nomo-lang/demo".to_string(),
                    version: "1.1.0".to_string(),
                    subject_digest: release.subject.digest().unwrap(),
                    key_id: key_two,
                },
            },
        ];
        let log = TransparencyLog::new(events).unwrap();
        let bundle = TransparencyBundle {
            head: signed_head(&log, &log_key),
            log_public_key: encode_hex(log_key.public_key().as_ref()),
            log_key_rotations: Vec::new(),
            head_history: Vec::new(),
            release: log.inclusion(3).unwrap(),
            key_events: vec![
                log.inclusion(0).unwrap(),
                log.inclusion(1).unwrap(),
                log.inclusion(2).unwrap(),
            ],
        };
        let verified = verify_transparency_bundle(
            &bundle,
            &release,
            None,
            std::slice::from_ref(&bundle.log_public_key),
        )
        .unwrap();
        let untrusted = encode_hex(key(SEED_ONE).public_key().as_ref());
        let error =
            verify_transparency_bundle(&bundle, &release, None, std::slice::from_ref(&untrusted))
                .unwrap_err();
        assert!(error.contains("not trusted by policy"), "{error}");
        let rollback = CachedTreeHead {
            tree_size: verified.tree_size + 1,
            root_hash: sha256_digest(b"newer"),
            issued_at_unix_seconds: current_unix_seconds(),
            key_id: bundle.head.key_id.clone(),
        };
        let error = verify_transparency_bundle(
            &bundle,
            &release,
            Some(&rollback),
            std::slice::from_ref(&bundle.log_public_key),
        )
        .unwrap_err();
        assert!(error.contains("rollback"), "{error}");

        let serialized = serde_json::to_string(&bundle).unwrap();
        assert!(!serialized.contains(SEED_ONE));
        assert!(!serialized.contains(SEED_TWO));
    }

    #[test]
    fn revoked_key_cannot_authorize_a_later_release() {
        let publisher = key(SEED_ONE);
        let log_key = key(SEED_TWO);
        let public = encode_hex(publisher.public_key().as_ref());
        let key_id = publisher_key_id(publisher.public_key().as_ref());
        let release = sign(subject("1.2.0"), &publisher);
        let log = TransparencyLog::new(vec![
            TransparencyEvent {
                sequence: 0,
                kind: TransparencyEventKind::KeyRegistered {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_id.clone(),
                    public_key: public,
                },
            },
            TransparencyEvent {
                sequence: 1,
                kind: TransparencyEventKind::KeyRevoked {
                    package: "nomo-lang/demo".to_string(),
                    key_id: key_id.clone(),
                },
            },
            TransparencyEvent {
                sequence: 2,
                kind: TransparencyEventKind::Release {
                    package: "nomo-lang/demo".to_string(),
                    version: "1.2.0".to_string(),
                    subject_digest: release.subject.digest().unwrap(),
                    key_id,
                },
            },
        ])
        .unwrap();
        let bundle = TransparencyBundle {
            head: signed_head(&log, &log_key),
            log_public_key: encode_hex(log_key.public_key().as_ref()),
            log_key_rotations: Vec::new(),
            head_history: Vec::new(),
            release: log.inclusion(2).unwrap(),
            key_events: vec![log.inclusion(0).unwrap(), log.inclusion(1).unwrap()],
        };
        let error = verify_transparency_bundle(
            &bundle,
            &release,
            None,
            std::slice::from_ref(&bundle.log_public_key),
        )
        .unwrap_err();
        assert!(error.contains("not active"), "{error}");
    }

    #[test]
    fn log_key_rotation_gossip_and_online_offline_freshness_are_enforced() {
        let publisher = key(SEED_ONE);
        let old_log_key = key(SEED_ONE);
        let new_log_key = key(SEED_TWO);
        let publisher_public = encode_hex(publisher.public_key().as_ref());
        let publisher_id = publisher_key_id(publisher.public_key().as_ref());
        let release = sign(subject("2.0.0"), &publisher);
        let first_log = TransparencyLog::new(vec![TransparencyEvent {
            sequence: 0,
            kind: TransparencyEventKind::KeyRegistered {
                package: "nomo-lang/demo".to_string(),
                key_id: publisher_id.clone(),
                public_key: publisher_public.clone(),
            },
        }])
        .unwrap();
        let first_head = signed_head_at(&first_log, &old_log_key, 1_000, None);
        let current_log = TransparencyLog::new(vec![
            TransparencyEvent {
                sequence: 0,
                kind: TransparencyEventKind::KeyRegistered {
                    package: "nomo-lang/demo".to_string(),
                    key_id: publisher_id.clone(),
                    public_key: publisher_public,
                },
            },
            TransparencyEvent {
                sequence: 1,
                kind: TransparencyEventKind::Release {
                    package: "nomo-lang/demo".to_string(),
                    version: "2.0.0".to_string(),
                    subject_digest: release.subject.digest().unwrap(),
                    key_id: publisher_id,
                },
            },
        ])
        .unwrap();
        let current_head = signed_head_at(&current_log, &new_log_key, 1_100, Some(&first_head));
        let bundle = TransparencyBundle {
            head: current_head.clone(),
            log_public_key: encode_hex(new_log_key.public_key().as_ref()),
            log_key_rotations: vec![log_key_rotation(&old_log_key, &new_log_key, 2)],
            head_history: vec![first_head.clone()],
            release: current_log.inclusion(1).unwrap(),
            key_events: vec![current_log.inclusion(0).unwrap()],
        };
        let policy = TransparencyVerificationPolicy {
            trusted_log_keys: vec![encode_hex(old_log_key.public_key().as_ref())],
            freshness: ProofFreshnessPolicy {
                max_age_seconds: 200,
                offline_max_age_seconds: 1_000,
                max_future_skew_seconds: 10,
            },
            gossip_checkpoints: vec![GossipCheckpoint {
                observed_at_unix_seconds: 1_050,
                head: first_head.clone(),
            }],
        };
        let verified =
            verify_transparency_bundle_with_policy(&bundle, &release, None, &policy, false, 1_200)
                .unwrap();
        assert_eq!(verified.cached_head.key_id, current_head.key_id);

        let mut bad_rotation = bundle.clone();
        bad_rotation.log_key_rotations[0].old_signature = "00".repeat(64);
        let error = verify_transparency_bundle_with_policy(
            &bad_rotation,
            &release,
            None,
            &policy,
            false,
            1_200,
        )
        .unwrap_err();
        assert!(
            error.contains("old transparency log key signature"),
            "{error}"
        );

        let fork_log = TransparencyLog::new(vec![TransparencyEvent {
            sequence: 0,
            kind: TransparencyEventKind::KeyRegistered {
                package: "nomo-lang/other".to_string(),
                key_id: publisher_key_id(old_log_key.public_key().as_ref()),
                public_key: encode_hex(old_log_key.public_key().as_ref()),
            },
        }])
        .unwrap();
        let mut fork_policy = policy.clone();
        fork_policy.gossip_checkpoints[0] = GossipCheckpoint {
            observed_at_unix_seconds: 1_050,
            head: signed_head_at(&fork_log, &old_log_key, 1_000, None),
        };
        let error = verify_transparency_bundle_with_policy(
            &bundle,
            &release,
            None,
            &fork_policy,
            false,
            1_200,
        )
        .unwrap_err();
        assert!(error.contains("equivocation"), "{error}");

        let mut age_policy = policy;
        age_policy.gossip_checkpoints.clear();
        age_policy.freshness.max_age_seconds = 50;
        let error = verify_transparency_bundle_with_policy(
            &bundle,
            &release,
            None,
            &age_policy,
            false,
            1_200,
        )
        .unwrap_err();
        assert!(error.contains("stale"), "{error}");
        verify_transparency_bundle_with_policy(&bundle, &release, None, &age_policy, true, 1_200)
            .unwrap();
    }
}
