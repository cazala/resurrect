//! Regenerates the deterministic cross-language signed peer-record vector.

use rbp_libp2p::{Keypair, Multiaddr, sign_peer_record};

fn main() {
    let keypair = Keypair::ed25519_from_bytes([7_u8; 32]).expect("fixed key is valid");
    let address: Multiaddr = "/dns4/seed.example/tcp/443/wss"
        .parse()
        .expect("fixed multiaddr is valid");
    let record = sign_peer_record(&keypair, 42, std::slice::from_ref(&address))
        .expect("fixed record can be signed");
    let record_hex = hex::encode(record);
    println!("peerId={}", keypair.public().to_peer_id());
    println!("sequence=42");
    println!("endpoint={address}");
    println!("recordHex=0x{record_hex}");

    let native_address: Multiaddr = "/dns4/seed.example/tcp/4001"
        .parse()
        .expect("fixed multiaddr is valid");
    let native_record = sign_peer_record(&keypair, 43, std::slice::from_ref(&native_address))
        .expect("fixed record can be signed");
    let native_hex = hex::encode(native_record);
    println!("nativeSequence=43");
    println!("nativeEndpoint={native_address}");
    println!("nativeRecordHex=0x{native_hex}");
}
