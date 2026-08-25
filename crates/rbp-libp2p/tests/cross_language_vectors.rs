//! Cross-language conformance tests for checked-in signed record bytes.

use rbp_core::{DialContext, PeerRecordCodec};
use rbp_libp2p::{EndpointPolicy, Libp2pPeerRecordCodec};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vectors {
    peer_id: String,
    browser: Vector,
    native_only: Vector,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vector {
    sequence: u64,
    endpoint: String,
    record_hex: String,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(
        "../../../test-vectors/peer-records/libp2p-ed25519.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn rust_parses_shared_browser_vector() {
    let vectors = vectors();
    let bytes = hex::decode(vectors.browser.record_hex.trim_start_matches("0x")).unwrap();
    let codec = Libp2pPeerRecordCodec::new(EndpointPolicy::default(), 16);
    let candidate = codec
        .decode_and_verify(&bytes, DialContext::Browser)
        .await
        .unwrap();
    assert_eq!(candidate.sequence, vectors.browser.sequence);
    assert_eq!(candidate.endpoints[0].address, vectors.browser.endpoint);
    assert_eq!(
        libp2p_identity::PeerId::from_bytes(&candidate.peer_id)
            .unwrap()
            .to_string(),
        vectors.peer_id
    );
}

#[tokio::test]
async fn rust_native_vector_is_not_browser_dialable() {
    let vectors = vectors();
    let bytes = hex::decode(vectors.native_only.record_hex.trim_start_matches("0x")).unwrap();
    let codec = Libp2pPeerRecordCodec::new(EndpointPolicy::default(), 16);
    assert!(
        codec
            .decode_and_verify(&bytes, DialContext::Browser)
            .await
            .is_err()
    );
    let candidate = codec
        .decode_and_verify(&bytes, DialContext::NativeServer)
        .await
        .unwrap();
    assert_eq!(candidate.sequence, vectors.native_only.sequence);
    assert_eq!(candidate.endpoints[0].address, vectors.native_only.endpoint);
}
