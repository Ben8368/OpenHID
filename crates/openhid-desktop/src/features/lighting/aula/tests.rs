use super::*;

#[test]
fn unknown_effect_requires_explicit_selection_before_writing() {
    let mut editor = Editor::default();
    editor.received(LightingState {
        effect: 21,
        brightness: None,
        speed: None,
        color_mode: None,
    });
    assert!(editor.change().is_none());
    editor.select_effect(12);
    let change = editor.change().unwrap();
    assert_eq!(change.effect(), Some(12));
    assert_eq!(change.brightness(), None);
    assert_eq!(change.speed(), None);
}

#[test]
fn switching_effect_preserves_its_device_parameters_until_explicitly_changed() {
    let mut editor = Editor::default();
    editor.received(LightingState {
        effect: 12,
        brightness: Some(3),
        speed: Some(4),
        color_mode: None,
    });
    editor.select_effect(1);
    assert_eq!(editor.change().unwrap().brightness(), None);
    assert_eq!(editor.change().unwrap().speed(), None);
    editor.select_effect(12);
    assert_eq!(editor.change().unwrap().brightness(), Some(3));
    editor.select_effect(0);
    let change = editor.change().unwrap();
    assert_eq!(change.effect(), Some(0));
    assert_eq!(change.brightness(), None);
    assert_eq!(change.speed(), None);
}

use crate::services::{assets::AssetResolver, ipc::Command};
use crate::state::{AgentLink, Sources};
use gpui::{Entity, TestAppContext};
use openhid_core::device::{DeviceKind, RawDeviceAddress, StandaloneDevice};
use openhid_ipc::{AgentStatus, InventoryHealth, PROTOCOL_VERSION};
use tokio::sync::mpsc;

fn setup(cx: &mut TestAppContext) -> (Entity<AulaLightingPanel>, mpsc::UnboundedReceiver<Command>) {
    let (commands, received) = mpsc::unbounded_channel();
    let cx = cx.add_empty_window();
    let panel = cx.update(|window, cx| {
        gpui_component::init(cx);
        theme::register_builtin_themes(cx);
        let keyboards: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|id| StandaloneDevice {
                address: RawDeviceAddress {
                    vendor_id: 0x3554,
                    product_id: 0xfa09,
                    usage_page: 0xff02,
                    usage_id: 2,
                    identity: format!("serial:synthetic-{id}"),
                },
                display_name: format!("F87 PRO {id}"),
                manufacturer: Some("AULA".into()),
                serial_number: None,
                unit_id: [0; 4],
                kind: DeviceKind::Keyboard,
                online: true,
                capabilities: None,
                light_capabilities: None,
                driver_id: "aula-f87-pro-wireless".into(),
                registry_model_id: None,
                battery: None,
            })
            .collect();
        let resolver = AssetResolver::new();
        let state = cx.new(|_| {
            let mut state = AppState::new(Sources {
                standalone: &keyboards,
                ..Sources::in_memory(
                    openhid_core::config::Config::ephemeral(),
                    &resolver,
                    commands,
                )
            });
            let _ = state.set_agent_link(AgentLink::Ready(AgentStatus {
                accessibility_granted: true,
                hook_installed: true,
                launch_at_login: false,
                inventory: InventoryHealth::Ready,
                protocol_version: PROTOCOL_VERSION,
                agent_version: "aula-ui-test-mock".into(),
                input_monitoring_granted: true,
                hid_open_failures: false,
            }));
            state
        });
        AppState::set_global(state, cx);
        cx.new(|cx| AulaLightingPanel::new(window, cx))
    });
    (panel, received)
}

fn next_read(
    received: &mut mpsc::UnboundedReceiver<Command>,
) -> crate::services::ipc::ReadAulaSettings {
    loop {
        match received
            .try_recv()
            .expect("a fresh configuration read was requested")
        {
            Command::ReadAulaSettings(read) => return read,
            Command::ReloadConfig(_) => {}
            _ => panic!("unexpected IPC request"),
        }
    }
}

#[gpui::test]
fn aula_reselecting_the_same_route_discards_the_previous_read(cx: &mut TestAppContext) {
    let (panel, mut commands) = setup(cx);
    let original = next_read(&mut commands);
    cx.update(|cx| AppState::apply(cx, |state| state.select_device(1)));
    cx.update(|cx| AppState::apply(cx, |state| state.select_device(0)));
    original
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 12,
                brightness: Some(3),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            panel.read(cx).editor.current.is_none(),
            "a reply from before reselection must not repopulate the editor"
        );
        assert!(
            panel.read(cx).pending,
            "reselection must obtain a fresh device snapshot"
        );
    });
    let fresh = next_read(&mut commands);
    assert_eq!(fresh.route, original.route);
    fresh
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 1,
                brightness: Some(1),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(panel.read(cx).editor.current.unwrap().effect, 1));
}

#[gpui::test]
fn aula_failed_writes_require_refresh_and_pending_requests_block_duplicates(
    cx: &mut TestAppContext,
) {
    let (panel, mut commands) = setup(cx);
    next_read(&mut commands)
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 12,
                brightness: Some(3),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            panel.write(LightingMode::Save, cx);
            panel.write(LightingMode::Save, cx);
            panel.read(cx);
        });
    });
    let Command::SetAulaSettings(write) = commands.try_recv().unwrap() else {
        panic!("expected a save request")
    };
    assert_eq!(write.mode, LightingMode::Save);
    assert!(
        commands.try_recv().is_err(),
        "a pending transaction must exclude duplicate reads and writes"
    );
    write.reply.send(Err(WriteError::AgentUnavailable)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            assert!(panel.editor.current.is_none());
            assert_eq!(panel.editor.error, Some(WriteError::AgentUnavailable));
            panel.write(LightingMode::Apply, cx);
        });
    });
    assert!(
        commands.try_recv().is_err(),
        "an unverified editor must not send another write"
    );
    cx.update(|cx| panel.update(cx, AulaLightingPanel::read));
    next_read(&mut commands)
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 12,
                brightness: Some(3),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            assert!(panel.editor.error.is_none());
            panel.write(LightingMode::Apply, cx);
        });
    });
    assert!(matches!(
        commands.try_recv(),
        Ok(Command::SetAulaSettings(_))
    ));
}

#[gpui::test]
fn aula_save_button_uses_verified_agent_state_and_disables_while_busy(cx: &mut TestAppContext) {
    use crate::services::i18n::LOCALE_LOCK;
    use gpui::{Modifiers, px, size};
    let _locale = LOCALE_LOCK.lock().unwrap();
    rust_i18n::set_locale("en");
    let (panel, mut commands) = setup(cx);
    next_read(&mut commands)
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 12,
                brightness: Some(3),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    cx.run_until_parked();
    let hosted = panel.clone();
    let (_, visual) =
        cx.add_window_view(|window, cx| gpui_component::Root::new(hosted, window, cx));
    visual.simulate_resize(size(px(900.), px(700.)));
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let save = visual
        .debug_bounds("aula-save")
        .expect("the lighting panel renders Save");
    visual.simulate_click(save.center(), Modifiers::default());
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let Command::SetAulaSettings(request) = commands.try_recv().unwrap() else {
        panic!("Save must route through IPC")
    };
    assert_eq!(request.mode, LightingMode::Save);
    assert_eq!(request.change.lighting().unwrap().effect(), Some(12));
    visual.simulate_click(save.center(), Modifiers::default());
    assert!(commands.try_recv().is_err(), "busy Save must be disabled");
    request
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 1,
                brightness: Some(2),
                speed: Some(4),
                color_mode: None,
            },
            SleepTimeout::from_seconds(300),
        )))
        .unwrap();
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(
            panel.read(cx).editor.current.unwrap().effect,
            1,
            "the displayed state must follow verified readback"
        );
        assert_eq!(panel.read(cx).editor.completed, Some(LightingMode::Save));
    });
}

#[test]
fn sleep_only_edit_preserves_custom_lighting_and_accepts_unknown_current_timeout() {
    let mut editor = Editor::default();
    editor.received_settings(
        AulaState::new(
            LightingState {
                effect: 21,
                brightness: None,
                speed: None,
                color_mode: None,
            },
            None,
        ),
        false,
    );
    assert!(editor.settings_change().is_none());
    assert_eq!(editor.completed, None, "a successful read is not a save");
    editor.sleep = SleepTimeout::from_seconds(600);
    let change = editor.settings_change().unwrap();
    assert_eq!(change.lighting(), None);
    assert_eq!(change.sleep_timeout().unwrap().seconds(), 600);
}

#[gpui::test]
fn aula_sleep_edit_uses_one_snapshot_and_blocks_concurrent_lighting_writes(
    cx: &mut TestAppContext,
) {
    let (panel, mut commands) = setup(cx);
    next_read(&mut commands)
        .reply
        .send(Ok(AulaState::new(
            LightingState {
                effect: 21,
                brightness: None,
                speed: None,
                color_mode: None,
            },
            None,
        )))
        .unwrap();
    cx.run_until_parked();
    assert!(
        commands.try_recv().is_err(),
        "only one panel owns the initial read"
    );
    cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            assert_eq!(panel.editor.completed, None);
            panel.editor.sleep = SleepTimeout::from_seconds(600);
            panel.write(LightingMode::Save, cx);
            panel.read(cx);
            panel.write(LightingMode::Apply, cx);
        });
    });
    let Command::SetAulaSettings(write) = commands.try_recv().unwrap() else {
        panic!("expected settings save")
    };
    assert_eq!(write.change.lighting(), None);
    assert_eq!(write.change.sleep_timeout().unwrap().seconds(), 600);
    assert!(commands.try_recv().is_err());
    write.reply.send(Err(WriteError::AgentUnavailable)).unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        panel.update(cx, |panel, cx| {
            assert!(panel.editor.current.is_none());
            panel.write(LightingMode::Save, cx);
        });
    });
    assert!(
        commands.try_recv().is_err(),
        "failure requires a fresh read"
    );
}
