//! Cross-platform local-socket transport for the agent ↔ GUI tarpc IPC.
//!
//! `interprocess` exposes one API over a Unix-domain socket on Unix and a named
//! pipe on Windows, so the agent (server) and GUI (client) share a single code
//! path. The agent [`bind`]s a listener; the GUI [`connect`]s a stream; each
//! connection is wrapped in a tarpc `serde_transport` `Transport` with the same
//! length-delimited + [`BincodeNext`] framing on both ends (see [`wrap`]).
//!
//! This replaces the previous Unix-domain-socket-only `tarpc::serde_transport::unix`
//! path, which did not compile on Windows.

use std::io;
use std::pin::Pin;

use bytes::{Bytes, BytesMut};
use interprocess::local_socket::ListenerOptions;
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{Listener, Stream};
use serde::{Serialize, de::DeserializeOwned};
use tarpc::serde_transport::Transport;
use tarpc::tokio_serde::{Deserializer, Serializer};

/// Resolve the IPC endpoint name.
///
/// On Unix this is the filesystem Unix-domain socket at
/// [`agent_socket_path`](openhid_core::paths::agent_socket_path). Production
/// builds keep `~/.config/openhid/agent.sock`; local macOS `-dev` bundles use
/// the sibling `openhid-dev` profile so development agents cannot occupy the
/// installed app's endpoint. On Windows it is a named pipe in the OS namespace
/// (`\\.\pipe\openhid-agent.sock`).
///
/// # Errors
///
/// Fails if the home directory can't be resolved (Unix) or the platform rejects
/// the name.
pub fn endpoint_name() -> io::Result<interprocess::local_socket::Name<'static>> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        openhid_core::paths::agent_socket_path()
            .map_err(|e| io::Error::other(e.to_string()))?
            .to_fs_name::<GenericFilePath>()
    }
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        // A fixed per-machine pipe name. The default DACL grants the creating
        // user + administrators, which is acceptable for a single-user desktop;
        // per-user isolation on a shared machine is a future hardening point.
        "openhid-agent.sock".to_ns_name::<GenericNamespaced>()
    }
}

/// Bind the agent's IPC listener.
///
/// `try_overwrite(true)` unlinks a stale Unix-domain socket left by a *non-clean*
/// exit (SIGKILL / panic=abort / power loss, where the listener's `Drop` never
/// ran) before binding — otherwise `bind` fails with `AddrInUse` on the leftover
/// file, since the OS does not remove a socket inode on process death, and the
/// agent's IPC would stay dead across every relaunch. `main` holds the
/// single-instance lock, so no *live* agent owns the socket. No-op for Windows
/// named pipes (no filesystem entry to reclaim).
///
/// # Errors
///
/// Fails if the endpoint name can't be resolved, the socket directory can't be
/// created (Unix), or the listener can't be created.
pub fn bind() -> io::Result<Listener> {
    // On Unix the socket is a filesystem path; ensure its directory exists.
    #[cfg(unix)]
    if let Some(parent) = openhid_core::paths::agent_socket_path()
        .map_err(|e| io::Error::other(e.to_string()))?
        .parent()
    {
        std::fs::create_dir_all(parent)?;
    }
    ListenerOptions::new()
        .name(endpoint_name()?)
        .try_overwrite(true)
        .create_tokio()
}

/// Connect a client stream to the agent's IPC endpoint.
///
/// # Errors
///
/// Fails if the endpoint name can't be resolved or the agent isn't listening.
pub async fn connect() -> io::Result<Stream> {
    Stream::connect(endpoint_name()?).await
}

/// Wrap a connected local-socket [`Stream`] in a tarpc transport with the
/// length-delimited + bincode framing both sides expect. The `Item`/`SinkItem`
/// types are inferred by the caller (`BaseChannel` on the server, `*Client::new`
/// on the client).
#[must_use]
pub fn wrap<Item, SinkItem>(stream: Stream) -> Transport<Stream, Item, SinkItem, BincodeNext>
where
    Item: DeserializeOwned,
    SinkItem: Serialize,
{
    Transport::from((stream, BincodeNext))
}

/// The IPC wire codec.
///
/// `bincode-next` is a maintained fork of bincode. Its serde `standard()`
/// encoding is byte-identical to bincode 1.3 `DefaultOptions` — the format
/// tokio-serde's `Bincode` produced before this crate took ownership of the
/// codec: varint integers, little-endian, one value per frame. The goldens in
/// `tests/wire_format.rs` pin those bytes; they must stay green for a codec
/// swap, or the wire changed and `PROTOCOL_VERSION` must move.
///
/// Decoding rejects trailing bytes, matching bincode 1.3's default. A frame
/// carries exactly one value; anything left over is corruption, not padding.
#[derive(Clone, Copy, Debug, Default)]
pub struct BincodeNext;

/// Encode one IPC frame exactly as the transport does.
///
/// # Errors
///
/// Fails if the value cannot be serialized.
pub fn encode_frame<T>(value: &T) -> io::Result<Vec<u8>>
where
    T: Serialize + ?Sized,
{
    bincode_next::serde::encode_to_vec(value, bincode_next::config::standard())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Decode exactly one IPC frame, rejecting trailing bytes.
///
/// # Errors
///
/// Fails if the frame is malformed or holds bytes after the encoded value.
pub fn decode_frame<T>(frame: &[u8]) -> io::Result<T>
where
    T: DeserializeOwned,
{
    let (value, consumed) =
        bincode_next::serde::decode_from_slice(frame, bincode_next::config::standard())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if consumed != frame.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "trailing bytes after frame: consumed {consumed} of {}",
                frame.len()
            ),
        ));
    }
    Ok(value)
}

impl<SinkItem> Serializer<SinkItem> for BincodeNext
where
    SinkItem: Serialize,
{
    type Error = io::Error;

    fn serialize(self: Pin<&mut Self>, item: &SinkItem) -> io::Result<Bytes> {
        encode_frame(item).map(Bytes::from)
    }
}

impl<Item> Deserializer<Item> for BincodeNext
where
    Item: DeserializeOwned,
{
    type Error = io::Error;

    fn deserialize(self: Pin<&mut Self>, src: &BytesMut) -> io::Result<Item> {
        decode_frame(&src[..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_round_trips_and_rejects_trailing_bytes() {
        let mut codec = BincodeNext;
        let value = (7_u32, "x".to_owned(), Some(-3_i64));
        let encoded = Pin::new(&mut codec)
            .serialize(&value)
            .expect("frame encodes");
        let mut frame = BytesMut::from(&encoded[..]);
        let decoded: (u32, String, Option<i64>) = Pin::new(&mut codec)
            .deserialize(&frame)
            .expect("frame decodes");
        assert_eq!(decoded, value);

        frame.extend_from_slice(&[0]);
        assert!(
            decode_frame::<(u32, String, Option<i64>)>(&frame).is_err(),
            "a trailing byte must be refused"
        );
    }
}
