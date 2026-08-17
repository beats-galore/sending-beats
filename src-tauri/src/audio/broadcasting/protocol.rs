// Which transmitter puts a station on air.

use anyhow::Result;

/// The two shapes a broadcast can take
///
/// Not two settings on one mechanism. Icecast opens a socket and writes into it
/// for the length of the show; Impulse holds nothing open at all and sends each
/// few seconds of audio as its own finite request. The second exists because the
/// first cannot reach the edge: Cloudflare buffers a streaming request body and
/// only runs the worker once the request has completed, so a show sent down one
/// long connection does not exist until after it has ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CastProtocol {
    Icecast,
    Impulse,
}

impl CastProtocol {
    /// Read a stored protocol
    ///
    /// An unrecognised value is an error rather than a fallback to Icecast: a
    /// station quietly transmitted by the wrong protocol would connect, appear
    /// to work, and reach nobody.
    pub fn from_stored(stored: &str) -> Result<Self> {
        match stored {
            "icecast" => Ok(Self::Icecast),
            "impulse" => Ok(Self::Impulse),
            other => Err(anyhow::anyhow!(
                "'{}' is not a cast protocol this build knows about",
                other
            )),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Icecast => "icecast",
            Self::Impulse => "impulse",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CastProtocol;

    #[test]
    fn stored_values_round_trip() {
        for protocol in [CastProtocol::Icecast, CastProtocol::Impulse] {
            assert_eq!(
                CastProtocol::from_stored(protocol.as_str()).unwrap(),
                protocol
            );
        }
    }

    /// A row written by a newer build should refuse to go on air rather than
    /// silently going out over the wrong transmitter
    #[test]
    fn an_unknown_protocol_is_refused() {
        assert!(CastProtocol::from_stored("srt").is_err());
    }
}
