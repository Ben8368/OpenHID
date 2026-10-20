//! Agent policy proofs use an injected replay; no native device is opened.

use super::*;
use openhid_core::hid::aula::{LightingChange, LightingMode};
use openhid_hid::replay::{
    NodePresence, OpenOutcome, RawWriterAvailability, ReplayBackend, ReplayNode, ReplayTopology,
};
use openhid_hid::{NodeInfo, device_io_channel};

fn setup() -> (
    HardwareContext,
    openhid_hid::DeviceIoSignal,
    Arc<ReplayBackend>,
    DeviceRoute,
) {
    let info = NodeInfo {
        id: "agent-aula".to_owned().into(),
        vendor_id: 0x3554,
        product_id: 0xfa09,
        usage_page: 0xff02,
        usage_id: 2,
        name: "Test F87 PRO".into(),
        manufacturer: Some("AULA".into()),
        serial_number: None,
    };
    let route = DeviceRoute::from(&openhid_core::device::RawDeviceAddress {
        vendor_id: info.vendor_id,
        product_id: info.product_id,
        usage_page: info.usage_page,
        usage_id: info.usage_id,
        identity: info.identity(),
    });
    let backend = Arc::new(
        ReplayBackend::new(
            ReplayTopology {
                nodes: vec![ReplayNode {
                    info,
                    presence: NodePresence::Present,
                    open_outcome: OpenOutcome::NotHidpp,
                    channel: None,
                    raw_writer: RawWriterAvailability::Capture,
                    receiver_slots: Vec::new(),
                }],
                channels: Vec::new(),
            },
            Vec::new(),
        )
        .unwrap(),
    );
    let (signal, gate) = device_io_channel();
    (
        HardwareContext::injected(backend.clone(), gate),
        signal,
        backend,
        route,
    )
}

#[tokio::test]
async fn aula_suspended_context_refuses_reads_and_writes() {
    let (hardware, signal, backend, route) = setup();
    assert!(signal.suspend());
    assert_eq!(
        hardware.read_aula_lighting(&route).await,
        Err(WriteError::DeviceNotFound)
    );
    assert_eq!(
        hardware
            .set_aula_lighting(
                &route,
                LightingChange::new(Some(1), None, None).unwrap(),
                LightingMode::Save
            )
            .await,
        Err(WriteError::DeviceNotFound)
    );
    assert_eq!(
        backend.open_count(&"agent-aula".to_owned().into()).unwrap(),
        0
    );
}

#[tokio::test]
async fn aula_rejects_neighbor_collection_and_hidpp_routes() {
    let (hardware, _, backend, mut route) = setup();
    if let DeviceRoute::RawHid { usage_page, .. } = &mut route {
        *usage_page = 0xff04;
    }
    for route in [
        route,
        DeviceRoute::Direct {
            vendor_id: 0x3554,
            product_id: 0xfa09,
        },
    ] {
        assert_eq!(
            hardware.read_aula_lighting(&route).await,
            Err(WriteError::DeviceNotFound)
        );
        assert_eq!(
            hardware
                .set_aula_lighting(
                    &route,
                    LightingChange::new(Some(1), None, None).unwrap(),
                    LightingMode::Apply
                )
                .await,
            Err(WriteError::DeviceNotFound)
        );
    }
    assert_eq!(
        backend.open_count(&"agent-aula".to_owned().into()).unwrap(),
        0
    );
}

#[tokio::test]
async fn aula_read_and_owned_write_use_only_the_injected_backend() {
    let (hardware, _, backend, route) = setup();
    // Replay deliberately has no duplex session. Its own error must return;
    // accidentally falling back to the real receiver would touch hardware.
    let read = hardware.read_aula_lighting(&route).await;
    assert!(
        matches!(read, Err(WriteError::Hid(message)) if message.contains("this backend has no raw input session"))
    );
    let write = hardware
        .set_aula_lighting(
            &route,
            LightingChange::new(Some(1), None, None).unwrap(),
            LightingMode::Save,
        )
        .await;
    assert!(
        matches!(write, Err(WriteError::Hid(message)) if message.contains("this backend has no raw input session"))
    );
    assert_eq!(
        backend.open_count(&"agent-aula".to_owned().into()).unwrap(),
        0,
        "no HID++ channel may open"
    );
    backend.require_complete().unwrap();
}

#[tokio::test]
async fn aula_settings_honor_suspend_identity_and_injected_backend() {
    use openhid_core::hid::aula::{AulaChange, SleepTimeout};
    let (hardware, signal, backend, route) = setup();
    let change = AulaChange::new(None, SleepTimeout::from_seconds(600)).unwrap();
    let read = hardware.read_aula_state(&route).await;
    assert!(
        matches!(read, Err(WriteError::Hid(message)) if message.contains("this backend has no raw input session"))
    );
    let write = hardware
        .set_aula_settings(&route, change, LightingMode::Save)
        .await;
    assert!(
        matches!(write, Err(WriteError::Hid(message)) if message.contains("this backend has no raw input session"))
    );
    let mut neighbor = route.clone();
    if let DeviceRoute::RawHid { usage_page, .. } = &mut neighbor {
        *usage_page = 0xff04;
    }
    assert_eq!(
        hardware.read_aula_state(&neighbor).await,
        Err(WriteError::DeviceNotFound)
    );
    assert_eq!(
        hardware
            .set_aula_settings(&neighbor, change, LightingMode::Apply)
            .await,
        Err(WriteError::DeviceNotFound)
    );
    assert!(signal.suspend());
    assert_eq!(
        hardware.read_aula_state(&route).await,
        Err(WriteError::DeviceNotFound)
    );
    assert_eq!(
        hardware
            .set_aula_settings(&route, change, LightingMode::Save)
            .await,
        Err(WriteError::DeviceNotFound)
    );
    assert_eq!(
        backend.open_count(&"agent-aula".to_owned().into()).unwrap(),
        0
    );
    backend.require_complete().unwrap();
}
