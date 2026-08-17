// Putting one segment on the wire, as its own finite request.
//
// The shape here is the whole point of the protocol and is not negotiable: one
// bounded request per segment, `Content-Length` set, connection closed
// afterwards. A request that completes runs the worker immediately; a request
// held open is buffered at the edge and runs the worker only once it ends, which
// produces a broadcast that materialises after it is over.
//
// So nothing in this file streams, and nothing in it holds a connection.

use anyhow::{Context, Result};
use colored::*;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use super::segmenter::Segment;

/// A segment upload that hangs is worse than one that fails: the next segment is
/// already waiting behind it, and four seconds of audio arrives every four
/// seconds whether or not the last one landed.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How many times one segment is worth retrying
///
/// Bounded on purpose. A failure should cost one segment, not the broadcast, and
/// a segment retried past its own duration is arriving too late to be in the
/// window anyway.
const MAX_ATTEMPTS: u32 = 3;

const RETRY_DELAY: Duration = Duration::from_millis(400);

/// What the other end says about the station after a segment lands
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveStatus {
    pub station_slug: String,
    pub on_air: bool,
    /// The ordinal of the oldest segment still in the listener's window
    pub media_sequence: u64,
    pub segment_seconds: u32,
}

/// Now-playing carried alongside a segment
///
/// Two fields rather than one free-text string, which is the thing Icecast
/// cannot express: its convention is `"Artist - Title"` and sources honour it
/// inconsistently, so the receiver is left splitting a string that may or may
/// not have been joined the way it expects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    /// True on the first segment after a gap
    ///
    /// Flagged here because this is the only moment it is knowable. Whether the
    /// audio was interrupted cannot be recovered later from timestamps.
    pub discontinuity: bool,
}

/// Where a station's segments go, and what proves we may send them
#[derive(Debug, Clone)]
pub struct ImpulseUploader {
    client: reqwest::Client,
    /// Origin with no trailing slash
    endpoint: String,
    slug: String,
    token: String,
}

impl ImpulseUploader {
    pub fn new(endpoint_url: &str, slug: &str, token: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("Could not build the HTTP client for the ingest connection")?;

        Ok(Self {
            client,
            endpoint: endpoint_url.trim_end_matches('/').to_string(),
            slug: slug.trim_matches('/').to_string(),
            token: token.to_string(),
        })
    }

    fn segment_url(&self, segment: &Segment, metadata: &SegmentMetadata) -> String {
        let mut url = format!(
            "{}/ingest/{}/segment?durationMs={}&extension=mp3",
            self.endpoint, self.slug, segment.duration_ms
        );

        if metadata.discontinuity {
            url.push_str("&discontinuity=true");
        }

        // The metadata rides in the query string because the body is the audio,
        // and reading inside the body to find anything would mean buffering it.
        if let Some(title) = metadata.title.as_deref().filter(|t| !t.is_empty()) {
            url.push_str(&format!("&title={}", urlencoding::encode(title)));
        }

        if let Some(artist) = metadata.artist.as_deref().filter(|a| !a.is_empty()) {
            url.push_str(&format!("&artist={}", urlencoding::encode(artist)));
        }

        url
    }

    /// Send one segment, retrying only what is worth retrying
    ///
    /// A rejection is final — a bad token or a station that does not exist will
    /// be just as bad in four hundred milliseconds, and retrying it only delays
    /// telling somebody.
    pub async fn put_segment(
        &self,
        segment: &Segment,
        metadata: &SegmentMetadata,
    ) -> Result<LiveStatus> {
        let url = self.segment_url(segment, metadata);
        let mut last_error = None;

        for attempt in 1..=MAX_ATTEMPTS {
            match self.attempt(&url, segment).await {
                Ok(status) => return Ok(status),
                Err(SendFailure::Rejected(error)) => return Err(error),
                Err(SendFailure::Retryable(error)) => {
                    warn!(
                        "⚠️ {}: Segment attempt {}/{} failed: {}",
                        "IMPULSE_SEND".on_purple().white(),
                        attempt,
                        MAX_ATTEMPTS,
                        error
                    );
                    last_error = Some(error);

                    if attempt < MAX_ATTEMPTS {
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("The segment could not be sent")))
    }

    async fn attempt(&self, url: &str, segment: &Segment) -> Result<LiveStatus, SendFailure> {
        let response = self
            .client
            .put(url)
            .bearer_auth(&self.token)
            .header("content-type", "audio/mpeg")
            // Set explicitly from an owned body, because a request whose length
            // is unknown is one the far end cannot store without buffering.
            .body(segment.body.clone())
            .send()
            .await
            .map_err(|e| SendFailure::Retryable(anyhow::anyhow!("{}", e)))?;

        let status = response.status();

        if status.is_success() {
            return response
                .json::<LiveStatus>()
                .await
                .map_err(|e| SendFailure::Retryable(anyhow::anyhow!("Unreadable reply: {}", e)));
        }

        let body = response.text().await.unwrap_or_default();

        // 5xx is the far end having a bad moment; 4xx is us being wrong about
        // something, and sending it again will not make us right.
        if status.is_server_error() {
            return Err(SendFailure::Retryable(anyhow::anyhow!(
                "The ingest worker returned {}: {}",
                status,
                body.trim()
            )));
        }

        Err(SendFailure::Rejected(anyhow::anyhow!(
            "The ingest worker refused the segment ({}): {}",
            status,
            body.trim()
        )))
    }

    /// Say the broadcast has ended, rather than leaving it to be noticed
    ///
    /// Without this the station stays on air until the dead-air alarm fires
    /// several segments later, so listeners hear the end of the show followed by
    /// a stall instead of a sign-off.
    pub async fn go_off_air(&self) -> Result<()> {
        let url = format!("{}/ingest/{}/control", self.endpoint, self.slug);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .json(&serde_json::json!({ "action": "off-air" }))
            .send()
            .await
            .context("Could not tell the ingest worker the broadcast ended")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Signing off was refused: {}",
                response.status()
            ));
        }

        debug!("{}: Signed off", "IMPULSE_SEND".on_purple().white());
        Ok(())
    }
}

/// Whether a failure is worth another go
enum SendFailure {
    Retryable(anyhow::Error),
    Rejected(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use super::{ImpulseUploader, SegmentMetadata};
    use crate::audio::broadcasting::impulse::segmenter::Segment;

    fn segment() -> Segment {
        Segment {
            body: vec![0xFF, 0xFB],
            duration_ms: 4_023,
            elapsed_ms: 0,
        }
    }

    fn uploader() -> ImpulseUploader {
        ImpulseUploader::new("https://impulse.example.com/", "shady", "token").unwrap()
    }

    #[test]
    fn the_url_carries_the_measured_duration() {
        let url = uploader().segment_url(&segment(), &SegmentMetadata::default());

        assert_eq!(
            url,
            "https://impulse.example.com/ingest/shady/segment?durationMs=4023&extension=mp3"
        );
    }

    #[test]
    fn a_trailing_slash_on_the_endpoint_does_not_double_up() {
        let url = ImpulseUploader::new("https://impulse.example.com///", "shady", "t")
            .unwrap()
            .segment_url(&segment(), &SegmentMetadata::default());

        assert!(url.starts_with("https://impulse.example.com/ingest/shady/segment"));
    }

    /// A mount point typed the Icecast way is the mistake to expect, and a slug
    /// with a slash in it addresses a different station
    #[test]
    fn a_slug_typed_with_slashes_is_cleaned_up() {
        let url = ImpulseUploader::new("https://impulse.example.com", "/shady/", "t")
            .unwrap()
            .segment_url(&segment(), &SegmentMetadata::default());

        assert!(url.contains("/ingest/shady/segment"));
    }

    #[test]
    fn metadata_rides_in_the_query_string() {
        let url = uploader().segment_url(
            &segment(),
            &SegmentMetadata {
                title: Some("Blue Monday".to_string()),
                artist: Some("New Order".to_string()),
                discontinuity: true,
            },
        );

        assert!(url.contains("&discontinuity=true"));
        assert!(url.contains("&title=Blue%20Monday"));
        assert!(url.contains("&artist=New%20Order"));
    }

    /// Track titles contain ampersands and question marks, and one of those
    /// unescaped turns the rest of the title into somebody else's parameter
    #[test]
    fn awkward_titles_are_escaped() {
        let url = uploader().segment_url(
            &segment(),
            &SegmentMetadata {
                title: Some("Q&A?".to_string()),
                artist: None,
                discontinuity: false,
            },
        );

        assert!(url.contains("&title=Q%26A%3F"), "{}", url);
        assert!(!url.contains("artist="), "an absent artist is not sent");
    }

    #[test]
    fn empty_metadata_is_the_same_as_none() {
        let url = uploader().segment_url(
            &segment(),
            &SegmentMetadata {
                title: Some(String::new()),
                artist: Some(String::new()),
                discontinuity: false,
            },
        );

        assert!(!url.contains("title="));
        assert!(!url.contains("artist="));
    }
}
