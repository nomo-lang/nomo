# Transparency Log Operations

Nomo's `signed+transparent` policy treats the manifest-pinned Ed25519 key as a
root of trust, not as a key that must sign every future tree head. Production
logs can rotate that key, exchange signed checkpoints between clients, and
enforce explicit online and offline proof ages without weakening the original
pin.

## Signed tree heads

Tree-head schema v2 signs all of the following:

- a stable non-empty log id;
- the Merkle tree size and root hash;
- the Unix issuance time;
- the immediately preceding tree size and root, when one exists;
- the signature algorithm and signing-key id.

The predecessor fields form a signed checkpoint chain. When a client already
has a cached or gossiped checkpoint, a larger head must provide enough
`head_history` entries to walk back to that exact size and root. A smaller head
is rollback; a different root at the same size or a predecessor mismatch is
equivocation. The inclusion proof is still verified against the current Merkle
root.

## Log-key rotation

A `LogKeyRotation` names the log id, old and new public keys, derived key ids,
and the first tree size signed by the new key. Its canonical statement must be
signed by both keys. Clients then apply these rules:

1. The first old key must match a key pinned in `trust.transparency-keys`.
2. Every rotation must be dual-signed and connect exactly to the preceding new
   key.
3. Activation sizes must strictly increase and may not exceed the current tree.
4. Historical heads use the key active at their size; the current head must use
   the final key in the chain.

Operators should generate the new key offline, publish and gossip the
dual-signed transition before activation, retain the complete public rotation
chain indefinitely, and stop using the old private key at activation. Routine
rotation does not require projects to replace their original pin.

If the old private key is suspected compromised before it signs a transition,
the normal rotation chain is no longer sufficient. Operators must publish an
incident notice and distribute a new root key through the same out-of-band
channel used for the original manifest pin. Clients should not auto-trust an
unilateral new key supplied by registry metadata.

## Gossip checkpoints

A gossip checkpoint contains an observation time and the complete log-signed
tree head. The observation wrapper is not a new trust root: verification still
requires the pinned key or its dual-signed rotation chain. A peer checkpoint
must have a valid signature, the same log id, a plausible observation time, and
an exact path through the current head history.

Projects can list checkpoint files in `nomo.toml`:

```toml
[trust]
policy = "signed+transparent"
transparency-keys = ["<root-log-key>"]
gossip-checkpoints = ["trust/ci.json", "trust/mirror.json"]
```

Each file may contain one checkpoint or a JSON array. Paths are package-relative
and cannot escape the package root. After a resolver verification, Nomo writes
the latest checkpoint to:

```text
.nomo/cache/registry/trust/<registry-id>/gossip-checkpoint.json
```

That file can be published as a CI artifact, mirrored, or compared by an
independent monitor. The standalone verifier supports the same flow:

```bash
nomo verify package.nomo-package \
  --envelope envelope.json --key <publisher-key> \
  --transparency bundle.json --log-key <pinned-log-key> \
  --gossip peer.json --write-gossip latest.json
```

Operators should gossip every externally published head and retain checkpoints
long enough to cover the offline freshness window. Any rollback, equivocation,
missing history, invalid rotation signature, or log-id mismatch is a hard
verification failure and should stop package promotion.

## Proof freshness

The default policy is:

| Mode | Maximum head age |
| --- | ---: |
| Online | 86,400 seconds (24 hours) |
| Offline | 604,800 seconds (7 days) |
| Future clock skew | 300 seconds (5 minutes) |

Configure it per project:

```toml
[trust]
proof-max-age-seconds = 43200
offline-proof-max-age-seconds = 259200
max-future-skew-seconds = 120
```

Both age limits must be positive and the offline limit must not be shorter than
the online limit. The standalone verifier exposes matching flags plus
`--offline`. Freshness applies to the current signed tree head; an old gossip
checkpoint may still serve as a consistency anchor if the current proof is
fresh.

Air-gapped environments should import a bundle, rotation chain, and at least
one independently distributed checkpoint together. Extending the offline age
is an explicit local policy decision; it never permits a different log key or
suppresses rollback/equivocation checks.
