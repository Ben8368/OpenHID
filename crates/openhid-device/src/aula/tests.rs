use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use hidpp::async_trait;
use hidpp::channel::HidppChannel;

use super::*;
use crate::backend::{HotplugStream, RawSession, RawWriter};

// Literal frames captured from a real F87 PRO receiver; independent of the encoder.
const CAPTURE: [[u8; 20]; 10] = [
    [
        0x13, 0x44, 0x0a, 0, 0x0e, 0, 3, 3, 1, 0, 0, 4, 4, 7, 0, 0x0c, 0x20, 1, 0, 0xb2,
    ],
    [
        0x13, 0x44, 0x0a, 1, 0x0e, 1, 0, 0, 0, 1, 4, 3, 0, 0, 0xff, 0x0a, 0, 0, 0, 0x82,
    ],
    [
        0x13, 0x44, 0x0a, 2, 0x0e, 2, 0, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x78,
    ],
    [
        0x13, 0x44, 0x0a, 3, 0x0e, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x72,
    ],
    [
        0x13, 0x44, 0x0a, 4, 0x0e, 0xff, 0xff, 1, 0x40, 4, 0x47, 4, 0x47, 4, 0x40, 4, 0x47, 4,
        0x47, 0x22,
    ],
    [
        0x13, 0x44, 0x0a, 5, 0x0e, 2, 0x40, 4, 0x47, 4, 0x47, 4, 0x47, 4, 0x47, 3, 0x40, 4, 0x47,
        0x70,
    ],
    [
        0x13, 0x44, 0x0a, 6, 0x0e, 4, 0x47, 4, 0x47, 4, 0x47, 4, 0x37, 4, 0x37, 4, 0x37, 4, 0x37,
        0x42,
    ],
    [
        0x13, 0x44, 0x0a, 7, 0x0e, 7, 0x47, 7, 0x47, 7, 0x44, 7, 0x44, 7, 0x44, 7, 0x44, 7, 0x44,
        0x89,
    ],
    [
        0x13, 0x44, 0x0a, 8, 0x0e, 7, 0x44, 7, 0x44, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 0x35,
    ],
    [
        0x13, 0x44, 0x0a, 9, 2, 0x5a, 0xa5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x6b,
    ],
];

fn node() -> NodeInfo {
    NodeInfo {
        id: "test-aula".to_owned().into(),
        vendor_id: 0x3554,
        product_id: 0xfa09,
        usage_page: 0xff02,
        usage_id: 2,
        name: "F87".into(),
        manufacturer: Some("AULA".into()),
        serial_number: None,
    }
}

#[derive(Default)]
struct Evidence {
    writes: Mutex<Vec<Vec<u8>>>,
    opens: AtomicUsize,
    drops: AtomicUsize,
    reads: AtomicUsize,
}

struct Session {
    reports: VecDeque<Result<Vec<u8>, BackendError>>,
    evidence: Arc<Evidence>,
    stalled_write: bool,
    save_echo_delay: Duration,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.evidence.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl RawWriter for Session {
    async fn write_output_report(&mut self, report: &[u8]) -> Result<(), BackendError> {
        self.evidence
            .writes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(report.to_vec());
        if self.stalled_write {
            return std::future::pending().await;
        }
        Ok(())
    }
}

#[async_trait]
impl RawSession for Session {
    async fn read_input_report(&mut self, buffer: &mut [u8]) -> Result<usize, BackendError> {
        self.evidence.reads.fetch_add(1, Ordering::SeqCst);
        match self.reports.pop_front() {
            Some(report) => {
                let report = report?;
                if report == save_echo() {
                    tokio::time::sleep(self.save_echo_delay).await;
                }
                buffer[..report.len()].copy_from_slice(&report);
                Ok(report.len())
            }
            None => std::future::pending().await,
        }
    }
}

struct Backend {
    nodes: Vec<NodeInfo>,
    session: Mutex<Option<Session>>,
    evidence: Arc<Evidence>,
    stalled_open: bool,
}

impl Backend {
    fn new(reports: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let evidence = Arc::new(Evidence::default());
        Self {
            nodes: vec![node()],
            session: Mutex::new(Some(Session {
                reports: reports.into_iter().map(Ok).collect(),
                evidence: evidence.clone(),
                stalled_write: false,
                save_echo_delay: Duration::ZERO,
            })),
            evidence,
            stalled_open: false,
        }
    }
}

#[async_trait]
impl HidBackend for Backend {
    async fn enumerate(&self) -> Result<Vec<NodeInfo>, BackendError> {
        Ok(self.nodes.clone())
    }
    async fn enumerate_hidpp(&self) -> Result<Vec<NodeInfo>, BackendError> {
        panic!("AULA must not probe HID++")
    }
    async fn open_hidpp(&self, _: &NodeInfo) -> Result<Option<Arc<HidppChannel>>, BackendError> {
        panic!("AULA must not open HID++")
    }
    async fn open_raw_writer(&self, _: &NodeInfo) -> Result<Box<dyn RawWriter>, BackendError> {
        panic!("read path must retain both halves")
    }
    async fn open_raw_session(&self, _: &NodeInfo) -> Result<Box<dyn RawSession>, BackendError> {
        self.evidence.opens.fetch_add(1, Ordering::SeqCst);
        if self.stalled_open {
            return std::future::pending().await;
        }
        let session = self
            .session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or(BackendError::Disconnected)?;
        Ok(Box::new(session))
    }
    fn watch(&self) -> Result<HotplugStream, BackendError> {
        Ok(Box::new(futures_lite::stream::empty()))
    }
}

#[tokio::test]
async fn capture_reads_out_of_order_with_identical_duplicates_and_unrelated_traffic() {
    let mut reports = vec![
        vec![0x13, 0x45, 1],
        CAPTURE[4].to_vec(),
        CAPTURE[4].to_vec(),
    ];
    reports.extend(CAPTURE.into_iter().rev().map(Vec::from));
    let backend = Backend::new(reports);
    let config = read_selected_config(&backend, None).await.unwrap();
    assert_eq!(config.frames(), &CAPTURE);
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        *backend.evidence.writes.lock().unwrap(),
        vec![vec![
            0x13, 0x44, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x58
        ]]
    );
}

fn checksum(frame: &mut [u8; 20]) {
    frame[19] = frame[..19].iter().fold(0u8, |a, b| a.wrapping_add(*b));
}

#[tokio::test]
async fn rejects_malformed_config_without_publishing_partial_data() {
    let mut bad_checksum = CAPTURE[0];
    bad_checksum[19] ^= 1;
    let mut bad_sequence = CAPTURE[0];
    bad_sequence[3] = 10;
    checksum(&mut bad_sequence);
    let mut bad_length = CAPTURE[0];
    bad_length[4] = 15;
    checksum(&mut bad_length);
    let mut bad_terminator = CAPTURE[9];
    bad_terminator[5] = 0;
    checksum(&mut bad_terminator);
    for report in [
        bad_checksum.to_vec(),
        bad_sequence.to_vec(),
        bad_length.to_vec(),
        bad_terminator.to_vec(),
        CAPTURE[0][..19].to_vec(),
        [CAPTURE[0].to_vec(), vec![0]].concat(),
    ] {
        let backend = Backend::new([report]);
        assert!(matches!(
            read_config(&backend, &node()).await,
            Err(ReadError::InvalidResponse(_))
        ));
        assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn rejects_conflicting_duplicates() {
    let mut different = CAPTURE[0];
    different[5] ^= 1;
    checksum(&mut different);
    let backend = Backend::new([CAPTURE[0].to_vec(), different.to_vec()]);
    assert!(matches!(
        read_config(&backend, &node()).await,
        Err(ReadError::InvalidResponse("conflicting duplicate fragment"))
    ));
}

#[tokio::test(start_paused = true)]
async fn missing_fragment_times_out_and_drops_session() {
    let backend = Backend::new(CAPTURE[..9].iter().map(|f| f.to_vec()));
    let started = tokio::time::Instant::now();
    assert!(matches!(
        read_config(&backend, &node()).await,
        Err(ReadError::Timeout)
    ));
    assert_eq!(
        started.elapsed(),
        READ_ATTEMPT_TIMEOUT * u32::from(READ_ATTEMPTS)
    );
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn disconnect_is_typed_and_closes_session() {
    for response in [Ok(Vec::new()), Err(BackendError::Disconnected)] {
        let backend = Backend::new([]);
        backend
            .session
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .reports
            .push_back(response);
        assert!(matches!(
            read_config(&backend, &node()).await,
            Err(ReadError::Backend(BackendError::Disconnected))
        ));
        assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn rejects_wrong_collection_and_ambiguous_routes_before_open() {
    let mut backend = Backend::new([]);
    let mut wrong = node();
    wrong.usage_page = 0xff04;
    assert!(matches!(
        read_config(&backend, &wrong).await,
        Err(ReadError::UnsupportedCollection)
    ));
    backend.nodes.push(node());
    assert!(matches!(
        read_selected_config(&backend, None).await,
        Err(ReadError::Selection(_))
    ));
    assert!(matches!(
        crate::enumerate_standalone(&backend).await,
        Err(crate::InventoryError::AmbiguousRawDevice)
    ));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn inventory_marks_unresponsive_keyboard_offline_then_recovers_without_capabilities() {
    let backend = Backend::new(CAPTURE.map(Vec::from));
    let online = crate::enumerate_standalone(&backend).await.unwrap();
    assert_eq!(online.len(), 1);
    assert!(online[0].online);
    assert!(online[0].capabilities.is_none());
    assert!(online[0].light_capabilities.is_none());
    let offline = crate::enumerate_standalone(&backend).await.unwrap();
    assert_eq!(offline.len(), 1);
    assert!(!offline[0].online);
    *backend.session.lock().unwrap() = Some(Session {
        reports: CAPTURE.map(|f| Ok(f.to_vec())).into(),
        evidence: backend.evidence.clone(),
        stalled_write: false,
        save_echo_delay: Duration::ZERO,
    });
    assert!(crate::enumerate_standalone(&backend).await.unwrap()[0].online);
}

#[tokio::test(start_paused = true)]
async fn deadline_includes_open_and_identical_duplicates_cannot_complete_a_read() {
    let mut backend = Backend::new([]);
    backend.stalled_open = true;
    assert!(matches!(
        read_config(&backend, &node()).await,
        Err(ReadError::Timeout)
    ));
    assert!(backend.evidence.writes.lock().unwrap().is_empty());
    let backend = Backend::new([]);
    // Nine distinct fragments plus repetitions are still incomplete.
    let backend = Backend {
        session: Mutex::new(Some(Session {
            reports: CAPTURE[..9]
                .iter()
                .chain(std::iter::repeat_n(&CAPTURE[0], 20))
                .map(|f| Ok(f.to_vec()))
                .collect(),
            evidence: backend.evidence.clone(),
            stalled_write: false,
            save_echo_delay: Duration::ZERO,
        })),
        ..backend
    };
    assert!(matches!(
        read_config(&backend, &node()).await,
        Err(ReadError::Timeout)
    ));
}

#[tokio::test]
async fn cancellation_closes_pending_reader() {
    let backend = Backend::new([]);
    {
        let target = node();
        let read = read_config(&backend, &target);
        tokio::pin!(read);
        tokio::select! {
            biased;
            result = &mut read => panic!("empty reader completed: {result:?}"),
            () = std::future::ready(()) => {},
        }
        assert_eq!(backend.evidence.reads.load(Ordering::SeqCst), 1);
    }
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn failed_keyboard_does_not_suppress_other_standalone_devices() {
    let mut backend = Backend::new([]);
    backend.session.lock().unwrap().take();
    let mut headset = node();
    headset.id = "headset".to_owned().into();
    headset.vendor_id = 0x046d;
    headset.product_id = 0x0ad8;
    headset.usage_page = 0xff00;
    headset.usage_id = 1;
    backend.nodes.push(headset);
    let devices = crate::enumerate_standalone(&backend).await.unwrap();
    assert_eq!(devices.len(), 2);
    assert!(!devices[0].online);
    assert!(devices[1].online);
}

#[tokio::test]
async fn selects_distinct_serials_and_ignores_the_feature_collection() {
    let mut backend = Backend::new(CAPTURE.map(Vec::from));
    backend.nodes[0].serial_number = Some("ONE".into());
    let mut second = node();
    second.id = "second".to_owned().into();
    second.serial_number = Some("two".into());
    let mut feature = node();
    feature.usage_page = 0xff04;
    backend.nodes.extend([second, feature]);
    assert!(matches!(
        read_selected_config(&backend, None).await,
        Err(ReadError::Selection(_))
    ));
    assert!(matches!(
        read_selected_config(&backend, Some("serial:missing")).await,
        Err(ReadError::Selection(_))
    ));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 0);
    assert_eq!(
        read_selected_config(&backend, Some("serial:one"))
            .await
            .unwrap()
            .frames(),
        &CAPTURE
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_query_write_times_out_and_releases_session_without_reading() {
    let backend = Backend::new(CAPTURE.map(Vec::from));
    backend
        .session
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .stalled_write = true;
    let started = tokio::time::Instant::now();
    assert!(matches!(
        read_config(&backend, &node()).await,
        Err(ReadError::Timeout)
    ));
    assert_eq!(
        started.elapsed(),
        READ_ATTEMPT_TIMEOUT * u32::from(READ_ATTEMPTS)
    );
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), usize::from(READ_ATTEMPTS));
    assert!(writes.iter().all(|report| report.get(1) == Some(&0x44)));
    assert_eq!(backend.evidence.reads.load(Ordering::SeqCst), 0);
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
}

fn edited_lighting_capture() -> [[u8; 20]; 10] {
    // Independent wire oracle: effect 1, brightness 2, speed 2.
    let mut frames = CAPTURE;
    frames[0][15] = 1;
    frames[4][7] = 2;
    frames[4][8] = 0x20;
    for frame in &mut frames {
        checksum(frame);
    }
    frames
}

fn echoed_frames(frames: &[[u8; 20]; 10]) -> Vec<Vec<u8>> {
    frames
        .iter()
        .map(|frame| {
            let mut echo = *frame;
            echo[1] = 4;
            checksum(&mut echo);
            echo.to_vec()
        })
        .collect()
}

fn change() -> LightingChange {
    LightingChange::new(Some(1), Some(2), Some(2)).unwrap()
}

fn custom_effect_capture() -> [[u8; 20]; 10] {
    let mut frames = CAPTURE;
    frames[0][8] = 1;
    frames[0][14] = 0;
    frames[0][15] = 21;
    frames[0][17] = 1;
    for frame in &mut frames {
        checksum(frame);
    }
    frames
}

fn per_key_colors() -> AulaPerKeyRgb {
    AulaPerKeyRgb::new(
        (0..openhid_core::hid::aula::AULA_PER_KEY_LED_COUNT)
            .map(|index| {
                let led = u8::try_from(index).unwrap();
                AulaRgb::new(led, led.wrapping_add(1), led.wrapping_add(2))
            })
            .collect(),
    )
    .unwrap()
}

#[tokio::test]
async fn per_key_apply_selects_custom_mode_before_table_and_saves_verified_config() {
    let target = custom_effect_capture();
    let reports = per_key::encode_wireless_per_key_frames(&per_key_colors());
    let expected_reports = reports.map(Vec::from);
    let mut incoming = CAPTURE.map(Vec::from).to_vec();
    incoming.extend(echoed_frames(&target));
    incoming.extend(target.map(Vec::from));
    incoming.extend(expected_reports.iter().cloned());
    incoming.push(save_echo());
    incoming.extend(target.map(Vec::from));
    let backend = Backend::new(incoming);

    let state = apply_selected_per_key_rgb(&backend, None, &per_key_colors(), || false)
        .await
        .unwrap();

    assert_eq!(state.lighting.effect, 21);
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), 42);
    assert_eq!(writes[0][1], 0x44, "the complete baseline is read first");
    assert_eq!(&writes[1..11], echoed_frames(&target));
    assert_eq!(writes[11][1], 0x44, "full config readback follows staging");
    assert_eq!(
        &writes[12..40],
        expected_reports.as_slice(),
        "custom mode is staged before the 28 color frames"
    );
    assert_eq!(writes[40], save_echo());
    assert_eq!(writes[41][1], 0x44, "full config readback follows SAVE");
}

#[tokio::test]
async fn per_key_apply_refuses_to_replace_an_active_custom_table() {
    let current = custom_effect_capture();
    let backend = Backend::new(current.map(Vec::from));
    assert!(matches!(
        apply_selected_per_key_rgb(&backend, None, &per_key_colors(), || false).await,
        Err(PerKeyRgbError::CustomEffectAlreadyActive)
    ));
    assert_eq!(
        backend.evidence.writes.lock().unwrap().len(),
        1,
        "only the read request was sent"
    );
}

#[tokio::test(start_paused = true)]
async fn per_key_partial_failure_does_not_claim_unverified_configuration_recovery() {
    let frames = per_key::encode_wireless_per_key_frames(&per_key_colors());
    let mut incoming = CAPTURE.map(Vec::from).to_vec();
    let target = custom_effect_capture();
    incoming.extend(echoed_frames(&target));
    incoming.extend(target.map(Vec::from));
    incoming.extend(frames[..2].iter().map(|frame| frame.to_vec()));
    let backend = Backend::new(incoming);

    assert!(matches!(
        apply_selected_per_key_rgb(&backend, None, &per_key_colors(), || false).await,
        Err(PerKeyRgbError::RecoveryFailed { cause, .. })
            if matches!(*cause, PerKeyRgbError::PartialWrite {
                completed_reports: 2,
                failed_sequence: 2,
                ..
            })
    ));
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(
        writes[12..15]
            .iter()
            .map(|frame| frame[1])
            .collect::<Vec<_>>(),
        [2, 2, 2]
    );
    assert!(writes[15..].iter().all(|frame| frame[1] == 0x04));
}

#[test]
fn aula_state_decodes_verified_sleep_timeout_and_preserves_unknown_values() {
    let config = WirelessConfig::from_frames(CAPTURE).unwrap();
    assert_eq!(config.aula_state().sleep_timeout.unwrap().seconds(), 300);
    let mut unknown = CAPTURE;
    unknown[1][15] = 11;
    checksum(&mut unknown[1]);
    assert_eq!(
        WirelessConfig::from_frames(unknown)
            .unwrap()
            .aula_state()
            .sleep_timeout,
        None
    );
    assert_eq!(SleepTimeout::from_seconds(600).unwrap().wire_value(), 20);
}

#[tokio::test]
async fn sleep_timeout_edit_preserves_lighting_and_unknown_bytes() {
    let mut target = CAPTURE;
    target[1][15] = SleepTimeout::from_seconds(600).unwrap().wire_value();
    checksum(&mut target[1]);
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&target));
    reports.extend(target.map(Vec::from));
    let backend = Backend::new(reports);
    let change = AulaChange::new(None, Some(SleepTimeout::from_seconds(600).unwrap())).unwrap();
    let result = update_config(
        &backend,
        None,
        ConfigChange::Settings(change),
        LightingMode::Apply,
        || false,
    )
    .await
    .unwrap();
    assert_eq!(result.changed.frames()[0], CAPTURE[0]);
    assert_eq!(result.changed.frames()[1][15], 20);
    assert_eq!(result.state.sleep_timeout.unwrap().seconds(), 600);
    assert_eq!(result.changed.frames()[4], CAPTURE[4]);
}

#[tokio::test]
async fn lighting_apply_preserves_unknown_bytes_and_verifies_full_readback_without_save() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    reports.extend(edited.map(Vec::from));
    let backend = Backend::new(reports);
    let result = update_lighting(&backend, None, change(), LightingMode::Apply, || false)
        .await
        .unwrap();
    assert_eq!(result.before.frames(), &CAPTURE);
    assert_eq!(result.changed.frames(), &edited);
    assert_eq!(
        result.changed.lighting(),
        LightingState {
            effect: 1,
            brightness: Some(2),
            speed: Some(2),
            color_mode: Some(AulaColorModeState::SingleColor),
        }
    );
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), 12);
    assert_eq!(&writes[1..11], echoed_frames(&edited));
    assert_eq!(writes[0], writes[11]);
    assert!(writes.iter().all(|frame| matches!(frame[1], 4 | 0x44)));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 1);
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lighting_preview_restores_every_original_byte_and_proves_restoration() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    reports.extend(edited.map(Vec::from));
    reports.extend(echoed_frames(&CAPTURE));
    reports.extend(CAPTURE.map(Vec::from));
    let backend = Backend::new(reports);
    let result = update_lighting(&backend, None, change(), LightingMode::Preview, || false)
        .await
        .unwrap();
    assert_eq!(result.changed.frames(), &edited);
    assert_eq!(result.current, result.before.lighting());
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), 23);
    assert_eq!(&writes[12..22], echoed_frames(&CAPTURE));
    assert_eq!(writes[0], writes[22]);
    assert!(writes.iter().all(|frame| frame[1] != 0x0a));
}

#[tokio::test]
async fn lighting_readback_mismatch_rolls_back_instead_of_reporting_echo_success() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    // All writes echo, but the device still reports the original settings.
    reports.extend(CAPTURE.map(Vec::from));
    reports.extend(echoed_frames(&CAPTURE));
    reports.extend(CAPTURE.map(Vec::from));
    let backend = Backend::new(reports);
    assert!(matches!(
        update_lighting(&backend, None, change(), LightingMode::Apply, || false).await,
        Err(ConfigError::Restored(cause)) if matches!(*cause, ConfigError::ReadbackMismatch)
    ));
    assert_eq!(backend.evidence.writes.lock().unwrap().len(), 23);
}

#[tokio::test(start_paused = true)]
async fn lighting_ack_requires_exact_command_subcommand_sequence_payload_and_checksum() {
    for offset in [0, 1, 2, 3, 4, 7, 19] {
        let edited = edited_lighting_capture();
        let mut reports = CAPTURE.map(Vec::from).to_vec();
        let mut foreign_echo = echoed_frames(&edited)[0].clone();
        foreign_echo[offset] ^= 1;
        reports.push(foreign_echo);
        // Recovery echoes must not be mistaken for the target packet either.
        // No matching ACK -> timeout and no next target fragment.
        let backend = Backend::new(reports);
        assert!(
            matches!(
                update_lighting(&backend, None, change(), LightingMode::Apply, || false).await,
                Err(ConfigError::RecoveryFailed { .. })
            ),
            "offset {offset}"
        );
        let writes = backend.evidence.writes.lock().unwrap();
        assert_eq!(
            writes.len(),
            3,
            "one read, one target packet, one recovery attempt"
        );
        assert_eq!(writes[1], echoed_frames(&edited)[0]);
        assert_eq!(writes[2], echoed_frames(&CAPTURE)[0]);
    }
}

#[tokio::test]
async fn lighting_ignores_stale_and_malformed_echoes_before_the_exact_packet() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    for echo in echoed_frames(&edited) {
        reports.push(vec![0x13, 4]);
        reports.push(vec![0x13, 4, 10, 9]);
        let mut stale = echo.clone();
        stale[3] = 200;
        reports.push(stale);
        reports.push(echo);
    }
    reports.extend(edited.map(Vec::from));
    let backend = Backend::new(reports);
    update_lighting(&backend, None, change(), LightingMode::Apply, || false)
        .await
        .unwrap();
    assert_eq!(backend.evidence.writes.lock().unwrap().len(), 12);
}

#[tokio::test]
async fn lighting_cancellation_between_fragments_restores_in_the_same_session() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.push(echoed_frames(&edited)[0].clone());
    reports.extend(echoed_frames(&CAPTURE));
    reports.extend(CAPTURE.map(Vec::from));
    let backend = Backend::new(reports);
    let checks = AtomicUsize::new(0);
    assert!(matches!(
        update_lighting(&backend, None, change(), LightingMode::Apply,
            || checks.fetch_add(1, Ordering::SeqCst) >= 2).await,
        Err(ConfigError::Restored(cause)) if matches!(*cause, ConfigError::Cancelled)
    ));
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), 13);
    assert_eq!(&writes[2..12], echoed_frames(&CAPTURE));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn lighting_rejects_unsupported_and_empty_edits_and_never_templates_missing_config() {
    for (effect, brightness, speed) in [
        (Some(19), None, None),
        (Some(21), None, None),
        (Some(0), Some(1), None),
        (None, Some(5), None),
        (None, None, Some(5)),
        (None, None, None),
    ] {
        let _ = LightingChange::new(effect, brightness, speed).unwrap_err();
    }
    let backend = Backend::new([vec![]]);
    update_lighting(&backend, None, change(), LightingMode::Apply, || false)
        .await
        .unwrap_err();
    assert_eq!(backend.evidence.writes.lock().unwrap().len(), 1);
    let mut custom = CAPTURE;
    custom[0][15] = 21;
    checksum(&mut custom[0]);
    let backend = Backend::new(custom.map(Vec::from));
    assert!(matches!(
        update_lighting(
            &backend,
            None,
            LightingChange::new(None, Some(1), None).unwrap(),
            LightingMode::Apply,
            || false
        )
        .await,
        Err(ConfigError::InvalidSetting)
    ));
    assert_eq!(backend.evidence.writes.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn lighting_noop_and_pre_cancelled_edits_do_not_write_configuration() {
    for cancelled in [false, true] {
        let backend = Backend::new(CAPTURE.map(Vec::from));
        let result = update_lighting(
            &backend,
            None,
            LightingChange::new(Some(12), Some(3), Some(4)).unwrap(),
            LightingMode::Apply,
            || cancelled,
        )
        .await;
        if cancelled {
            assert!(matches!(result, Err(ConfigError::Cancelled)));
        } else {
            assert_eq!(result.unwrap().changed.frames(), &CAPTURE);
        }
        assert_eq!(backend.evidence.writes.lock().unwrap().len(), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn preview_restore_failure_is_not_falsely_reported_as_caller_cancellation() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    reports.extend(edited.map(Vec::from));
    reports.extend(echoed_frames(&CAPTURE));
    // Restore echoes arrive, but the restoration readback query exhausts its retries.
    let backend = Backend::new(reports);
    assert!(matches!(
        update_lighting(&backend, None, change(), LightingMode::Preview, || false).await,
        Err(ConfigError::PreviewRestoreFailed(_))
    ));
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(
        writes
            .iter()
            .filter(|report| report.get(1) == Some(&0x04))
            .count(),
        20
    );
    assert_eq!(
        writes
            .iter()
            .filter(|report| report.get(1) == Some(&0x44))
            .count(),
        usize::from(READ_ATTEMPTS) + 2
    );
}

fn save_echo() -> Vec<u8> {
    vec![
        0x13, 0x0a, 1, 0, 4, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x29,
    ]
}

#[tokio::test]
async fn lighting_save_only_follows_exact_echoes_and_full_staged_readback() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    reports.extend(edited.map(Vec::from));
    reports.push(save_echo());
    reports.extend(edited.map(Vec::from));
    let backend = Backend::new(reports);
    update_lighting(&backend, None, change(), LightingMode::Save, || false)
        .await
        .unwrap();
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(writes.len(), 14);
    assert_eq!(writes[12], save_echo());
    assert_eq!(writes[0], writes[13]);
}

#[tokio::test]
async fn explicit_save_commits_an_already_staged_snapshot() {
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&CAPTURE));
    reports.extend(CAPTURE.map(Vec::from));
    reports.push(save_echo());
    reports.extend(CAPTURE.map(Vec::from));
    let backend = Backend::new(reports);
    update_lighting(
        &backend,
        None,
        LightingChange::new(Some(12), Some(3), Some(4)).unwrap(),
        LightingMode::Save,
        || false,
    )
    .await
    .unwrap();
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(
        writes.len(),
        14,
        "an explicit save must not skip committing a staged snapshot"
    );
    assert_eq!(writes[12], save_echo());
}

#[tokio::test(start_paused = true)]
async fn save_accepts_the_measured_flash_commit_latency() {
    let edited = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&edited));
    reports.extend(edited.map(Vec::from));
    reports.push(save_echo());
    reports.extend(edited.map(Vec::from));
    let backend = Backend::new(reports);
    // The physical SAVE echo took 609 ms; the previous 500 ms budget
    // incorrectly declared failure and began compensation during flash commit.
    backend
        .session
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .save_echo_delay = Duration::from_millis(650);
    let started = tokio::time::Instant::now();
    update_lighting(&backend, None, change(), LightingMode::Save, || false)
        .await
        .unwrap();
    assert_eq!(started.elapsed(), Duration::from_millis(650));
    assert_eq!(backend.evidence.writes.lock().unwrap().len(), 14);
}

#[test]
fn configuration_backups_reject_wrong_family_version_corruption_and_reordered_frames() {
    let config = WirelessConfig::from_frames(CAPTURE).unwrap();
    let encoded = serde_json::to_value(&config).unwrap();
    assert_eq!(
        serde_json::from_value::<WirelessConfig>(encoded.clone()).unwrap(),
        config
    );
    for (field, invalid) in [
        ("version", serde_json::json!(2)),
        ("driver", serde_json::json!("other-device")),
    ] {
        let mut value = encoded.clone();
        value[field] = invalid;
        serde_json::from_value::<WirelessConfig>(value).unwrap_err();
    }
    let mut value = encoded.clone();
    value["serial"] = serde_json::json!("not part of a backup");
    serde_json::from_value::<WirelessConfig>(value).unwrap_err();
    for mutation in 0..5 {
        let mut frames = CAPTURE;
        match mutation {
            0 => frames.swap(0, 1),
            1 => frames[2][19] ^= 1,
            2 => {
                frames[2][1] = 4;
                checksum(&mut frames[2]);
            }
            3 => {
                frames[9][5] = 0;
                checksum(&mut frames[9]);
            }
            _ => {
                frames[3][4] = 13;
                checksum(&mut frames[3]);
            }
        }
        WirelessConfig::from_frames(frames).unwrap_err();
    }
    let mut value = encoded;
    value["frames"].as_array_mut().unwrap().pop();
    serde_json::from_value::<WirelessConfig>(value).unwrap_err();
}

#[tokio::test]
async fn configuration_restore_saves_every_unknown_byte_and_reads_back_after_save() {
    let mut target = CAPTURE;
    target[0][15] = 21; // Preserve custom effect, outside the lighting editor's range.
    target[3][12] = 0x93; // A field whose meaning the implementation does not assume.
    checksum(&mut target[0]);
    checksum(&mut target[3]);
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&target));
    reports.extend(target.map(Vec::from));
    reports.push(save_echo());
    reports.extend(target.map(Vec::from));
    let backend = Backend::new(reports);
    let result = update_config(
        &backend,
        None,
        ConfigChange::Restore(WirelessConfig::from_frames(target).unwrap()),
        LightingMode::Save,
        || false,
    )
    .await
    .unwrap();
    assert_eq!(result.changed.frames(), &target);
    assert_eq!(result.before.frames(), &CAPTURE);
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(&writes[1..11], echoed_frames(&target));
    assert_eq!(writes[12], save_echo());
    assert_eq!(writes.len(), 14);
}

#[tokio::test]
async fn configuration_restore_mismatch_recovers_the_current_device_snapshot() {
    let target = edited_lighting_capture();
    let mut reports = CAPTURE.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&target));
    reports.extend(CAPTURE.map(Vec::from)); // Device rejected target.
    reports.extend(echoed_frames(&CAPTURE));
    reports.extend(CAPTURE.map(Vec::from));
    reports.push(save_echo());
    reports.extend(CAPTURE.map(Vec::from));
    let backend = Backend::new(reports);
    assert!(
        matches!(update_config(&backend, None, ConfigChange::Restore(WirelessConfig::from_frames(target).unwrap()), LightingMode::Save, || false).await, Err(ConfigError::Restored(cause)) if matches!(*cause, ConfigError::ReadbackMismatch))
    );
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(&writes[12..22], echoed_frames(&CAPTURE));
    assert_eq!(writes[23], save_echo());
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sleep_preview_restores_unknown_lighting_and_every_configuration_byte() {
    let mut original = CAPTURE;
    original[0][15] = 21;
    original[1][15] = 10;
    original[3][12] = 0x93;
    checksum(&mut original[0]);
    checksum(&mut original[1]);
    checksum(&mut original[3]);
    let mut target = original;
    target[1][15] = 20; // Independently measured OEM 300 -> 600 second edit.
    checksum(&mut target[1]);
    let mut reports = original.map(Vec::from).to_vec();
    reports.extend(echoed_frames(&target));
    reports.extend(target.map(Vec::from));
    reports.extend(echoed_frames(&original));
    reports.extend(original.map(Vec::from));
    let backend = Backend::new(reports);
    let change = AulaChange::new(None, SleepTimeout::from_seconds(600)).unwrap();
    let result = update_config(
        &backend,
        None,
        ConfigChange::Settings(change),
        LightingMode::Preview,
        || false,
    )
    .await
    .unwrap();
    assert_eq!(result.before.frames(), &original);
    assert_eq!(result.changed.frames(), &target);
    assert_eq!(result.state.lighting.effect, 21);
    assert_eq!(result.state.sleep_timeout.unwrap().seconds(), 300);
    let writes = backend.evidence.writes.lock().unwrap();
    assert_eq!(&writes[1..11], echoed_frames(&target));
    assert_eq!(&writes[12..22], echoed_frames(&original));
    assert_eq!(writes.len(), 23, "preview never sends SAVE");
}

// Synthetic protocol examples, not a physical-device capture. Four raw entries
// split across two frames; the literal sums are independent of the encoder.
const MATRIX_EXAMPLE: [[u8; 20]; 2] = [
    [
        0x13, 0x41, 2, 0, 14, 2, 0, 4, 0, 2, 0, 5, 0, 2, 0, 6, 0, 2, 0, 0x7b,
    ],
    [
        0x13, 0x41, 2, 1, 2, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x60,
    ],
];

#[tokio::test]
async fn wired_macro_write_guard_refuses_before_opening_hardware() {
    let backend = Backend::new([]);
    let original = WiredMacroTable::from_bytes(vec![0; 4096]).unwrap();

    assert!(matches!(
        write_wired_macro_table(&backend, Some("test-identity"), &original, &original).await,
        Err(WiredMacroWriteError::ProtocolUnverified)
    ));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 0);
    assert!(backend.evidence.writes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn matrix_read_reorders_complete_raw_entries_and_sends_only_the_read_request() {
    let mut reports = vec![
        CAPTURE[0].to_vec(),
        MATRIX_EXAMPLE[1].to_vec(),
        MATRIX_EXAMPLE[1].to_vec(),
    ];
    reports.push(MATRIX_EXAMPLE[0].to_vec());
    let backend = Backend::new(reports);
    let matrix = read_selected_matrix(&backend, None).await.unwrap();
    assert_eq!(
        matrix.entries().collect::<Vec<_>>(),
        vec![
            &[2, 0, 4, 0][..],
            &[2, 0, 5, 0],
            &[2, 0, 6, 0],
            &[2, 0, 7, 0]
        ]
    );
    assert_eq!(
        *backend.evidence.writes.lock().unwrap(),
        vec![vec![
            0x13, 0x41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x55
        ]]
    );
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn matrix_read_refuses_malformed_fragment_headers_payloads_and_conflicting_replies() {
    for mutation in 0..9 {
        let mut frame = MATRIX_EXAMPLE[0];
        match mutation {
            0 => frame[2] = 0,
            1 => frame[2] = 38,
            2 => frame[3] = 2,
            3 => frame[4] = 0,
            4 => frame[4] = 15,
            5 => frame[4] = 0x1e,
            6 => frame[4] = 12,
            7 => frame[19] ^= 1,
            _ => frame[3] = 255,
        }
        if mutation != 7 {
            checksum(&mut frame);
        }
        let backend = Backend::new([frame.to_vec()]);
        assert!(
            matches!(
                read_selected_matrix(&backend, None).await,
                Err(ReadError::InvalidResponse(_))
            ),
            "mutation {mutation}"
        );
        assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
    }
    let mut other = MATRIX_EXAMPLE[0];
    other[5] ^= 1;
    checksum(&mut other);
    let backend = Backend::new([MATRIX_EXAMPLE[0].to_vec(), other.to_vec()]);
    assert!(matches!(
        read_selected_matrix(&backend, None).await,
        Err(ReadError::InvalidResponse("conflicting duplicate fragment"))
    ));
    other = MATRIX_EXAMPLE[0];
    other[2] = 3;
    checksum(&mut other);
    let backend = Backend::new([MATRIX_EXAMPLE[0].to_vec(), other.to_vec()]);
    assert!(matches!(
        read_selected_matrix(&backend, None).await,
        Err(ReadError::InvalidResponse("inconsistent fragment count"))
    ));
    let mut end = MATRIX_EXAMPLE[1];
    end[4] = 1;
    checksum(&mut end);
    let backend = Backend::new([MATRIX_EXAMPLE[0].to_vec(), end.to_vec()]);
    assert!(matches!(
        read_selected_matrix(&backend, None).await,
        Err(ReadError::InvalidResponse("invalid key matrix size"))
    ));
}

#[tokio::test(start_paused = true)]
async fn matrix_read_never_publishes_missing_fragments_and_rejects_ambiguous_targets_before_open() {
    let mut backend = Backend::new([MATRIX_EXAMPLE[0].to_vec()]);
    assert!(matches!(
        read_selected_matrix(&backend, None).await,
        Err(ReadError::Timeout)
    ));
    assert_eq!(backend.evidence.drops.load(Ordering::SeqCst), 1);
    backend.nodes.push(node());
    assert!(matches!(
        read_selected_matrix(&backend, None).await,
        Err(ReadError::Selection(_))
    ));
    assert_eq!(backend.evidence.opens.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn matrix_read_preserves_the_full_oem_bound_and_refuses_oversized_data() {
    for (last_length, expected) in [(8, Some(128)), (12, None)] {
        let mut reports = Vec::new();
        for sequence in 0..37 {
            let mut frame = [0; 20];
            frame[..5].copy_from_slice(&[
                0x13,
                0x41,
                37,
                sequence,
                if sequence == 36 { last_length } else { 14 },
            ]);
            checksum(&mut frame);
            reports.push(frame.to_vec());
        }
        let backend = Backend::new(reports);
        match expected {
            Some(count) => assert_eq!(
                read_selected_matrix(&backend, None)
                    .await
                    .unwrap()
                    .entries()
                    .len(),
                count
            ),
            None => assert!(matches!(
                read_selected_matrix(&backend, None).await,
                Err(ReadError::InvalidResponse("invalid key matrix size"))
            )),
        }
    }
}

#[test]
fn matrix_backups_preserve_opaque_entries_and_reject_wrong_regions_and_partial_data() {
    let matrix = WirelessMatrix::from_frames(MATRIX_EXAMPLE.to_vec()).unwrap();
    assert_eq!(matrix.frames(), &MATRIX_EXAMPLE);
    let encoded = serde_json::to_value(&matrix).unwrap();
    assert_eq!(encoded["region"], "wireless-base-matrix");
    assert_eq!(
        serde_json::from_value::<WirelessMatrix>(encoded.clone()).unwrap(),
        matrix
    );
    serde_json::from_value::<WirelessConfig>(encoded.clone()).unwrap_err();
    let config = serde_json::to_value(WirelessConfig::from_frames(CAPTURE).unwrap()).unwrap();
    serde_json::from_value::<WirelessMatrix>(config).unwrap_err();
    for (field, value) in [
        ("version", serde_json::json!(2)),
        ("driver", serde_json::json!("other-device")),
        ("region", serde_json::json!("fn-matrix")),
        ("serial", serde_json::json!("not a backup field")),
    ] {
        let mut invalid = encoded.clone();
        invalid[field] = value;
        serde_json::from_value::<WirelessMatrix>(invalid).unwrap_err();
    }
    for mutation in 0..9 {
        let mut frames = MATRIX_EXAMPLE.to_vec();
        match mutation {
            0 => frames.clear(),
            1 => {
                frames.pop();
            }
            2 => frames.swap(0, 1),
            3 => frames.push(frames[1]),
            4 => frames[0][19] ^= 1,
            5 => {
                frames[0][1] = 0x44;
                checksum(&mut frames[0]);
            }
            6 => {
                frames[0][2] = 38;
                checksum(&mut frames[0]);
            }
            7 => {
                frames[1][4] = 1;
                checksum(&mut frames[1]);
            }
            _ => {
                frames[0][4] = 13;
                checksum(&mut frames[0]);
            }
        }
        WirelessMatrix::from_frames(frames).unwrap_err();
    }
}

#[test]
fn wired_identity_accepts_copyable_windows_unc_spellings() {
    let node = r#"id:UncPath("\\?\HID#VID_258A&PID_010C&MI_01&Col06#7&255edd5&0&0005#{4d1e55b2-f16f-11cf-88cb-001111000030}")"#;
    assert!(super::wired::wired_identity_matches(node, node));
    assert!(super::wired::wired_identity_matches(
        node,
        r"id:UncPath(\?\HID#VID_258A&PID_010C&MI_01&Col06#7&255edd5&0&0005#{4d1e55b2-f16f-11cf-88cb-001111000030})",
    ));
    assert!(!super::wired::wired_identity_matches(
        node,
        "id:UncPath(\\\\?\\HID#VID_258A&PID_010C&MI_01&Col05#7&255edd5&0&0004#{4d1e55b2-f16f-88cb-001111000030})",
    ));
}
