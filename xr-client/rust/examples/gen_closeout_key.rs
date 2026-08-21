//! Mint a fresh close-out test keypair with the same semantics as
//! `NostrSigner::generate()` (secp256k1 0.29, x-only pubkey = nostr pubkey/DID).
//! Prints `SECRET_HEX` and `PUBKEY_HEX` to stdout; the caller redirects to a
//! scratchpad file OUTSIDE the repo. The pubkey is the DID (`did:nostr:<hex>`).

use secp256k1::Secp256k1;

fn main() {
    let secp = Secp256k1::new();
    let (sk, _pk) = secp.generate_keypair(&mut secp256k1::rand::thread_rng());
    let keypair = secp256k1::Keypair::from_secret_key(&secp, &sk);
    let (xonly, _parity) = keypair.x_only_public_key();
    let secret_hex = hex::encode(sk.secret_bytes());
    let pubkey_hex = hex::encode(xonly.serialize());
    println!("SECRET_HEX={secret_hex}");
    println!("PUBKEY_HEX={pubkey_hex}");
    println!("DID=did:nostr:{pubkey_hex}");
}
