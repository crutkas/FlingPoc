use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{AirPlayError, Result};

const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(25);

/// An HTTP-like request using either RTSP/1.0 or HTTP/1.1 framing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RtspRequest {
    pub method: String,
    pub uri: String,
    pub protocol: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl RtspRequest {
    #[must_use]
    pub fn new(method: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            uri: uri.into(),
            protocol: "RTSP/1.0".to_string(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_token("method", &self.method)?;
        validate_token("URI", &self.uri)?;
        validate_token("protocol", &self.protocol)?;

        let mut output = format!("{} {} {}\r\n", self.method, self.uri, self.protocol);
        for (name, value) in &self.headers {
            validate_header(name, value)?;
            output.push_str(name);
            output.push_str(": ");
            output.push_str(value);
            output.push_str("\r\n");
        }
        if !self.body.is_empty()
            && !self
                .headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("content-length"))
        {
            output.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        output.push_str("\r\n");

        let mut bytes = output.into_bytes();
        bytes.extend_from_slice(&self.body);
        Ok(bytes)
    }
}

/// Parsed response from an AirPlay HTTP/RTSP endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RtspResponse {
    pub protocol: String,
    pub status: u16,
    pub reason: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl RtspResponse {
    /// Parse one response and return it with the number of consumed bytes.
    pub fn decode(bytes: &[u8]) -> Result<Option<(Self, usize)>> {
        let Some(header_offset) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            if bytes.len() > MAX_HEADER_BYTES {
                return Err(AirPlayError::Protocol(
                    "response headers exceed 64 KiB".to_string(),
                ));
            }
            return Ok(None);
        };
        if header_offset > MAX_HEADER_BYTES {
            return Err(AirPlayError::Protocol(
                "response headers exceed 64 KiB".to_string(),
            ));
        }

        let header_end = header_offset + 4;
        let header = std::str::from_utf8(&bytes[..header_offset])
            .map_err(|_| AirPlayError::Protocol("response headers are not UTF-8".to_string()))?;
        let mut lines = header.split("\r\n");
        let status_line = lines
            .next()
            .ok_or_else(|| AirPlayError::Protocol("response has no status line".to_string()))?;
        let mut status_parts = status_line.splitn(3, ' ');
        let protocol = status_parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AirPlayError::Protocol("response protocol is missing".to_string()))?
            .to_string();
        let status = status_parts
            .next()
            .ok_or_else(|| AirPlayError::Protocol("response status is missing".to_string()))?
            .parse()
            .map_err(|_| AirPlayError::Protocol("response status is invalid".to_string()))?;
        let reason = status_parts.next().unwrap_or_default().to_string();

        let mut headers = BTreeMap::new();
        for line in lines {
            let (name, value) = line.split_once(':').ok_or_else(|| {
                AirPlayError::Protocol("response contains an invalid header".to_string())
            })?;
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
        let body_length = headers.get("content-length").map_or(Ok(0), |value| {
            value
                .parse::<usize>()
                .map_err(|_| AirPlayError::Protocol("Content-Length is invalid".to_string()))
        })?;
        if body_length > MAX_BODY_BYTES {
            return Err(AirPlayError::Protocol(
                "response body exceeds 8 MiB".to_string(),
            ));
        }
        let total_length = header_end
            .checked_add(body_length)
            .ok_or_else(|| AirPlayError::Protocol("response length overflowed".to_string()))?;
        if bytes.len() < total_length {
            return Ok(None);
        }

        Ok(Some((
            Self {
                protocol,
                status,
                reason,
                headers,
                body: bytes[header_end..total_length].to_vec(),
            },
            total_length,
        )))
    }
}

/// Persistent plain TCP transport for AirPlay HTTP and RTSP exchanges.
pub struct RtspConnection {
    stream: TcpStream,
    pending: Vec<u8>,
    next_cseq: u32,
    timeout: Duration,
}

impl RtspConnection {
    pub async fn connect(address: SocketAddr) -> Result<Self> {
        let stream = tokio::time::timeout(DEFAULT_TIMEOUT, TcpStream::connect(address))
            .await
            .map_err(|_| AirPlayError::Protocol("connection timed out".to_string()))??;
        Ok(Self {
            stream,
            pending: Vec::new(),
            next_cseq: 0,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub async fn exchange(&mut self, mut request: RtspRequest) -> Result<RtspResponse> {
        if request.protocol.eq_ignore_ascii_case("RTSP/1.0") {
            request
                .headers
                .entry("CSeq".to_string())
                .or_insert_with(|| self.next_cseq.to_string());
            self.next_cseq = self.next_cseq.wrapping_add(1);
        }
        request
            .headers
            .entry("User-Agent".to_string())
            .or_insert_with(|| "AirPlay/550.10".to_string());

        let encoded = request.encode()?;
        tokio::time::timeout(self.timeout, self.stream.write_all(&encoded))
            .await
            .map_err(|_| AirPlayError::Protocol("request timed out".to_string()))??;

        loop {
            if let Some((response, consumed)) = RtspResponse::decode(&self.pending)? {
                self.pending.drain(..consumed);
                return Ok(response);
            }

            let mut buffer = [0_u8; 8192];
            let read = tokio::time::timeout(self.timeout, self.stream.read(&mut buffer))
                .await
                .map_err(|_| AirPlayError::Protocol("response timed out".to_string()))??;
            if read == 0 {
                return Err(AirPlayError::Protocol(
                    "connection closed before a complete response".to_string(),
                ));
            }
            self.pending.extend_from_slice(&buffer[..read]);
            if self.pending.len() > MAX_HEADER_BYTES + MAX_BODY_BYTES {
                return Err(AirPlayError::Protocol(
                    "response exceeds the configured limit".to_string(),
                ));
            }
        }
    }
}

pub fn encode_binary_plist<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    plist::to_writer_binary(&mut encoded, value)?;
    Ok(encoded)
}

pub fn decode_plist<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    Ok(plist::from_bytes(bytes)?)
}

fn validate_token(kind: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(AirPlayError::Protocol(format!(
            "{kind} contains invalid characters"
        )));
    }
    Ok(())
}

fn validate_header(name: &str, value: &str) -> Result<()> {
    validate_token("header name", name)?;
    if name.contains(':') || value.contains(['\r', '\n']) {
        return Err(AirPlayError::Protocol(
            "header contains invalid characters".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "PascalCase")]
    struct PlayBody {
        content_location: String,
    }

    #[test]
    fn request_encodes_content_length_and_rejects_header_injection() {
        let mut request = RtspRequest::new("POST", "/play");
        request.body = b"body".to_vec();
        let encoded = request.encode().expect("request is valid");
        assert!(encoded.starts_with(b"POST /play RTSP/1.0\r\n"));
        assert!(
            encoded
                .windows(b"Content-Length: 4\r\n".len())
                .any(|window| window == b"Content-Length: 4\r\n")
        );

        request
            .headers
            .insert("X-Test".to_string(), "ok\r\nInjected: yes".to_string());
        assert!(request.encode().is_err());
    }

    #[test]
    fn response_parser_waits_for_the_complete_body() {
        let partial = b"RTSP/1.0 200 OK\r\nContent-Length: 4\r\n\r\nab";
        assert!(
            RtspResponse::decode(partial)
                .expect("partial response is valid")
                .is_none()
        );

        let complete = b"RTSP/1.0 200 OK\r\nContent-Length: 4\r\n\r\nbodytrailing";
        let (response, consumed) = RtspResponse::decode(complete)
            .expect("response is valid")
            .expect("response is complete");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"body");
        assert_eq!(&complete[consumed..], b"trailing");
    }

    #[test]
    fn binary_plist_round_trips() {
        let body = PlayBody {
            content_location: "https://example.test/video.mp4".to_string(),
        };
        let encoded = encode_binary_plist(&body).expect("plist encodes");
        assert!(encoded.starts_with(b"bplist"));
        assert_eq!(
            decode_plist::<PlayBody>(&encoded).expect("plist decodes"),
            body
        );
    }
}
