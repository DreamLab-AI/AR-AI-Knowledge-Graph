// Type definitions for NIP-07 window.nostr based on the specification
// https://github.com/nostr-protocol/nips/blob/master/07.md

import type { Event as NostrEvent, UnsignedEvent } from 'nostr-tools';

// Define the structure of the event object passed to signEvent
// Note: NIP-07 specifies the input event lacks id, pubkey, sig.
// nostr-tools' UnsignedEvent fits this description.
type Nip07Event = Omit<UnsignedEvent, 'pubkey'>; // pubkey is added by the signer

// Define the interface for the window.nostr object
interface NostrProvider {
  getPublicKey(): Promise<string>; // Returns hex public key
  signEvent(event: Nip07Event): Promise<NostrEvent>; // Returns the full signed event

  // Optional NIP-44 methods
  nip44?: {
    encrypt(pubkey: string, plaintext: string): Promise<string>; // returns ciphertext
    decrypt(pubkey: string, ciphertext: string): Promise<string>; // returns plaintext
  };

  // Optional NIP-04 methods (Deprecated but might exist)
  nip04?: {
    encrypt(pubkey: string, plaintext: string): Promise<string>;
    decrypt(pubkey: string, ciphertext: string): Promise<string>;
  };

  // Optional: Get Relays method (Not in core NIP-07 spec but common)
  getRelays?(): Promise<{ [url: string]: { read: boolean; write: boolean } }>;
}

// Augment the global Window interface
declare global {
  interface Window {
    nostr?: NostrProvider;
  }
}

// Export an empty object to ensure this is treated as a module
export {};