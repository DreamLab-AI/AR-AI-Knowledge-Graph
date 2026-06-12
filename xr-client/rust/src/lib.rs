//! VisionClaw Quest 3 native APK — gdext hot-path crate (PRD-008).
//!
//! Owns:
//! - 0x03 V3 graph position frame decode (`binary_protocol`)
//! - 0x43 avatar pose presence client (`presence`) — wire format from
//!   `visionclaw_xr_presence::wire`
//! - BIP-340 Schnorr challenge signing (`signer`)
//! - tokio-tungstenite sockets + main-thread inbox pumps (`transport`, `runtime`)
//! - hand-tracking ray cast + pinch detection (`interaction`)
//! - distance-bucket LOD policy (`lod`)
//! - spatial voice routing surface (`webrtc_audio`)
//!
//! GDScript drives scene composition only; this crate owns every byte that
//! crosses the wire and every threshold that gates a pose / hit / level.

pub mod binary_protocol;
pub mod interaction;
pub mod lod;
pub mod ports;
pub mod presence;
pub mod runtime;
pub mod signer;
pub mod transport;
pub mod webrtc_audio;

#[cfg(not(test))]
use godot::prelude::*;

#[cfg(not(test))]
struct VisionclawXrExtension;

#[cfg(not(test))]
#[gdextension]
unsafe impl ExtensionLibrary for VisionclawXrExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            init_tracing();
        }
    }
}

/// Install a `tracing` subscriber once the Scene level is up. Without this every
/// `tracing::{error,warn,info}` in the transport/presence path emits to no
/// subscriber and is silently dropped. Quest builds route to logcat via
/// `tracing-android`; the desktop sidecar prints to stdout so connection
/// attempts and decode errors surface in `docker logs`.
#[cfg(not(test))]
fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        #[cfg(target_os = "android")]
        {
            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;
            if let Ok(layer) = tracing_android::layer("visionclaw-xr") {
                let _ = tracing_subscriber::registry().with(layer).try_init();
            }
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_target(true)
                .try_init();
        }
    });
}
