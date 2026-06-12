use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use visionclaw_xr_gdext::binary_protocol::{decode_position_frame, NODE_RECORD_BYTES, PROTOCOL_V3};
use visionclaw_xr_presence::types::{PoseFrame, Transform};
use visionclaw_xr_presence::{decode, encode, AvatarId, Did, RoomId};

const FULL_GRAPH_NODE_COUNT: usize = 1000;

/// Build a V3 graph position frame: version byte + N×52-byte node records
/// (id, pos[3], vel[3], then sssp_dist/sssp_parent/cluster_id/anomaly/community/centrality).
fn build_position_frame(node_count: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + node_count * NODE_RECORD_BYTES);
    out.push(PROTOCOL_V3);
    for i in 0..node_count {
        let id = (i as u32) + 1;
        out.extend_from_slice(&id.to_le_bytes());
        let phase = (i as f32) * 0.01;
        let pos = [phase.sin() * 10.0, phase.cos() * 5.0, phase * 0.1];
        let vel = [0.001 * phase, -0.001 * phase, 0.0];
        for v in pos {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in vel {
            out.extend_from_slice(&v.to_le_bytes());
        }
        // Analytics tail (24 bytes): sssp_dist, sssp_parent, cluster_id, anomaly, community, centrality.
        out.extend_from_slice(&f32::INFINITY.to_le_bytes());
        out.extend_from_slice(&(-1i32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0f32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0f32.to_le_bytes());
    }
    out
}

fn presence_fixture() -> (RoomId, AvatarId, PoseFrame) {
    let room = RoomId::parse("urn:visionclaw:room:sha256-12-deadbeefcafe").unwrap();
    let did = Did::parse(format!("did:nostr:{}", "b".repeat(64))).unwrap();
    let avatar = AvatarId::from_did(&did);
    let t = Transform {
        position: [0.0, 1.7, -1.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
    };
    let frame = PoseFrame {
        timestamp_us: 1_700_000_000_000_000,
        head: t,
        left_hand: Some(t),
        right_hand: Some(t),
    };
    (room, avatar, frame)
}

fn bench_decode_full_graph(c: &mut Criterion) {
    let frame = build_position_frame(FULL_GRAPH_NODE_COUNT);
    let mut group = c.benchmark_group("decode_position_frame_1k");
    group.throughput(Throughput::Bytes(frame.len() as u64));
    group.bench_function("decode_1000_nodes", |b| {
        b.iter(|| decode_position_frame(black_box(&frame)).unwrap())
    });
    group.finish();
}

fn bench_decode_per_node_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_position_frame_scaling");
    for &n in &[100usize, 500, 1000, 5000] {
        let frame = build_position_frame(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_function(format!("nodes_{}", n), |b| {
            b.iter(|| decode_position_frame(black_box(&frame)).unwrap())
        });
    }
    group.finish();
}

fn bench_presence_round_trip(c: &mut Criterion) {
    let (room, avatar, frame) = presence_fixture();
    c.bench_function("presence_0x43_round_trip", |b| {
        b.iter(|| {
            let bytes = encode(black_box(&frame), black_box(&room), black_box(&avatar)).unwrap();
            let decoded = decode(black_box(&bytes)).unwrap();
            black_box(decoded);
        })
    });
}

criterion_group!(
    benches,
    bench_decode_full_graph,
    bench_decode_per_node_scaling,
    bench_presence_round_trip
);
criterion_main!(benches);
