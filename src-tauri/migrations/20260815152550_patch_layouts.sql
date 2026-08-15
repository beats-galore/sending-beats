-- Where the user has put the things on the patchbay, and how big they made them.
--
-- Every position and size on the canvas is otherwise computed: the interface
-- stacks sources down the left, buses down the middle and destinations down the
-- right, and each node's height follows from what it is showing. That reads well
-- until a patch outgrows the three columns, at which point there is no way to
-- arrange it by hand.
--
-- A row here is an override on that computed layout, not a replacement for it. A
-- node with no row sits where the stack puts it, which is what makes "tidy"
-- simply mean "delete these rows". Each column is nullable for the same reason:
-- a node that was only dragged has no size of its own and keeps taking its
-- height from what it is showing, and a node that was only resized stays in the
-- column it was stacked into.
--
-- Keyed the same way patch_colors is, and for the same reason — the things being
-- placed share no common row to hang columns off:
--
--   ch:<channel number>       an input strip
--   bus:<bus id>              a mix
--   out:<device identifier>   a hardware destination
--   stream / rec              the broadcast and the tape
--
-- Sizes and positions are in the canvas's own logical coordinates, which the
-- interface authors against a fixed width and scales to fit. Storing screen
-- pixels instead would move every node the first time the window was resized.

CREATE TABLE patch_layouts (
    id VARCHAR(36) PRIMARY KEY,
    configuration_id VARCHAR(36) NOT NULL,

    -- What is being placed, in the patchbay's own key vocabulary
    target_key TEXT NOT NULL,

    -- Canvas coordinates of the node's top left corner. NULL means it has not
    -- been moved and still follows the column it belongs to.
    x REAL,
    y REAL,

    -- NULL means the node has not been resized and still takes the size its
    -- contents ask for.
    width REAL,
    height REAL,

    -- Required timestamp columns
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    FOREIGN KEY (configuration_id) REFERENCES audio_mixer_configurations(id)
);

CREATE INDEX idx_patch_layouts_configuration_id ON patch_layouts(configuration_id);
CREATE INDEX idx_patch_layouts_created ON patch_layouts(created_at);

-- One placement per thing within a configuration
CREATE UNIQUE INDEX idx_patch_layouts_config_target
    ON patch_layouts(configuration_id, target_key);
