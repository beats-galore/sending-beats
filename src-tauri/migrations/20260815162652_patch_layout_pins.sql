-- Nodes pinned to the edge of another node, so they move as one.
--
-- Free placement lets a patch be arranged into groups that belong together — a
-- source beside the mix it feeds, a stack of related strips — but nothing holds
-- such a group together, so moving one member breaks it apart. A pin is that
-- relationship: the pinned node sits flush against an edge of its anchor and
-- takes its position from it, which is what makes dragging the anchor carry the
-- whole group without moving anything else.
--
-- Position is derived rather than stored while a node is pinned, so x and y are
-- left null and written back on unpin, where the node stays exactly where it
-- was last drawn rather than jumping to wherever it was last dropped.
--
-- pinned_to holds the anchor's target key, in the same vocabulary the rest of
-- this table is keyed by. Not a foreign key: the things being pinned are
-- channels, buses, devices and the broadcast, which share no common row — the
-- same reason this table is keyed by a string at all.
--
-- pin_edge is which edge of the anchor the node sits against — bottom, left or
-- right. Text rather than a constraint, and validated by the interface, which
-- also decides what an edge it no longer understands should fall back to.
--
-- Top is deliberately absent. Pinning A above B and pinning B below A are the
-- same arrangement, and storing both ways to say it means two code paths that
-- have to agree.

ALTER TABLE patch_layouts ADD COLUMN pinned_to TEXT;
ALTER TABLE patch_layouts ADD COLUMN pin_edge TEXT;

-- Finding a node's followers, for moving a group and for releasing them when
-- their anchor is deleted
CREATE INDEX idx_patch_layouts_pinned_to ON patch_layouts(configuration_id, pinned_to);
