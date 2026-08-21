extends "res://addons/gut/test.gd"

# M1 identity-badge tests on the human-peer avatar (PRD-023 WP-9). The DID the
# presence join carries (graph_scene.gd:431 set_meta) must render, with an
# unverified claim held visually distinct — the invariant-2 counter to a
# self-reported nameplate.

const COLOR_VERIFIED := Color(0.75, 1.0, 0.82)
const COLOR_UNVERIFIED := Color(1.0, 0.72, 0.28)


func _make_avatar() -> Node3D:
	var packed: PackedScene = load("res://scenes/Avatar.tscn")
	var avatar: Node3D = packed.instantiate()
	add_child(avatar)
	await get_tree().process_frame
	return avatar


func test_set_identity_populates_nameplate_and_did_badge():
	var avatar: Node3D = await _make_avatar()
	var did := "did:nostr:" + "c".repeat(56) + "0badf00d"
	avatar.set_avatar_identity("Alice", did, true)
	var nameplate: Label3D = avatar.get_node("Head/Nameplate")
	var badge: Label3D = avatar.get_node("Head/DidBadge")
	assert_eq(nameplate.text, "Alice", "nameplate carries the display name")
	assert_true(badge.text.contains("…0badf00d"), "DID badge shows the …last8 pubkey")
	assert_true(badge.text.contains("✓"), "verified badge carries a check mark")
	assert_eq(badge.modulate, COLOR_VERIFIED, "verified badge colour")
	avatar.queue_free()
	await get_tree().process_frame


func test_unverified_identity_is_distinct():
	var avatar: Node3D = await _make_avatar()
	avatar.set_avatar_identity("Mallory", "did:nostr:" + "d".repeat(64), false)
	var badge: Label3D = avatar.get_node("Head/DidBadge")
	assert_true(badge.text.contains("?"), "unverified badge carries a query mark")
	assert_eq(badge.modulate, COLOR_UNVERIFIED, "unverified badge is amber")
	avatar.queue_free()
	await get_tree().process_frame


func test_set_verified_flips_badge_state():
	var avatar: Node3D = await _make_avatar()
	avatar.set_avatar_identity("Bob", "did:nostr:" + "e".repeat(64), false)
	var badge: Label3D = avatar.get_node("Head/DidBadge")
	assert_eq(badge.modulate, COLOR_UNVERIFIED, "starts unverified")
	avatar.set_verified(true)
	assert_eq(badge.modulate, COLOR_VERIFIED, "flips to verified once the Schnorr check passes")
	assert_true(badge.text.contains("✓"), "mark updates on verification")
	avatar.queue_free()
	await get_tree().process_frame


func test_did_badge_drops_at_medium_lod():
	var avatar: Node3D = await _make_avatar()
	avatar.set_avatar_identity("Carol", "did:nostr:" + "f".repeat(64), true)
	avatar.set_lod_level(0)
	await get_tree().process_frame
	assert_true(avatar.get_node("Head/DidBadge").visible, "DID badge visible at High")
	avatar.set_lod_level(1)
	await get_tree().process_frame
	assert_false(avatar.get_node("Head/DidBadge").visible, "DID badge drops with the nameplate at Medium")
	avatar.queue_free()
	await get_tree().process_frame


func test_short_did_static_helper():
	var avatar: Node3D = await _make_avatar()
	assert_eq(avatar._short_did(""), "unverified")
	assert_eq(avatar._short_did("did:nostr:aabbccddeeff0011"), "…eeff0011")
	avatar.queue_free()
	await get_tree().process_frame
