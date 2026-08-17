-- Which kind of transmitter a station is, and what the new kind needs to know.
--
-- Icecast holds one socket open for the length of a show. Impulse is the
-- opposite shape: nothing is held open at all, and the mix is cut into bounded
-- segments that are each sent as their own finite request. That is not a style
-- preference — Cloudflare buffers a streaming request body and only invokes the
-- worker once the request completes, so a broadcast sent as one long connection
-- does not exist until after it has ended. Segmenting before the edge is the
-- entire reason there is a second protocol rather than a second server field.
--
-- Text rather than a database constraint, as with every other enum here.
ALTER TABLE cast_configurations ADD COLUMN protocol TEXT NOT NULL DEFAULT 'icecast';

-- The three columns below are Impulse's, and are null on an Icecast row. The
-- two protocols address a station in genuinely different terms — a host, a port
-- and a mount against an origin and a slug — and folding them into shared
-- columns would mean one of the two lying about what it holds.

-- Where Impulse answers, scheme included. It is https in front of Cloudflare and
-- http in front of a local `wrangler dev`, so a bare host could not say which.
ALTER TABLE cast_configurations ADD COLUMN endpoint_url TEXT;

-- Names the Durable Object instance on the other end, and is immutable once a
-- station has been on air: changing it points the broadcast at a fresh, empty
-- object rather than renaming anything.
ALTER TABLE cast_configurations ADD COLUMN station_slug TEXT;

-- The master latency knob. On the other end the same number sets the target
-- duration, the playlist cache TTL and the dead-air threshold, so it belongs to
-- a station rather than being a global.
ALTER TABLE cast_configurations ADD COLUMN segment_ms INTEGER NOT NULL DEFAULT 4000;
