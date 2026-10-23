//! Native duplex sessions, with platform-owned cancellation and handle lifetimes.

#[cfg(not(target_os = "windows"))]
#[path = "raw_session/portable.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "raw_session/windows.rs"]
mod platform;

use std::time::Duration;

use async_hid::Device;
use hidpp::async_trait;
use openhid_device::backend::{BackendError, RawSession, RawWriter};
use openhid_device::{DeviceIoGate, host_lock};

/// Serializes every duplex transaction across inventory, UI jobs and CLI
/// processes. Fail closed if arbitration is unavailable: this protocol has no
/// transaction ID and cannot distinguish concurrent users' replies.
pub(super) async fn open(
    device: &Device,
    gate: DeviceIoGate,
) -> Result<Box<dyn RawSession>, BackendError> {
    gate.ensure_allowed()?;
    let name = format!(
        "{}-raw-session",
        host_lock::node_lock_name(&super::node_id(device))
    );
    let lock = host_lock::lock_within(&name, Duration::from_secs(2))
        .await
        .map_err(|error| BackendError::Backend(format!("raw-session lock unavailable: {error}")))?
        .ok_or_else(|| BackendError::Backend("raw-session device is busy".into()))?;
    #[cfg(target_os = "windows")]
    let inner = platform::open(device, gate)?;
    #[cfg(not(target_os = "windows"))]
    let inner = platform::open(device, gate).await?;
    Ok(Box::new(ExclusiveSession { inner, _lock: lock }))
}

struct ExclusiveSession {
    // Field drop order matters: cancel/drain native I/O before unlocking.
    inner: Box<dyn RawSession>,
    _lock: host_lock::HostLock,
}

#[async_trait]
impl RawWriter for ExclusiveSession {
    async fn write_output_report(&mut self, report: &[u8]) -> Result<(), BackendError> {
        self.inner.write_output_report(report).await
    }
}

#[async_trait]
impl RawSession for ExclusiveSession {
    async fn read_input_report(&mut self, buffer: &mut [u8]) -> Result<usize, BackendError> {
        self.inner.read_input_report(buffer).await
    }

    async fn exchange_output_report(
        &mut self,
        report: &[u8],
        input: &mut [u8],
    ) -> Result<usize, BackendError> {
        self.inner.exchange_output_report(report, input).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    struct Inner {
        name: String,
        dropped: Arc<AtomicUsize>,
        writes: Arc<AtomicUsize>,
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            assert!(
                host_lock::try_lock(&self.name).unwrap().is_none(),
                "native cleanup must finish while arbitration is still held"
            );
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl RawWriter for Inner {
        async fn write_output_report(&mut self, _: &[u8]) -> Result<(), BackendError> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl RawSession for Inner {
        async fn read_input_report(&mut self, _: &mut [u8]) -> Result<usize, BackendError> {
            std::future::pending().await
        }
    }

    #[tokio::test]
    async fn exclusive_session_retains_arbitration_through_native_cleanup() {
        let name = format!("test-{}-raw-session-cleanup", std::process::id());
        let dropped = Arc::new(AtomicUsize::new(0));
        let writes = Arc::new(AtomicUsize::new(0));
        let mut session = ExclusiveSession {
            inner: Box::new(Inner {
                name: name.clone(),
                dropped: dropped.clone(),
                writes: writes.clone(),
            }),
            _lock: host_lock::try_lock(&name).unwrap().unwrap(),
        };
        session.write_output_report(&[0x13]).await.unwrap();
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        {
            let mut buffer = [0; 20];
            let pending = session.read_input_report(&mut buffer);
            tokio::pin!(pending);
            tokio::select! {
                biased;
                result = &mut pending => panic!("reader completed: {result:?}"),
                () = std::future::ready(()) => {},
            }
        }
        assert_eq!(dropped.load(Ordering::SeqCst), 0);
        assert!(host_lock::try_lock(&name).unwrap().is_none());
        drop(session);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert!(host_lock::try_lock(&name).unwrap().is_some());
    }
}
