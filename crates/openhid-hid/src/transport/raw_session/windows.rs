#![expect(
    unsafe_code,
    reason = "Win32 overlapped HID I/O requires stable buffers and FFI"
)]

use std::{io, ptr::null_mut, sync::Arc, time::Duration};

use async_hid::Device;
use hidpp::async_trait;
use openhid_device::{
    DeviceIoGate,
    backend::{BackendError, RawSession, RawWriter},
};
use windows_sys::Win32::{
    Foundation::{
        ERROR_DEVICE_NOT_CONNECTED, ERROR_INVALID_HANDLE, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING,
        ERROR_OPERATION_ABORTED,
    },
    Storage::FileSystem::{ReadFile, WriteFile},
    System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
};

use super::super::windows_hid::HidHandle;

// GetOverlappedResult is nonblocking; yield between checks so one sleeping
// device cannot occupy the agent runtime. Protocol deadlines remain upstream.
const COMPLETION_POLL: Duration = Duration::from_millis(2);

pub(crate) fn open(
    device: &Device,
    gate: DeviceIoGate,
) -> Result<Box<dyn RawSession>, BackendError> {
    gate.ensure_allowed()?;
    let handle = HidHandle::open_overlapped(device).map_err(|error| backend_error(&error))?;
    let caps = handle
        .caps()
        .map_err(|error| BackendError::Backend(error.to_string()))?;
    if caps.UsagePage != device.usage_page || caps.Usage != device.usage_id {
        return Err(BackendError::Backend(
            "opened HID collection does not match enumeration".into(),
        ));
    }
    if caps.InputReportByteLength == 0 || caps.OutputReportByteLength == 0 {
        return Err(BackendError::Backend(
            "collection does not support duplex reports".into(),
        ));
    }
    let session = Session {
        handle: Arc::new(handle),
        input_length: usize::from(caps.InputReportByteLength),
        output_length: usize::from(caps.OutputReportByteLength),
        gate,
    };
    session.gate.ensure_allowed()?;
    Ok(Box::new(session))
}

fn backend_error(error: &io::Error) -> BackendError {
    if error.raw_os_error().is_some_and(|code| {
        [
            ERROR_DEVICE_NOT_CONNECTED,
            ERROR_INVALID_HANDLE,
            ERROR_OPERATION_ABORTED,
        ]
        .iter()
        .any(|expected| i64::from(code) == i64::from(*expected))
    }) {
        BackendError::Disconnected
    } else {
        BackendError::Backend(error.to_string())
    }
}

struct Session {
    handle: Arc<HidHandle>,
    input_length: usize,
    output_length: usize,
    gate: DeviceIoGate,
}

/// Every OS pointer targets a stable allocation owned by this operation.
/// Drop cancels AND waits before freeing anything Windows might still access.
struct Pending {
    handle: Arc<HidHandle>,
    buffer: Box<[u8]>,
    overlapped: Box<OVERLAPPED>,
    submitted: bool,
}

// SAFETY: All OS-visible storage is heap allocated and remains at the same
// address when Pending moves. Its mutable borrow serializes access to storage.
unsafe impl Send for Pending {}

impl Pending {
    fn new(handle: Arc<HidHandle>, buffer: Box<[u8]>) -> Self {
        Self {
            handle,
            buffer,
            overlapped: Box::default(),
            submitted: false,
        }
    }

    fn start_read(&mut self) -> Result<(), BackendError> {
        let length = u32::try_from(self.buffer.len())
            .map_err(|error| BackendError::Backend(error.to_string()))?;
        // SAFETY: The handle is live and opened for overlapped reads. Both
        // allocations remain valid until completion, including on cancellation.
        let success = unsafe {
            ReadFile(
                self.handle.raw(),
                self.buffer.as_mut_ptr(),
                length,
                null_mut(),
                &raw mut *self.overlapped,
            )
        };
        self.started(success)
    }

    fn start_write(&mut self) -> Result<(), BackendError> {
        let length = u32::try_from(self.buffer.len())
            .map_err(|error| BackendError::Backend(error.to_string()))?;
        // SAFETY: Same ownership as start_read; initialized bytes are readable
        // for the whole write, and the OVERLAPPED allocation stays stable.
        let success = unsafe {
            WriteFile(
                self.handle.raw(),
                self.buffer.as_ptr(),
                length,
                null_mut(),
                &raw mut *self.overlapped,
            )
        };
        self.started(success)
    }

    fn started(&mut self, success: windows_sys::core::BOOL) -> Result<(), BackendError> {
        if success == 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != i32::try_from(ERROR_IO_PENDING).ok() {
                return Err(backend_error(&error));
            }
        }
        self.submitted = true;
        Ok(())
    }

    async fn finish(&mut self, gate: &DeviceIoGate) -> Result<usize, BackendError> {
        loop {
            gate.ensure_allowed()?;
            let mut transferred = 0;
            // SAFETY: This OVERLAPPED belongs to the pending operation on this
            // still-live handle. The out-pointer is valid; false never blocks.
            let success = unsafe {
                GetOverlappedResult(
                    self.handle.raw(),
                    &raw const *self.overlapped,
                    &raw mut transferred,
                    0,
                )
            };
            if success != 0 {
                self.submitted = false;
                gate.ensure_allowed()?;
                return Ok(transferred as usize);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() != i32::try_from(ERROR_IO_INCOMPLETE).ok() {
                // Terminal errors also complete the request; retain the guard
                // on errors so drop confirms completion before freeing storage.
                return Err(backend_error(&error));
            }
            tokio::time::sleep(COMPLETION_POLL).await;
        }
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.submitted {
            // SAFETY: The operation, buffer, OVERLAPPED and handle are alive.
            // CancelIoEx only requests cancellation. Waiting for its terminal
            // result is mandatory before Windows can lose these allocations.
            unsafe {
                CancelIoEx(self.handle.raw(), &raw const *self.overlapped);
                let mut transferred = 0;
                GetOverlappedResult(
                    self.handle.raw(),
                    &raw const *self.overlapped,
                    &raw mut transferred,
                    1,
                );
            }
        }
    }
}

#[async_trait]
impl RawWriter for Session {
    async fn write_output_report(&mut self, report: &[u8]) -> Result<(), BackendError> {
        self.gate.ensure_allowed()?;
        if report.len() != self.output_length {
            return Err(BackendError::Backend(
                "output report length does not match the selected collection".into(),
            ));
        }
        let mut pending = Pending::new(self.handle.clone(), report.into());
        pending.start_write()?;
        if pending.finish(&self.gate).await? != report.len() {
            return Err(BackendError::Backend("short HID output write".into()));
        }
        Ok(())
    }
}

#[async_trait]
impl RawSession for Session {
    async fn read_input_report(&mut self, buffer: &mut [u8]) -> Result<usize, BackendError> {
        self.gate.ensure_allowed()?;
        if buffer.len() < self.input_length {
            return Err(BackendError::Backend(
                "input buffer is smaller than the selected collection report".into(),
            ));
        }
        let mut pending = Pending::new(
            self.handle.clone(),
            vec![0; self.input_length].into_boxed_slice(),
        );
        pending.start_read()?;
        let length = pending.finish(&self.gate).await?;
        let report = pending
            .buffer
            .get(..length)
            .ok_or_else(|| BackendError::Backend("oversized input report".into()))?;
        buffer[..length].copy_from_slice(report);
        Ok(length)
    }
}
