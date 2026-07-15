pub use nomo_manifest::RegistryTrustPolicy as TrustPolicy;
use nomo_manifest::{validate_package_id, validate_version_like};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const RELEASE_ENVELOPE_SCHEMA: u32 = 1;
pub const PROVENANCE_SCHEMA: u32 = 1;

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
    pub tree_size: u64,
    pub root_hash: String,
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
}

impl SignedTreeHead {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        validate_sha256("transparency root hash", &self.root_hash)?;
        let mut out = b"nomo-transparency-head-v1\n".to_vec();
        canonical_field(&mut out, "tree-size", &self.tree_size.to_string());
        canonical_field(&mut out, "root-hash", &self.root_hash);
        Ok(out)
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
    pub release: LogInclusion,
    pub key_events: Vec<LogInclusion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedTreeHead {
    pub tree_size: u64,
    pub root_hash: String,
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
    if !trusted_log_keys
        .iter()
        .any(|key| key.eq_ignore_ascii_case(&bundle.log_public_key))
    {
        return Err("transparency log public key is not trusted by policy".to_string());
    }
    verify_tree_head(&bundle.head, &bundle.log_public_key)?;
    if let Some(cached) = cached_head {
        if bundle.head.tree_size < cached.tree_size {
            return Err(format!(
                "transparency log rollback detected: cached size {}, received {}",
                cached.tree_size, bundle.head.tree_size
            ));
        }
        if bundle.head.tree_size == cached.tree_size && bundle.head.root_hash != cached.root_hash {
            return Err(
                "transparency log equivocation detected at the cached tree size".to_string(),
            );
        }
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

    Ok(CachedTreeHead {
        tree_size: bundle.head.tree_size,
        root_hash: bundle.head.root_hash.clone(),
    })
}

pub fn verify_tree_head(head: &SignedTreeHead, public_key: &str) -> Result<(), String> {
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
        let mut head = SignedTreeHead {
            tree_size: log.events.len() as u64,
            root_hash: log.root_hash().unwrap(),
            algorithm: "ed25519".to_string(),
            key_id: publisher_key_id(pair.public_key().as_ref()),
            signature: String::new(),
        };
        head.signature = encode_hex(pair.sign(&head.canonical_bytes().unwrap()).as_ref());
        head
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
}
