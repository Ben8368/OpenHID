use async_hid::{AsyncHidRead as _, AsyncHidWrite as _, Device, DeviceReader, DeviceWriter};
use hidpp::async_trait;
use openhid_device::DeviceIoGate;
use openhid_device::backend::{BackendError, RawSession, RawWriter};

pub(crate) async fn open(
    device: &Device,
    gate: DeviceIoGate,
) -> Result<Box<dyn RawSession>, BackendError> {
    gate.ensure_allowed()?;
    let (reader, writer) = device.open().await.map_err(super::super::backend_error)?;
    gate.ensure_allowed()?;
    Ok(Box::new(Session {
        reader,
        writer,
        gate,
    }))
}

struct Session {
    reader: DeviceReader,
    writer: DeviceWriter,
    gate: DeviceIoGate,
}

#[async_trait]
impl RawWriter for Session {
    async fn write_output_report(&mut self, report: &[u8]) -> Result<(), BackendError> {
        self.gate.ensure_allowed()?;
        self.writer
            .write_output_report(report)
            .await
            .map_err(super::super::backend_error)?;
        self.gate.ensure_allowed()?;
        Ok(())
    }
}

#[async_trait]
impl RawSession for Session {
    async fn read_input_report(&mut self, buffer: &mut [u8]) -> Result<usize, BackendError> {
        self.gate.ensure_allowed()?;
        let length = self
            .reader
            .read_input_report(buffer)
            .await
            .map_err(super::super::backend_error)?;
        self.gate.ensure_allowed()?;
        Ok(length)
    }

    async fn exchange_output_report(
        &mut self,
        report: &[u8],
        input: &mut [u8],
    ) -> Result<usize, BackendError> {
        self.gate.ensure_allowed()?;
        let (reader, writer) = (&mut self.reader, &mut self.writer);
        let read = async {
            reader
                .read_input_report(input)
                .await
                .map_err(super::super::backend_error)
        };
        let write = async {
            writer
                .write_output_report(report)
                .await
                .map_err(super::super::backend_error)
        };
        let (length, ()) = tokio::try_join!(read, write)?;
        self.gate.ensure_allowed()?;
        Ok(length)
    }
}
