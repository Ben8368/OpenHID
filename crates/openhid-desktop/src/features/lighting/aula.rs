//! F87 PRO built-in lighting, using agent-owned snapshot-preserving transactions.

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, PathPromptOptions, Render, Role, SharedString, StatefulInteractiveElement as _,
    Styled, Subscription, Toggled, WeakEntity, Window, div, prelude::FluentBuilder as _, px, rgb,
};
use gpui_base::Button as BaseButton;
use gpui_component::{
    Colorize as _, Disableable as _, IconName, Selectable as _, WindowExt as _,
    button::{Button, ButtonGroup, ButtonVariant, ButtonVariants as _},
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    dialog::DialogButtonProps,
    h_flex,
    input::InputState,
    menu::{DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement as _,
    v_flex,
};
use openhid_core::hid::aula::{
    AULA_BASE_KEY_COUNT, AULA_F87_KEY_POSITIONS, AULA_FN_KEY_COUNT,
    AULA_OEM_KEY_COMBINATION_ACTION, AULA_OEM_MACRO_TREE_ROOT_NODE_ID, AULA_PER_KEY_LED_COUNT,
    AULA_WIRED_KEYMAP_SLOT_COUNT, AulaBattery, AulaBatteryPowerSource, AulaChange,
    AulaColorModeChange, AulaColorModeState, AulaDebounce, AulaEffectCapabilities, AulaKeymap,
    AulaKeymapActionOutput, AulaOemFnActions, AulaOemFnLayer, AulaOemKeyCombination,
    AulaOemMacroEvent, AulaOemMacroEventKind, AulaOemMacroFile, AulaOemMacroProfile,
    AulaOemMacroTree, AulaOemMacroTreeNode, AulaOemMacroTreeProfile, AulaPerKeyRgb,
    AulaPerKeyRgbProfile, AulaRgb, AulaSideLightEffect, AulaState, AulaWiredConfigBlock,
    AulaWiredKeymapBlob, AulaWiredKeymapLayer, LightingChange, LightingMode, LightingState,
    SleepTimeout, aula_effect_capabilities, aula_f87_oem_default_fn_actions,
    aula_f87_oem_default_fn_layer, aula_keymap_action_label, aula_keymap_action_output,
    aula_keymap_action_outputs, aula_keymap_slot_is_assignable, aula_oem_macro_mapped_virtual_keys,
    aula_wired_keymap_action_outputs, aula_wired_keymap_base_from_oem_profile, route_identity,
    wired_route_identity,
};
use openhid_core::hid::{DeviceRoute, WriteError};
use tokio::sync::oneshot;

use crate::services::aula_keymap_profiles::{self, SavedAulaKeymapProfile};
use crate::services::aula_macro_profiles::{self, SavedAulaMacroProfile};
use crate::services::aula_macro_tree_profiles::{self, SavedAulaMacroTreeProfile};
use crate::services::aula_profiles::{self, SavedAulaProfile};
use crate::state::{AppState, StateEvent};
use crate::ui::battery::BatteryIndicator;
use crate::ui::components::{control_button, control_input, save_cancel_footer};
use crate::ui::theme::{self, Typography as _};

const AULA_OEM_GAMING_KEY_LEGENDS: [&str; 9] =
    ["Esc", "W", "A", "S", "D", "Up", "Down", "Left", "Right"];
const AULA_OEM_GAMING_COLOR: AulaRgb = AulaRgb::new(255, 0, 0);

#[derive(Clone, PartialEq, Eq)]
struct Target {
    route: DeviceRoute,
    available: bool,
    wired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectedMacroSource {
    Profile(String),
    Tree(String),
    TreeNode { name: String, node_index: usize },
}

enum MacroProfileExport {
    Macro(AulaOemMacroProfile),
    Tree(AulaOemMacroTreeProfile),
}

#[derive(Default)]
struct Editor {
    current: Option<LightingState>,
    wired_config: Option<AulaWiredConfigBlock>,
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
    current_sleep: Option<SleepTimeout>,
    sleep: Option<SleepTimeout>,
    current_debounce: Option<AulaDebounce>,
    debounce: Option<AulaDebounce>,
    current_side_light_effect: Option<AulaSideLightEffect>,
    side_light_effect: Option<AulaSideLightEffect>,
    battery: Option<AulaBattery>,
    per_key_table: Option<AulaPerKeyRgb>,
    per_key_index: usize,
    per_key_selection: Vec<usize>,
    per_key_color: Option<AulaRgb>,
    keymap: Option<AulaKeymap>,
    keymap_draft: Option<AulaKeymap>,
    keymap_oem_fn_actions: Option<AulaOemFnActions>,
    keymap_fn_layer: bool,
    keymap_index: usize,
    keymap_action: Option<[u8; 4]>,
    wired_keymap: Option<AulaWiredKeymapBlob>,
    wired_keymap_action: Option<[u8; 4]>,
    wired_keymap_error: Option<String>,
    wired_keymap_status: Option<String>,
    color_mode: Option<AulaColorModeChange>,
    error: Option<WriteError>,
    completed: Option<LightingMode>,
}

#[derive(Clone)]
struct OemFnRecordTarget {
    view: WeakEntity<AulaLightingPanel>,
    profile_name: String,
    record_index: usize,
}

impl Editor {
    fn received_settings(&mut self, state: AulaState, preserve_battery: bool) {
        let battery = state.battery.or_else(|| {
            if preserve_battery {
                self.battery.clone()
            } else {
                None
            }
        });
        self.received(state.lighting);
        self.current_sleep = state.sleep_timeout;
        self.sleep = state.sleep_timeout;
        self.current_debounce = state.debounce;
        self.debounce = state.debounce;
        self.current_side_light_effect = state.side_light_effect;
        self.side_light_effect = state.side_light_effect;
        self.battery = battery;
        self.completed = None;
    }

    fn received_wired_config(&mut self, block: AulaWiredConfigBlock) {
        self.received_settings(block.aula_state(), false);
        self.wired_config = Some(block);
    }

    fn settings_change(&self) -> Option<AulaChange> {
        self.current?;
        let lighting = self.change().filter(|change| {
            change.color_mode().is_some()
                || self.current.is_some_and(|current| {
                    change.effect() != Some(current.effect)
                        || change
                            .brightness()
                            .is_some_and(|v| Some(v) != current.brightness)
                        || change.speed().is_some_and(|v| Some(v) != current.speed)
                })
        });
        let sleep = self
            .sleep
            .filter(|value| Some(*value) != self.current_sleep);
        let debounce = self
            .debounce
            .filter(|value| Some(*value) != self.current_debounce);
        let side_light_effect = self
            .side_light_effect
            .filter(|value| Some(*value) != self.current_side_light_effect);
        AulaChange::new_with_side_light(
            lighting.or_else(|| {
                (sleep.is_none() && debounce.is_none() && side_light_effect.is_none())
                    .then(|| self.change())
                    .flatten()
            }),
            sleep,
            debounce,
            side_light_effect,
        )
        .ok()
    }

    fn received(&mut self, state: LightingState) {
        self.current = Some(state);
        self.effect = LightingChange::supports_effect(state.effect).then_some(state.effect);
        self.brightness = state
            .brightness
            .filter(|value| LightingChange::levels().contains(value));
        self.speed = state
            .speed
            .filter(|value| LightingChange::levels().contains(value));
        self.color_mode = None;
        self.error = None;
    }

    fn select_effect(&mut self, effect: u8) {
        if self.effect != Some(effect) {
            self.color_mode = None;
        }
        self.effect = Some(effect);
        // A different effect has its own device-resident parameter table.
        // Leave it unchanged unless the user explicitly chooses new levels.
        if let Some(current) = self.current.filter(|state| state.effect == effect) {
            self.brightness = current
                .brightness
                .filter(|value| LightingChange::levels().contains(value));
            self.speed = current
                .speed
                .filter(|value| LightingChange::levels().contains(value));
        } else {
            self.brightness = None;
            self.speed = None;
        }
        self.completed = None;
    }

    fn change(&self) -> Option<LightingChange> {
        let effect = self.effect?;
        let change = LightingChange::new(
            Some(effect),
            (effect != 0).then_some(self.brightness).flatten(),
            (effect != 0).then_some(self.speed).flatten(),
        )
        .ok()?;
        match self.color_mode {
            Some(color_mode) => change.with_color_mode(color_mode).ok(),
            None => Some(change),
        }
    }

    fn selected_per_key_indices(&self) -> Vec<usize> {
        if self.per_key_selection.is_empty() {
            vec![self.per_key_index]
        } else {
            self.per_key_selection.clone()
        }
    }

    fn per_key_is_selected(&self, index: usize) -> bool {
        if self.per_key_selection.is_empty() {
            self.per_key_index == index
        } else {
            self.per_key_selection.contains(&index)
        }
    }

    fn select_per_key(&mut self, index: usize, extend: bool) {
        if !extend {
            self.per_key_selection.clear();
        }
        if !self.per_key_selection.contains(&index) {
            self.per_key_selection.push(index);
        }
        self.per_key_index = index;
        self.completed = None;
    }

    fn stage_keymap_action(&mut self) {
        let (Some(keymap), Some(action)) = (&self.keymap_draft, self.keymap_action) else {
            return;
        };
        let mut base = keymap.base().to_vec();
        let mut fn_layer = keymap.fn_layer().to_vec();
        let entries = if self.keymap_fn_layer {
            &mut fn_layer
        } else {
            &mut base
        };
        let Some(entry) = entries.get_mut(self.keymap_index) else {
            return;
        };
        *entry = action;
        if let Ok(draft) = AulaKeymap::new(base, fn_layer) {
            if self.keymap_fn_layer
                && let Some(actions) = &mut self.keymap_oem_fn_actions
            {
                let _ = actions.clear_matrix_slot(self.keymap_index);
            }
            self.keymap_draft = Some(draft);
            self.completed = None;
        }
    }

    fn restore_oem_fn_defaults(&mut self) {
        let Some(keymap) = self.keymap_draft.as_ref() else {
            return;
        };
        let base = keymap.base().to_vec();
        if let Ok(draft) = AulaKeymap::new(base, aula_f87_oem_default_fn_layer()) {
            self.keymap_draft = Some(draft);
            self.keymap_oem_fn_actions = Some(aula_f87_oem_default_fn_actions());
            self.keymap_fn_layer = true;
            self.keymap_action = None;
            self.completed = None;
        }
    }

    fn stage_wired_keymap_action(&mut self) {
        let (Some(blob), Some(action)) = (self.wired_keymap.as_mut(), self.wired_keymap_action)
        else {
            return;
        };
        match blob.replace_slot(self.keymap_index, action) {
            Ok(()) => {
                self.wired_keymap_error = None;
                self.wired_keymap_status = None;
            }
            Err(error) => self.wired_keymap_error = Some(error.to_string()),
        }
    }
}

/// The editor owns its draft and in-flight identity. Hardware and recovery live
/// in the agent; selecting another route invalidates late replies here.
pub struct AulaLightingPanel {
    target: Option<Target>,
    editor: Editor,
    pending: bool,
    selection_epoch: u64,
    saved_profiles: Vec<SavedAulaProfile>,
    selected_profile: Option<String>,
    profile_error: Option<String>,
    saved_keymap_profiles: Vec<SavedAulaKeymapProfile>,
    selected_keymap_profile: Option<String>,
    keymap_profile_error: Option<String>,
    saved_macro_profiles: Vec<SavedAulaMacroProfile>,
    saved_macro_tree_profiles: Vec<SavedAulaMacroTreeProfile>,
    selected_macro_source: Option<SelectedMacroSource>,
    selected_macro_event_index: usize,
    macro_profile_error: Option<String>,
    color_picker: Option<Entity<ColorPickerState>>,
    _color_picker_subscription: Option<Subscription>,
    _state_obs: Subscription,
}

impl AulaLightingPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let color_picker = cx.new(|cx| ColorPickerState::new(window, cx));
        let color_picker_subscription =
            cx.subscribe(&color_picker, |this, _, event: &ColorPickerEvent, cx| {
                let ColorPickerEvent::Change(Some(color)) = event else {
                    return;
                };
                let custom_color_supported = this
                    .editor
                    .effect
                    .and_then(aula_effect_capabilities)
                    .is_some_and(AulaEffectCapabilities::has_custom_color);
                if !custom_color_supported {
                    return;
                }
                if let Some(color) = aula_rgb_from_picker(*color) {
                    this.editor.color_mode = Some(AulaColorModeChange::SingleColor(color));
                    this.editor.completed = None;
                    cx.notify();
                }
            });
        let state_obs = AppState::on_device_panel(
            cx,
            |event| {
                matches!(
                    event,
                    StateEvent::AgentChanged | StateEvent::LanguageChanged
                )
            },
            |this: &mut Self, cx| {
                this.sync_target(cx);
                cx.notify();
            },
        );
        let mut this = Self {
            target: None,
            editor: Editor::default(),
            pending: false,
            selection_epoch: 0,
            saved_profiles: aula_profiles::load_all().unwrap_or_else(|error| {
                tracing::warn!(%error, "could not load local AULA RGB profiles");
                Vec::new()
            }),
            selected_profile: None,
            profile_error: None,
            saved_keymap_profiles: aula_keymap_profiles::load_all().unwrap_or_else(|error| {
                tracing::warn!(%error, "could not load local AULA keymap profiles");
                Vec::new()
            }),
            selected_keymap_profile: None,
            keymap_profile_error: None,
            saved_macro_profiles: aula_macro_profiles::load_all().unwrap_or_else(|error| {
                tracing::warn!(%error, "could not load local AULA macro profiles");
                Vec::new()
            }),
            saved_macro_tree_profiles: aula_macro_tree_profiles::load_all().unwrap_or_else(
                |error| {
                    tracing::warn!(%error, "could not load local AULA macro-tree profiles");
                    Vec::new()
                },
            ),
            selected_macro_source: None,
            selected_macro_event_index: 0,
            macro_profile_error: None,
            color_picker: Some(color_picker),
            _color_picker_subscription: Some(color_picker_subscription),
            _state_obs: state_obs,
        };
        this.sync_target(cx);
        this
    }

    fn sync_target(&mut self, cx: &mut Context<Self>) {
        let target = AppState::try_read(cx).and_then(|state| {
            let record = state.current_record()?;
            let route = record.route.as_ref()?;
            let wired = if route_identity(route).is_some() {
                false
            } else if wired_route_identity(route).is_some() {
                true
            } else {
                return None;
            };
            Some(Target {
                route: route.clone(),
                available: record.online && state.agent_status().is_some(),
                wired,
            })
        });
        if self.target != target {
            self.selection_epoch = self.selection_epoch.wrapping_add(1);
            self.target = target;
            self.editor = Editor::default();
            if !self.pending {
                self.read(cx);
            }
            cx.notify();
        }
    }

    fn available(&self) -> bool {
        self.target.as_ref().is_some_and(|target| target.available) && !self.pending
    }

    fn read(&mut self, cx: &mut Context<Self>) {
        if !self.available() {
            return;
        }
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        if target.wired {
            let reply = state.read_aula_wired_config(target.route.clone());
            self.wait_wired(target, None, reply, cx);
        } else {
            let reply = state.read_aula_settings(target.route.clone());
            self.wait(target, None, reply, cx);
        }
    }

    fn write(&mut self, mode: LightingMode, cx: &mut Context<Self>) {
        if !self.available() || self.editor.current.is_none() {
            return;
        }
        let Some(change) = self.editor.settings_change() else {
            return;
        };
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        if target.wired {
            let Some(mut block) = self.editor.wired_config.clone() else {
                return;
            };
            if let Err(error) = block.apply_change(change) {
                self.editor.error = Some(WriteError::LightUnsupported {
                    control: error.to_string(),
                });
                cx.notify();
                return;
            }
            let reply = state.set_aula_wired_config(target.route.clone(), block);
            self.wait_wired(target, Some(mode), reply, cx);
        } else {
            let reply = state.set_aula_settings(target.route.clone(), change, mode);
            self.wait(target, Some(mode), reply, cx);
        }
    }

    fn apply_per_key_rgb(&mut self, cx: &mut Context<Self>) {
        if !self.available() || self.editor.current.is_none_or(|state| state.effect == 21) {
            return;
        }
        let Some(table) = self.editor.per_key_table.clone() else {
            return;
        };
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        let reply = state.set_aula_per_key_rgb(target.route.clone(), table);
        self.wait(target, Some(LightingMode::Save), reply, cx);
    }

    fn read_keymap(&mut self, cx: &mut Context<Self>) {
        if !self.available() {
            return;
        }
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        let reply = state.read_aula_keymap(target.route.clone());
        self.wait_keymap(target, reply, cx);
    }

    fn write_keymap(&mut self, cx: &mut Context<Self>) {
        if !self.available() {
            return;
        }
        let Some(keymap) = self.editor.keymap_draft.clone() else {
            return;
        };
        let Some(expected) = self.editor.keymap.clone() else {
            return;
        };
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        let reply = state.set_aula_keymap_if_unchanged(target.route.clone(), expected, keymap);
        self.wait_keymap(target, reply, cx);
    }

    fn confirm_wired_keymap_write(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.available()
            || !self.target.as_ref().is_some_and(|target| target.wired)
            || self.editor.wired_keymap.is_none()
        {
            return;
        }
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(tr!("aula.wired_keymap_confirm_title"))
                .description(tr!("aula.wired_keymap_confirm_hint"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.wired_keymap_confirm_write"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    move |_, _, cx| {
                        let _ = view.update(cx, AulaLightingPanel::submit_wired_keymap);
                        true
                    }
                })
        });
    }

    fn submit_wired_keymap(&mut self, cx: &mut Context<Self>) {
        if !self.available() {
            return;
        }
        let (Some(target), Some(blob)) = (self.target.clone(), self.editor.wired_keymap.clone())
        else {
            return;
        };
        if !target.wired {
            return;
        }
        let Some(state) = AppState::try_read(cx) else {
            return;
        };
        let reply = state.set_aula_wired_keymap(target.route.clone(), blob);
        let selection_epoch = self.selection_epoch;
        self.pending = true;
        self.editor.wired_keymap_error = None;
        self.editor.wired_keymap_status = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = reply.await.unwrap_or(Err(WriteError::AgentUnavailable));
            let _ = view.update(cx, |this, cx| {
                this.pending = false;
                if this.selection_epoch == selection_epoch && this.target.as_ref() == Some(&target)
                {
                    match result {
                        Ok(()) => {
                            this.editor.wired_keymap_status =
                                Some(tr!("aula.wired_keymap_write_accepted").to_string());
                            this.editor.wired_keymap_error = None;
                        }
                        Err(error) => {
                            this.editor.wired_keymap_error = Some(error.to_string());
                            this.editor.wired_keymap_status = None;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn wait_keymap(
        &mut self,
        target: Target,
        reply: oneshot::Receiver<Result<AulaKeymap, WriteError>>,
        cx: &mut Context<Self>,
    ) {
        let selection_epoch = self.selection_epoch;
        self.pending = true;
        self.editor.error = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = reply.await.unwrap_or(Err(WriteError::AgentUnavailable));
            let _ = view.update(cx, |this, cx| {
                this.pending = false;
                if this.selection_epoch != selection_epoch || this.target.as_ref() != Some(&target)
                {
                    this.read_keymap(cx);
                } else {
                    match result {
                        Ok(keymap) => {
                            this.editor.keymap = Some(keymap.clone());
                            this.editor.keymap_draft = Some(keymap);
                            this.editor.keymap_oem_fn_actions = None;
                            this.editor.error = None;
                        }
                        Err(error) => {
                            this.editor.keymap = None;
                            this.editor.keymap_oem_fn_actions = None;
                            this.editor.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn stage_per_key_color(&mut self, cx: &mut Context<Self>) {
        if !self.available() || self.editor.current.is_none_or(|state| state.effect == 21) {
            return;
        }
        let Some(color) = self.editor.per_key_color else {
            return;
        };
        let mut table = self
            .editor
            .per_key_table
            .take()
            .unwrap_or_else(|| AulaPerKeyRgb::solid(AulaRgb::new(0, 0, 0)));
        for index in self.editor.selected_per_key_indices() {
            if let Some(updated) = table.clone().with_color(index, color) {
                table = updated;
            }
        }
        self.editor.per_key_table = Some(table);
        self.editor.completed = None;
        cx.notify();
    }

    fn select_oem_gaming_keys(&mut self, cx: &mut Context<Self>) {
        if !self.available() || self.editor.current.is_some_and(|state| state.effect == 21) {
            return;
        }
        let selected = AULA_F87_KEY_POSITIONS
            .iter()
            .filter(|position| AULA_OEM_GAMING_KEY_LEGENDS.contains(&position.label()))
            .map(|position| usize::from(position.led_index()))
            .collect::<Vec<_>>();
        let Some(first) = selected.first().copied() else {
            return;
        };
        self.editor.per_key_index = first;
        self.editor.per_key_selection = selected;
        self.editor.per_key_color = Some(AULA_OEM_GAMING_COLOR);
        self.stage_per_key_color(cx);
    }

    fn wait(
        &mut self,
        target: Target,
        mode: Option<LightingMode>,
        reply: oneshot::Receiver<Result<AulaState, WriteError>>,
        cx: &mut Context<Self>,
    ) {
        let selection_epoch = self.selection_epoch;
        self.pending = true;
        self.editor.error = None;
        self.editor.completed = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = reply.await.unwrap_or(Err(WriteError::AgentUnavailable));
            let _ = view.update(cx, |this, cx| {
                this.pending = false;
                if this.selection_epoch != selection_epoch || this.target.as_ref() != Some(&target)
                {
                    this.read(cx);
                } else {
                    match result {
                        Ok(state) => {
                            this.editor.received_settings(state, mode.is_some());
                            this.editor.completed = mode;
                        }
                        Err(error) => {
                            // A failed write may have required recovery. Require a
                            // fresh successful read before another write.
                            this.editor.current = None;
                            this.editor.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn wait_wired(
        &mut self,
        target: Target,
        mode: Option<LightingMode>,
        reply: oneshot::Receiver<Result<AulaWiredConfigBlock, WriteError>>,
        cx: &mut Context<Self>,
    ) {
        let selection_epoch = self.selection_epoch;
        self.pending = true;
        self.editor.error = None;
        self.editor.completed = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = reply.await.unwrap_or(Err(WriteError::AgentUnavailable));
            let _ = view.update(cx, |this, cx| {
                this.pending = false;
                if this.selection_epoch != selection_epoch || this.target.as_ref() != Some(&target)
                {
                    this.read(cx);
                } else {
                    match result {
                        Ok(block) => {
                            this.editor.received_wired_config(block);
                            this.editor.completed = mode;
                        }
                        Err(error) => {
                            this.editor.current = None;
                            this.editor.wired_config = None;
                            this.editor.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn transaction_controls(
        &self,
        writable: bool,
        wired: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(
                control_button("aula-refresh")
                    .label(tr!("aula.refresh"))
                    .disabled(!self.available())
                    .on_click(cx.listener(|this, _, _, cx| this.read(cx))),
            )
            .when(!wired, |controls| {
                controls.child(
                    control_button("aula-apply")
                        .label(tr!("aula.apply"))
                        .disabled(!writable)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.write(LightingMode::Apply, cx)),
                        ),
                )
            })
            .child(
                control_button("aula-save")
                    .debug_selector(|| "aula-save".into())
                    .primary()
                    .label(tr!("aula.save"))
                    .disabled(!writable)
                    .on_click(cx.listener(|this, _, _, cx| this.write(LightingMode::Save, cx))),
            )
    }

    fn status(&self) -> SharedString {
        if self.pending {
            tr!("aula.working")
        } else if !self.target.as_ref().is_some_and(|target| target.available) {
            tr!("aula.unavailable")
        } else if self.editor.error.is_some() {
            tr!("aula.failed")
        } else if self.editor.completed == Some(LightingMode::Save) {
            tr!("aula.saved")
        } else if self.editor.completed == Some(LightingMode::Apply) {
            tr!("aula.applied")
        } else {
            tr!("aula.ready")
        }
    }

    fn color_controls(
        &self,
        editable: bool,
        capabilities: AulaEffectCapabilities,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let has_custom_color = capabilities.has_custom_color();
        let has_random_color = capabilities.has_random_color();
        let selected_mode = self
            .editor
            .color_mode
            .map(|change| match change {
                AulaColorModeChange::SingleColor(_) => AulaColorModeState::SingleColor,
                AulaColorModeChange::Colorful => AulaColorModeState::Colorful,
            })
            .or_else(|| {
                self.editor
                    .current
                    .filter(|current| self.editor.effect == Some(current.effect))
                    .and_then(|current| current.color_mode)
            });
        let mut controls = h_flex().gap_2().flex_wrap();
        if has_custom_color {
            if editable {
                if let Some(color_picker) = &self.color_picker {
                    controls = controls
                        .child(ColorPicker::new(color_picker).label(tr!("aula.single_color")));
                }
            } else {
                controls = controls.child(
                    control_button("aula-single-color")
                        .label(tr!("aula.single_color"))
                        .disabled(true),
                );
            }
        }
        if has_random_color {
            controls = controls.child(
                control_button("aula-colorful")
                    .label(tr!("aula.colorful"))
                    .selected(selected_mode == Some(AulaColorModeState::Colorful))
                    .disabled(!editable)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let supports_random = this
                            .editor
                            .effect
                            .and_then(aula_effect_capabilities)
                            .is_some_and(AulaEffectCapabilities::has_random_color);
                        if this.available() && supports_random {
                            this.editor.color_mode = Some(AulaColorModeChange::Colorful);
                            this.editor.completed = None;
                            cx.notify();
                        }
                    })),
            );
        }
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.color_mode")))
            .child(controls)
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(color_mode_status(self.editor.color_mode, selected_mode)),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.color_hint")),
            )
    }

    fn sleep_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.editor.sleep;
        let view = cx.entity().downgrade();
        control_button("aula-sleep-timeout")
            .label(selected.map_or_else(
                || tr!("aula.choose_sleep"),
                |value| tr!("aula.sleep_seconds", seconds = value.seconds()),
            ))
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for timeout in SleepTimeout::options() {
                    let value = *timeout;
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr!("aula.sleep_seconds", seconds = value.seconds()))
                            .checked(selected == Some(value))
                            .on_click(move |_, _, cx| {
                                let _ = item_view.update(cx, |this, cx| {
                                    if this.available() && this.editor.current.is_some() {
                                        this.editor.sleep = Some(value);
                                        this.editor.completed = None;
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn debounce_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.editor.debounce;
        let view = cx.entity().downgrade();
        control_button("aula-debounce")
            .label(selected.map_or_else(
                || tr!("aula.choose_debounce"),
                |value| tr!("aula.debounce_ms", milliseconds = value.milliseconds()),
            ))
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for debounce in AulaDebounce::options() {
                    let value = *debounce;
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr!(
                            "aula.debounce_ms",
                            milliseconds = value.milliseconds()
                        ))
                        .checked(selected == Some(value))
                        .on_click(move |_, _, cx| {
                            let _ = item_view.update(cx, |this, cx| {
                                if this.available() && this.editor.current.is_some() {
                                    this.editor.debounce = Some(value);
                                    this.editor.completed = None;
                                    cx.notify();
                                }
                            });
                        }),
                    );
                }
                menu
            })
    }

    fn side_light_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.editor.side_light_effect;
        let view = cx.entity().downgrade();
        control_button("aula-side-light-effect")
            .label(selected.map_or_else(
                || tr!("aula.choose_side_light_effect"),
                side_light_effect_label,
            ))
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for effect in AulaSideLightEffect::options() {
                    let value = *effect;
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(side_light_effect_label(value))
                            .checked(selected == Some(value))
                            .on_click(move |_, _, cx| {
                                let _ = item_view.update(cx, |this, cx| {
                                    if this.available() && this.editor.current.is_some() {
                                        this.editor.side_light_effect = Some(value);
                                        this.editor.completed = None;
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn side_light_section(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.side_light_effect")))
            .child(self.side_light_control(enabled, cx))
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.side_light_hint")),
            )
    }

    fn per_key_controls(&self, editable: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let has_selected_color = self.editor.per_key_color.is_some();
        let can_apply = editable
            && self.editor.per_key_table.is_some()
            && self.editor.current.is_some_and(|state| state.effect != 21);
        let index = self.editor.per_key_index;
        let selected_count = self.editor.selected_per_key_indices().len();
        let shown_color = self
            .editor
            .per_key_table
            .as_ref()
            .and_then(|table| table.color(index))
            .unwrap_or(AulaRgb::new(52, 55, 62));
        let key_label = AULA_F87_KEY_POSITIONS
            .iter()
            .find(|key| usize::from(key.led_index()) == index)
            .map(|key| key.label());
        let mut keys = Vec::with_capacity(AULA_F87_KEY_POSITIONS.len());
        for position in AULA_F87_KEY_POSITIONS {
            let led_index = usize::from(position.led_index());
            let color = self
                .editor
                .per_key_table
                .as_ref()
                .and_then(|table| table.color(led_index))
                .unwrap_or(AulaRgb::new(52, 55, 62));
            keys.push(Self::per_key_keycap(
                position,
                color,
                self.editor.per_key_is_selected(led_index),
                editable,
                cx,
            ));
        }
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.per_key_heading")))
            .child(self.per_key_profile_controls(editable, cx))
            .child(
                h_flex()
                    .w_full()
                    .overflow_x_scrollbar()
                    .id("aula-rgb-keyboard-scroll")
                    .child(
                        div()
                            .flex_none()
                            .relative()
                            .w(px(AULA_LAYOUT_WIDTH))
                            .h(px(AULA_LAYOUT_HEIGHT))
                            .children(keys),
                    ),
            )
            .child(div().text_caption().child(if selected_count > 1 {
                tr!("aula.per_key_selected_count", count = selected_count)
            } else {
                tr!(
                    "aula.per_key_selected",
                    key = key_label.unwrap_or("?"),
                    index = index
                )
            }))
            .child(self.oem_gaming_key_control(editable, cx))
            .child(self.unmapped_led_control(editable, key_label, cx))
            .child(
                div()
                    .text_caption()
                    .child(tr!("aula.per_key_current", color = rgb_label(shown_color))),
            )
            .child(self.per_key_color_control(editable, cx))
            .child(
                control_button("aula-per-key-stage")
                    .label(tr!("aula.per_key_stage"))
                    .disabled(!editable || !has_selected_color)
                    .on_click(cx.listener(|this, _, _, cx| this.stage_per_key_color(cx))),
            )
            .child(
                control_button("aula-per-key-apply")
                    .primary()
                    .label(tr!("aula.per_key_apply"))
                    .disabled(!can_apply)
                    .on_click(cx.listener(|this, _, _, cx| this.apply_per_key_rgb(cx))),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.per_key_hint")),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.per_key_multi_select_hint")),
            )
            .children(self.profile_error.as_ref().map(|error| {
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(error.clone())
            }))
    }

    fn oem_gaming_key_control(&self, editable: bool, cx: &mut Context<Self>) -> impl IntoElement {
        control_button("aula-per-key-stage-game-keys")
            .label(tr!("aula.per_key_game_keys"))
            .disabled(!editable || self.editor.current.is_some_and(|state| state.effect == 21))
            .on_click(cx.listener(|this, _, _, cx| this.select_oem_gaming_keys(cx)))
    }

    fn keymap_profile_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_keymap_profile.clone();
        let selected_for_menu = selected.clone();
        let profiles = self.saved_keymap_profiles.clone();
        let view = cx.entity().downgrade();
        let profile_menu = control_button("aula-keymap-profile")
            .label(
                selected
                    .clone()
                    .unwrap_or_else(|| tr!("aula.keymap_choose_profile").to_string()),
            )
            .icon(IconName::ChevronDown)
            .disabled(profiles.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for saved in &profiles {
                    let name = saved.name.clone();
                    let keymap = saved.keymap.clone();
                    let oem_fn_actions = saved.oem_fn_actions.clone();
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(selected_for_menu.as_deref() == Some(name.as_str()))
                            .on_click(move |_, _, cx| {
                                let _ = item_view.update(cx, |this, cx| {
                                    this.editor.keymap_draft = Some(keymap.clone());
                                    this.editor
                                        .keymap_oem_fn_actions
                                        .clone_from(&oem_fn_actions);
                                    this.editor.keymap_action = None;
                                    this.selected_keymap_profile = Some(name.clone());
                                    this.keymap_profile_error = None;
                                    this.editor.completed = None;
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(profile_menu)
            .child(
                control_button("aula-keymap-profile-import")
                    .label(tr!("aula.keymap_import_profile"))
                    .on_click(cx.listener(|_, _, _, cx| Self::import_keymap_profile(cx))),
            )
            .child(
                control_button("aula-keymap-profile-export")
                    .label(tr!("aula.keymap_export_profile"))
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.export_keymap_profile(cx))),
            )
            .child(
                control_button("aula-keymap-oem-profile-export")
                    .label(tr!("aula.keymap_export_oem_profile"))
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.export_oem_keymap_profile(cx))),
            )
            .child(
                control_button("aula-keymap-profile-save")
                    .label(tr!("aula.keymap_save_profile"))
                    .disabled(self.editor.keymap_draft.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_save_keymap_profile(window, cx);
                    })),
            )
            .child(
                control_button("aula-keymap-profile-delete")
                    .label(tr!("aula.keymap_delete_profile"))
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_delete_keymap_profile(window, cx);
                    })),
            )
    }

    fn selected_oem_fn_record(&self) -> Option<(usize, [u8; 12])> {
        if !self.editor.keymap_fn_layer {
            return None;
        }
        let profile_name = self.selected_keymap_profile.as_deref()?;
        let record_index = AULA_F87_KEY_POSITIONS
            .iter()
            .position(|position| usize::from(position.matrix_slot()) == self.editor.keymap_index)?;
        let records = self
            .saved_keymap_profiles
            .iter()
            .find(|profile| profile.name == profile_name)?
            .oem_fn_records
            .as_ref()?;
        Some((record_index, records.record(record_index)?))
    }

    fn open_oem_fn_record_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(profile_name) = self.selected_keymap_profile.clone() else {
            return;
        };
        let Some((record_index, record)) = self.selected_oem_fn_record() else {
            return;
        };
        let combination = self.selected_oem_fn_combination(record_index);
        let view = cx.entity().downgrade();
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("000000000000000000000000");
            input.set_value(format_hex_bytes(&record), window, cx);
            input
        });
        let combination_inputs: Option<[Entity<InputState>; 3]> =
            combination.map(|combination| oem_fn_combo_inputs(combination, window, cx));
        let dialog_error = cx.new(|_| Option::<String>::None);
        let target = OemFnRecordTarget {
            view,
            profile_name,
            record_index,
        };
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(520.))
                .title(tr!("aula.keymap_oem_fields"))
                .child(oem_fn_record_editor_content(
                    &input,
                    combination_inputs.as_ref(),
                    dialog_error.read(cx).clone(),
                    cx,
                ))
                .footer(save_cancel_footer())
                .on_ok({
                    let target = target.clone();
                    let input = input.clone();
                    let dialog_error = dialog_error.clone();
                    let combination_inputs = combination_inputs.clone();
                    move |_, window, cx| {
                        save_oem_fn_profile_record(
                            &target,
                            &input,
                            combination_inputs.as_ref(),
                            &dialog_error,
                            window,
                            cx,
                        )
                    }
                })
        });
    }

    fn selected_oem_fn_combination(&self, record_index: usize) -> Option<AulaOemKeyCombination> {
        let action = self
            .editor
            .keymap_draft
            .as_ref()
            .and_then(|keymap| keymap.fn_layer().get(self.editor.keymap_index))?;
        if *action != AULA_OEM_KEY_COMBINATION_ACTION {
            return None;
        }
        let profile_name = self.selected_keymap_profile.as_deref()?;
        self.saved_keymap_profiles
            .iter()
            .find(|profile| profile.name == profile_name)?
            .oem_fn_records
            .as_ref()?
            .key_combination(record_index)
    }

    fn macro_profile_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_macro_source.clone();
        let export_label = if matches!(
            selected.as_ref(),
            Some(SelectedMacroSource::Tree(_) | SelectedMacroSource::TreeNode { .. })
        ) {
            tr!("aula.macro_tree_export")
        } else {
            tr!("aula.macro_export")
        };
        let (selected_count, selected_event) = self.macro_event_selection(selected.as_ref());
        let can_rename_tree_node = matches!(selected, Some(SelectedMacroSource::TreeNode { .. }));
        let can_delete_tree_node = can_rename_tree_node;
        let can_create_tree_node = matches!(
            selected,
            Some(SelectedMacroSource::Tree(_) | SelectedMacroSource::TreeNode { .. })
        );
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.macro_heading")))
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.macro_hint")),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(self.macro_profile_menu(selected.as_ref(), cx))
                    .child(
                        control_button("aula-macro-tree-new-profile")
                            .label(tr!("aula.macro_tree_new_profile"))
                            .on_click(cx.listener(|_, _, window, cx| {
                                Self::open_create_macro_tree_profile(window, cx);
                            })),
                    )
                    .child(
                        control_button("aula-macro-tree-rename-node")
                            .label(tr!("aula.macro_tree_rename_node"))
                            .disabled(!can_rename_tree_node)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_rename_macro_tree_node(window, cx);
                            })),
                    )
                    .child(
                        control_button("aula-macro-tree-create-folder")
                            .label(tr!("aula.macro_tree_create_folder"))
                            .disabled(!can_create_tree_node)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_macro_tree_folder(window, cx);
                            })),
                    )
                    .child(
                        control_button("aula-macro-tree-create-macro")
                            .label(tr!("aula.macro_tree_create_macro"))
                            .disabled(!can_create_tree_node)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_create_macro_tree_macro(window, cx);
                            })),
                    )
                    .child(
                        control_button("aula-macro-tree-delete-node")
                            .label(tr!("aula.macro_tree_delete_node"))
                            .disabled(!can_delete_tree_node)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_delete_macro_tree_node(window, cx);
                            })),
                    )
                    .child(
                        control_button("aula-macro-import")
                            .label(tr!("aula.macro_import"))
                            .on_click(cx.listener(|_, _, _, cx| Self::import_macro_profile(cx))),
                    )
                    .child(
                        control_button("aula-macro-export")
                            .label(export_label)
                            .disabled(selected.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.export_macro_profile(cx))),
                    )
                    .child(
                        control_button("aula-macro-delete")
                            .label(tr!("aula.macro_delete"))
                            .disabled(selected.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_delete_macro_profile(window, cx);
                            })),
                    ),
            )
            .child(self.macro_event_controls(selected_count, selected_event, cx))
            .children(self.macro_profile_error.as_ref().map(|error| {
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(error.clone())
            }))
    }

    fn macro_profile_menu(
        &self,
        selected: Option<&SelectedMacroSource>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected_for_menu = selected.cloned();
        let profiles = self.saved_macro_profiles.clone();
        let trees = self.saved_macro_tree_profiles.clone();
        let view = cx.entity().downgrade();
        control_button("aula-macro-profile")
            .label(selected.map_or_else(
                || tr!("aula.macro_choose_profile").to_string(),
                |source| self.macro_source_label(source),
            ))
            .icon(IconName::ChevronDown)
            .disabled(profiles.is_empty() && trees.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for saved in &profiles {
                    let name = saved.name.clone();
                    let count = saved.profile.macro_file().oem_editor_event_count();
                    let source = SelectedMacroSource::Profile(name.clone());
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{name} · {count}"))
                            .checked(selected_for_menu.as_ref() == Some(&source))
                            .on_click(move |_, _, cx| {
                                let _ = item_view.update(cx, |this, cx| {
                                    this.select_macro_source(source.clone());
                                    cx.notify();
                                });
                            }),
                    );
                }
                for saved in &trees {
                    let name = saved.name.clone();
                    let source = SelectedMacroSource::Tree(name.clone());
                    let item_view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(selected_for_menu.as_ref() == Some(&source))
                            .on_click(move |_, _, cx| {
                                let _ = item_view.update(cx, |this, cx| {
                                    this.select_macro_source(source.clone());
                                    cx.notify();
                                });
                            }),
                    );
                    let tree = saved.profile.tree();
                    for (node_index, node) in tree.nodes().iter().enumerate() {
                        let source = SelectedMacroSource::TreeNode {
                            name: name.clone(),
                            node_index,
                        };
                        let path_label = macro_tree_node_path_label(tree, node_index);
                        let node_name = format!("{name} · {path_label} · #{}", node_index + 1);
                        let count = node
                            .macro_file()
                            .map(|macro_file| format!(" · {}", macro_file.oem_editor_event_count()))
                            .unwrap_or_default();
                        let item_view = view.clone();
                        menu = menu.item(
                            PopupMenuItem::new(format!("{node_name}{count}"))
                                .checked(selected_for_menu.as_ref() == Some(&source))
                                .on_click(move |_, _, cx| {
                                    let _ = item_view.update(cx, |this, cx| {
                                        this.select_macro_source(source.clone());
                                        cx.notify();
                                    });
                                }),
                        );
                    }
                }
                menu
            })
    }

    fn macro_file_for_source(&self, source: &SelectedMacroSource) -> Option<AulaOemMacroFile> {
        match source {
            SelectedMacroSource::Profile(name) => self
                .saved_macro_profiles
                .iter()
                .find(|saved| saved.name == *name)
                .map(|saved| saved.profile.macro_file().clone()),
            SelectedMacroSource::Tree(_) => None,
            SelectedMacroSource::TreeNode { name, node_index } => self
                .saved_macro_tree_profiles
                .iter()
                .find(|saved| saved.name == *name)
                .and_then(|saved| saved.profile.tree().nodes().get(*node_index))
                .and_then(|node| node.macro_file().cloned()),
        }
    }

    fn macro_event_selection(
        &self,
        source: Option<&SelectedMacroSource>,
    ) -> (Option<usize>, Option<AulaOemMacroEvent>) {
        let Some(file) = source.and_then(|source| self.macro_file_for_source(source)) else {
            return (None, None);
        };
        let selected = file
            .oem_editor_record_index(self.selected_macro_event_index)
            .and_then(|index| file.records().get(index).copied())
            .map(AulaOemMacroEvent::from_record);
        (Some(file.oem_editor_event_count()), selected)
    }

    fn macro_source_label(&self, source: &SelectedMacroSource) -> String {
        match source {
            SelectedMacroSource::Profile(name) | SelectedMacroSource::Tree(name) => name.clone(),
            SelectedMacroSource::TreeNode { name, node_index } => {
                let path = self
                    .saved_macro_tree_profiles
                    .iter()
                    .find(|saved| saved.name == *name)
                    .map_or_else(
                        || format!("#{}", node_index + 1),
                        |saved| macro_tree_node_path_label(saved.profile.tree(), *node_index),
                    );
                format!("{name} · {path} · #{}", node_index + 1)
            }
        }
    }

    fn select_macro_source(&mut self, source: SelectedMacroSource) {
        self.selected_macro_source = Some(source);
        self.selected_macro_event_index = 0;
        self.macro_profile_error = None;
    }

    fn open_rename_macro_tree_node(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let SelectedMacroSource::TreeNode {
            name: profile_name,
            node_index,
        } = &source
        else {
            return;
        };
        let current_name = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == *profile_name)
            .and_then(|saved| saved.profile.tree().nodes().get(*node_index))
            .and_then(AulaOemMacroTreeNode::name)
            .unwrap_or_default();
        let view = cx.entity().downgrade();
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder(tr!("aula.macro_tree_node_name_placeholder"));
            input.set_value(current_name, window, cx);
            input
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.macro_tree_rename_node"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.macro_tree_node_name_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    let source = source.clone();
                    move |_, _, cx| {
                        let name = input.read(cx).value().to_string();
                        let mut renamed = false;
                        let _ = view.update(cx, |this, cx| {
                            match this.rename_macro_tree_node(&source, &name) {
                                Ok(()) => renamed = true,
                                Err(error) => this.macro_profile_error = Some(error),
                            }
                            cx.notify();
                        });
                        renamed
                    }
                })
        });
    }

    fn rename_macro_tree_node(
        &mut self,
        source: &SelectedMacroSource,
        new_name: &str,
    ) -> Result<(), String> {
        let SelectedMacroSource::TreeNode { name, node_index } = source else {
            return Err("select a macro-tree node before renaming it".to_owned());
        };
        let saved = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == *name)
            .ok_or_else(|| "the selected macro tree is no longer available".to_owned())?;
        let tree = saved
            .profile
            .tree()
            .rename_node(*node_index, new_name)
            .map_err(|error| error.to_string())?;
        let profile = AulaOemMacroTreeProfile::new(tree);
        aula_macro_tree_profiles::save(name, &profile).map_err(|error| error.to_string())?;
        match aula_macro_tree_profiles::load_all() {
            Ok(profiles) => self.saved_macro_tree_profiles = profiles,
            Err(error) => {
                tracing::warn!(%error, "could not reload local AULA macro-tree profiles after renaming");
                if let Some(saved) = self
                    .saved_macro_tree_profiles
                    .iter_mut()
                    .find(|saved| saved.name == *name)
                {
                    saved.profile = profile;
                }
            }
        }
        self.macro_profile_error = None;
        Ok(())
    }

    fn open_delete_macro_tree_node(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(SelectedMacroSource::TreeNode { name, node_index }) =
            self.selected_macro_source.clone()
        else {
            return;
        };
        let node_name = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == name)
            .and_then(|saved| saved.profile.tree().nodes().get(node_index))
            .and_then(AulaOemMacroTreeNode::name)
            .unwrap_or_else(|| format!("node {}", node_index + 1));
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(tr!("aula.macro_tree_delete_confirmation", name => node_name.clone()))
                .description(tr!("aula.macro_tree_delete_hint"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.macro_tree_delete_node"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    let name = name.clone();
                    move |_, _, cx| {
                        let mut deleted = false;
                        let _ = view.update(cx, |this, cx| {
                            match this.delete_macro_tree_node(&name, node_index) {
                                Ok(()) => deleted = true,
                                Err(error) => this.macro_profile_error = Some(error),
                            }
                            cx.notify();
                        });
                        deleted
                    }
                })
        });
    }

    fn delete_macro_tree_node(&mut self, name: &str, node_index: usize) -> Result<(), String> {
        let saved = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == name)
            .ok_or_else(|| "the selected macro tree is no longer available".to_owned())?;
        let tree = saved
            .profile
            .tree()
            .remove_node(node_index)
            .map_err(|error| error.to_string())?;
        let profile = AulaOemMacroTreeProfile::new(tree);
        aula_macro_tree_profiles::save(name, &profile).map_err(|error| error.to_string())?;
        self.saved_macro_tree_profiles =
            aula_macro_tree_profiles::load_all().map_err(|error| error.to_string())?;
        self.selected_macro_source = Some(SelectedMacroSource::Tree(name.to_owned()));
        self.selected_macro_event_index = 0;
        self.macro_profile_error = None;
        Ok(())
    }

    fn open_create_macro_tree_profile(window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("aula.macro_tree_profile_name_placeholder"))
        });
        let dialog_error = cx.new(|_| Option::<String>::None);
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.macro_tree_new_profile"))
                .child(v_flex().gap_2().child(control_input(&input)).children(
                    dialog_error.read(cx).as_ref().map(|error| {
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(error.clone())
                    }),
                ))
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    let dialog_error = dialog_error.clone();
                    move |_, window, cx| {
                        let name = input.read(cx).value().to_string();
                        let mut created = false;
                        let _ = view.update(cx, |this, cx| {
                            match this.create_macro_tree_profile(&name) {
                                Ok(()) => created = true,
                                Err(error) => {
                                    this.macro_profile_error = Some(error.clone());
                                    dialog_error.update(cx, |message, cx| {
                                        *message = Some(error);
                                        cx.notify();
                                    });
                                    input.update(cx, |input, cx| input.focus(window, cx));
                                }
                            }
                            cx.notify();
                        });
                        created
                    }
                })
        });
    }

    fn create_macro_tree_profile(&mut self, name: &str) -> Result<(), String> {
        let saved =
            aula_macro_tree_profiles::create_empty(name).map_err(|error| error.to_string())?;
        match aula_macro_tree_profiles::load_all() {
            Ok(profiles) => self.saved_macro_tree_profiles = profiles,
            Err(error) => {
                tracing::warn!(%error, "could not reload local AULA macro-tree profiles after creation");
                self.saved_macro_tree_profiles.push(saved.clone());
                self.saved_macro_tree_profiles
                    .sort_by_key(|profile| profile.name.to_lowercase());
            }
        }
        self.select_macro_source(SelectedMacroSource::Tree(saved.name));
        self.macro_profile_error = None;
        Ok(())
    }

    fn open_create_macro_tree_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source @ (SelectedMacroSource::Tree(_) | SelectedMacroSource::TreeNode { .. })) =
            self.selected_macro_source.clone()
        else {
            return;
        };
        let view = cx.entity().downgrade();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("aula.macro_tree_node_name_placeholder"))
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.macro_tree_create_folder"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.macro_tree_node_name_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    let source = source.clone();
                    move |_, _, cx| {
                        let name = input.read(cx).value().to_string();
                        let mut created_source = None;
                        let _ = view.update(cx, |this, cx| {
                            match this.create_macro_tree_folder(&source, &name) {
                                Ok(created) => created_source = Some(created),
                                Err(error) => this.macro_profile_error = Some(error),
                            }
                            if let Some(source) = created_source.clone() {
                                this.select_macro_source(source);
                            }
                            cx.notify();
                        });
                        created_source.is_some()
                    }
                })
        });
    }

    fn create_macro_tree_folder(
        &mut self,
        source: &SelectedMacroSource,
        name: &str,
    ) -> Result<SelectedMacroSource, String> {
        let profile_name = match source {
            SelectedMacroSource::Tree(name) | SelectedMacroSource::TreeNode { name, .. } => name,
            SelectedMacroSource::Profile(_) => {
                return Err("select a macro tree before creating a folder".to_owned());
            }
        };
        let saved = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == *profile_name)
            .ok_or_else(|| "the selected macro tree is no longer available".to_owned())?;
        let tree = saved.profile.tree();
        let parent_id = match source {
            SelectedMacroSource::Tree(_) | SelectedMacroSource::Profile(_) => {
                AULA_OEM_MACRO_TREE_ROOT_NODE_ID
            }
            SelectedMacroSource::TreeNode { node_index, .. } => {
                let node = tree.nodes().get(*node_index).ok_or_else(|| {
                    "the selected macro-tree node is no longer available".to_owned()
                })?;
                if node.kind_code() == 1 && node.macro_file().is_none() {
                    node.node_id()
                } else if node.parent_id() == AULA_OEM_MACRO_TREE_ROOT_NODE_ID {
                    AULA_OEM_MACRO_TREE_ROOT_NODE_ID
                } else {
                    node.parent_id()
                }
            }
        };
        let node_id = tree.next_available_node_id(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.subsec_nanos()),
        );
        let tree = tree
            .create_folder(parent_id, node_id, name)
            .map_err(|error| error.to_string())?;
        let folder_index = tree.nodes().len() - 1;
        let profile = AulaOemMacroTreeProfile::new(tree);
        aula_macro_tree_profiles::save(profile_name, &profile)
            .map_err(|error| error.to_string())?;
        match aula_macro_tree_profiles::load_all() {
            Ok(profiles) => self.saved_macro_tree_profiles = profiles,
            Err(error) => {
                tracing::warn!(%error, "could not reload local AULA macro-tree profiles after folder creation");
                if let Some(saved) = self
                    .saved_macro_tree_profiles
                    .iter_mut()
                    .find(|saved| saved.name == *profile_name)
                {
                    saved.profile = profile;
                }
            }
        }
        self.macro_profile_error = None;
        Ok(SelectedMacroSource::TreeNode {
            name: profile_name.clone(),
            node_index: folder_index,
        })
    }

    fn open_create_macro_tree_macro(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source @ (SelectedMacroSource::Tree(_) | SelectedMacroSource::TreeNode { .. })) =
            self.selected_macro_source.clone()
        else {
            return;
        };
        let view = cx.entity().downgrade();
        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr!("aula.macro_tree_node_name_placeholder"))
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.macro_tree_create_macro"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.macro_tree_node_name_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    let source = source.clone();
                    move |_, _, cx| {
                        let name = input.read(cx).value().to_string();
                        let mut created_source = None;
                        let _ = view.update(cx, |this, cx| {
                            match this.create_macro_tree_macro(&source, &name) {
                                Ok(created) => created_source = Some(created),
                                Err(error) => this.macro_profile_error = Some(error),
                            }
                            if let Some(source) = created_source.clone() {
                                this.select_macro_source(source);
                            }
                            cx.notify();
                        });
                        created_source.is_some()
                    }
                })
        });
    }

    fn create_macro_tree_macro(
        &mut self,
        source: &SelectedMacroSource,
        name: &str,
    ) -> Result<SelectedMacroSource, String> {
        let profile_name = match source {
            SelectedMacroSource::Tree(name) | SelectedMacroSource::TreeNode { name, .. } => name,
            SelectedMacroSource::Profile(_) => {
                return Err("select a macro tree before creating a macro".to_owned());
            }
        };
        let saved = self
            .saved_macro_tree_profiles
            .iter()
            .find(|saved| saved.name == *profile_name)
            .ok_or_else(|| "the selected macro tree is no longer available".to_owned())?;
        let tree = saved.profile.tree();
        let parent_id = match source {
            SelectedMacroSource::Tree(_) | SelectedMacroSource::Profile(_) => {
                AULA_OEM_MACRO_TREE_ROOT_NODE_ID
            }
            SelectedMacroSource::TreeNode { node_index, .. } => {
                let node = tree.nodes().get(*node_index).ok_or_else(|| {
                    "the selected macro-tree node is no longer available".to_owned()
                })?;
                if node.kind_code() == 1 && node.macro_file().is_none() {
                    node.node_id()
                } else {
                    node.parent_id()
                }
            }
        };
        let node_id = tree.next_available_node_id(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.subsec_nanos()),
        );
        let tree = tree
            .create_macro(parent_id, node_id, name)
            .map_err(|error| error.to_string())?;
        let macro_index = tree.nodes().len() - 1;
        let profile = AulaOemMacroTreeProfile::new(tree);
        aula_macro_tree_profiles::save(profile_name, &profile)
            .map_err(|error| error.to_string())?;
        match aula_macro_tree_profiles::load_all() {
            Ok(profiles) => self.saved_macro_tree_profiles = profiles,
            Err(error) => {
                tracing::warn!(%error, "could not reload local AULA macro-tree profiles after macro creation");
                if let Some(saved) = self
                    .saved_macro_tree_profiles
                    .iter_mut()
                    .find(|saved| saved.name == *profile_name)
                {
                    saved.profile = profile;
                }
            }
        }
        self.macro_profile_error = None;
        Ok(SelectedMacroSource::TreeNode {
            name: profile_name.clone(),
            node_index: macro_index,
        })
    }

    fn macro_event_controls(
        &self,
        count: Option<usize>,
        selected_event: Option<AulaOemMacroEvent>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let event_index = self.selected_macro_event_index;
        v_flex()
            .gap_1()
            .when_some(count, |panel, count| {
                panel
                    .child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.macro_event_count", count = count)),
                    )
                    .child(Self::macro_event_action_controls(
                        event_index,
                        count,
                        selected_event.is_some(),
                        cx,
                    ))
            })
            .children(selected_event.map(|event| {
                let reserved = event.reserved();
                let fields = tr!(
                    "aula.macro_event_fields",
                    code => format!("{:04X}", event.event_code()),
                    auxiliary => format!("{:04X}", event.auxiliary()),
                    value => format!("{:04X}", event.value()),
                    reserved => format!("{:02X}{:02X}", reserved[0], reserved[1]),
                    special => format!("{:08X}", event.special_value()),
                );
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(format!(
                        "{} · {} · {fields}",
                        tr!(
                            "aula.macro_event_position",
                            index = event_index.saturating_add(1),
                            count = count.unwrap_or_default()
                        ),
                        macro_event_kind_label(event.kind()),
                    ))
            }))
    }

    fn macro_event_action_controls(
        event_index: usize,
        count: usize,
        has_selected_event: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(
                control_button("aula-macro-event-previous")
                    .label(tr!("aula.macro_event_previous"))
                    .disabled(event_index == 0 || count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected_macro_event_index =
                            this.selected_macro_event_index.saturating_sub(1);
                        cx.notify();
                    })),
            )
            .child(
                control_button("aula-macro-event-next")
                    .label(tr!("aula.macro_event_next"))
                    .disabled(event_index.saturating_add(1) >= count)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected_macro_event_index =
                            this.selected_macro_event_index.saturating_add(1);
                        cx.notify();
                    })),
            )
            .child(
                control_button("aula-macro-event-move-up")
                    .label(tr!("aula.macro_event_move_up"))
                    .disabled(event_index == 0 || count < 2)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.move_macro_event_to(
                            this.selected_macro_event_index.saturating_sub(1),
                            cx,
                        );
                    })),
            )
            .child(
                control_button("aula-macro-event-move-down")
                    .label(tr!("aula.macro_event_move_down"))
                    .disabled(event_index.saturating_add(1) >= count)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.move_macro_event_to(
                            this.selected_macro_event_index.saturating_add(1),
                            cx,
                        );
                    })),
            )
            .child(
                control_button("aula-macro-event-edit")
                    .label(tr!("aula.macro_event_edit"))
                    .disabled(!has_selected_event)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_edit_macro_event(window, cx);
                    })),
            )
            .child(
                control_button("aula-macro-event-insert")
                    .label(tr!("aula.macro_event_insert"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_insert_macro_event(window, cx);
                    })),
            )
            .child(
                control_button("aula-macro-event-remove")
                    .label(tr!("aula.macro_event_remove"))
                    .disabled(!has_selected_event)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_remove_macro_event(window, cx);
                    })),
            )
    }

    fn import_macro_profile(cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("aula.macro_import")),
        });
        cx.spawn(async move |view, cx| {
            let selected = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.macro_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = selected else {
                return;
            };
            if aula_macro_tree_profiles::is_tree_input(&path) {
                match aula_macro_tree_profiles::import(&path) {
                    Ok(profile) => {
                        let selected_source = profile
                            .profile
                            .tree()
                            .nodes()
                            .iter()
                            .position(|node| node.macro_file().is_some())
                            .map_or_else(
                                || SelectedMacroSource::Tree(profile.name.clone()),
                                |node_index| SelectedMacroSource::TreeNode {
                                    name: profile.name.clone(),
                                    node_index,
                                },
                            );
                        let profiles = aula_macro_tree_profiles::load_all().unwrap_or_else(|error| {
                            tracing::warn!(%error, "could not reload local AULA macro-tree profiles");
                            Vec::new()
                        });
                        let _ = view.update(cx, |this, cx| {
                            this.select_macro_source(selected_source);
                            this.saved_macro_tree_profiles = profiles;
                            this.macro_profile_error = None;
                            cx.notify();
                        });
                    }
                    Err(error) => {
                        let _ = view.update(cx, |this, cx| {
                            this.macro_profile_error = Some(error.to_string());
                            cx.notify();
                        });
                    }
                }
                return;
            }
            match aula_macro_profiles::import(&path) {
                Ok(profile) => {
                    let profiles = aula_macro_profiles::load_all().unwrap_or_else(|error| {
                        tracing::warn!(%error, "could not reload local AULA macro profiles");
                        Vec::new()
                    });
                    let _ = view.update(cx, |this, cx| {
                        this.select_macro_source(SelectedMacroSource::Profile(profile.name));
                        this.saved_macro_profiles = profiles;
                        this.macro_profile_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.macro_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_edit_macro_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_macro_event_dialog(window, cx, Some(self.selected_macro_event_index));
    }

    fn open_insert_macro_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_macro_event_dialog(window, cx, None);
    }

    fn open_macro_event_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        edit_index: Option<usize>,
    ) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let Some(macro_file) = self.macro_file_for_source(&source) else {
            return;
        };
        let Some((visible_index, raw_index, record)) =
            macro_event_dialog_target(&macro_file, self.selected_macro_event_index, edit_index)
        else {
            return;
        };
        let view = cx.entity().downgrade();
        let inputs = macro_event_field_inputs(record, window, cx);
        let dialog_error = cx.new(|_| Option::<String>::None);
        window.open_dialog(cx, move |dialog, window, cx| {
            inputs[0].update(cx, |input, cx| input.focus(window, cx));
            let event_file = macro_file.clone();
            dialog
                .w_128()
                .title(if edit_index.is_some() {
                    tr!("aula.macro_event_edit")
                } else {
                    tr!("aula.macro_event_insert")
                })
                .child(macro_event_form(
                    &inputs,
                    edit_index.is_none(),
                    dialog_error.read(cx).clone(),
                    cx,
                ))
                .footer(save_cancel_footer())
                .on_ok({
                    let inputs = inputs.clone();
                    let view = view.clone();
                    let dialog_error = dialog_error.clone();
                    let source = source.clone();
                    let macro_file = event_file;
                    move |_, window, cx| {
                        let values = inputs
                            .each_ref()
                            .map(|input| input.read(cx).value().to_string());
                        let record = match parse_macro_event_fields(&values) {
                            Ok(record) => record,
                            Err(field_index) => {
                                let field = macro_event_field_label(field_index);
                                let error =
                                    tr!("aula.macro_event_invalid_hex", field = field).to_string();
                                dialog_error.update(cx, |message, cx| {
                                    *message = Some(error.clone());
                                    cx.notify();
                                });
                                inputs[field_index].update(cx, |input, cx| {
                                    input.focus(window, cx);
                                });
                                let _ = view.update(cx, |this, cx| {
                                    this.macro_profile_error = Some(error);
                                    cx.notify();
                                });
                                return false;
                            }
                        };
                        let edited = match edit_index {
                            Some(_) => macro_file.replace_record(raw_index, record),
                            None => macro_file.insert_record(raw_index, record),
                        };
                        let edited = match edited {
                            Ok(macro_file) => macro_file,
                            Err(error) => {
                                let message = error.to_string();
                                dialog_error.update(cx, |dialog_error, cx| {
                                    *dialog_error = Some(message.clone());
                                    cx.notify();
                                });
                                let _ = view.update(cx, |this, cx| {
                                    this.macro_profile_error = Some(message);
                                    cx.notify();
                                });
                                return false;
                            }
                        };
                        let mut saved = false;
                        let _ = view.update(cx, |this, cx| {
                            match this.save_macro_edit(&source, &edited, visible_index) {
                                Ok(()) => saved = true,
                                Err(error) => {
                                    dialog_error.update(cx, |dialog_error, cx| {
                                        *dialog_error = Some(error.clone());
                                        cx.notify();
                                    });
                                    this.macro_profile_error = Some(error);
                                }
                            }
                            cx.notify();
                        });
                        saved
                    }
                })
        });
    }

    fn open_remove_macro_event(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let Some(macro_file) = self.macro_file_for_source(&source) else {
            return;
        };
        let visible_count = macro_file.oem_editor_event_count();
        let visible_index = self.selected_macro_event_index;
        let Some(index) = macro_file.oem_editor_record_index(visible_index) else {
            return;
        };
        let selected_index = visible_index.min(visible_count.saturating_sub(2));
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let event_file = macro_file.clone();
            alert
                .title(tr!(
                    "aula.macro_event_remove_confirmation",
                    index = visible_index + 1
                ))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.macro_event_remove"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let view = view.clone();
                    let source = source.clone();
                    let macro_file = event_file;
                    move |_, _, cx| {
                        let edited = match macro_file.remove_record(index) {
                            Ok(macro_file) => macro_file,
                            Err(error) => {
                                let _ = view.update(cx, |this, cx| {
                                    this.macro_profile_error = Some(error.to_string());
                                    cx.notify();
                                });
                                return false;
                            }
                        };
                        let mut saved = false;
                        let _ = view.update(cx, |this, cx| {
                            match this.save_macro_edit(&source, &edited, selected_index) {
                                Ok(()) => saved = true,
                                Err(error) => this.macro_profile_error = Some(error),
                            }
                            cx.notify();
                        });
                        saved
                    }
                })
        });
    }

    fn move_macro_event_to(&mut self, target_index: usize, cx: &mut Context<Self>) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let Some(macro_file) = self.macro_file_for_source(&source) else {
            return;
        };
        let current_index = self.selected_macro_event_index;
        if target_index >= macro_file.oem_editor_event_count() || target_index == current_index {
            return;
        }
        let edited = match macro_file.swap_oem_editor_events(current_index, target_index) {
            Ok(macro_file) => macro_file,
            Err(error) => {
                self.macro_profile_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        if let Err(error) = self.save_macro_edit(&source, &edited, target_index) {
            self.macro_profile_error = Some(error);
        }
        cx.notify();
    }

    fn save_macro_edit(
        &mut self,
        source: &SelectedMacroSource,
        macro_file: &AulaOemMacroFile,
        selected_index: usize,
    ) -> Result<(), String> {
        match source {
            SelectedMacroSource::Profile(name) => {
                let profile = AulaOemMacroProfile::new(macro_file.clone());
                aula_macro_profiles::save(name, &profile).map_err(|error| error.to_string())?;
                match aula_macro_profiles::load_all() {
                    Ok(profiles) => self.saved_macro_profiles = profiles,
                    Err(error) => {
                        tracing::warn!(%error, "could not reload local AULA macro profiles after editing");
                        if let Some(saved) = self
                            .saved_macro_profiles
                            .iter_mut()
                            .find(|saved| saved.name == *name)
                        {
                            saved.profile = profile;
                        }
                    }
                }
            }
            SelectedMacroSource::TreeNode { name, node_index } => {
                let Some(saved) = self
                    .saved_macro_tree_profiles
                    .iter()
                    .find(|saved| saved.name == *name)
                else {
                    return Err("the selected macro tree is no longer available".to_owned());
                };
                let tree = saved
                    .profile
                    .tree()
                    .replace_macro_file(*node_index, macro_file.clone())
                    .map_err(|error| error.to_string())?;
                let profile = AulaOemMacroTreeProfile::new(tree);
                aula_macro_tree_profiles::save(name, &profile)
                    .map_err(|error| error.to_string())?;
                match aula_macro_tree_profiles::load_all() {
                    Ok(profiles) => self.saved_macro_tree_profiles = profiles,
                    Err(error) => {
                        tracing::warn!(%error, "could not reload local AULA macro-tree profiles after editing");
                        if let Some(saved) = self
                            .saved_macro_tree_profiles
                            .iter_mut()
                            .find(|saved| saved.name == *name)
                        {
                            saved.profile = profile;
                        }
                    }
                }
            }
            SelectedMacroSource::Tree(_) => {
                return Err("select a macro node before editing its events".to_owned());
            }
        }
        self.selected_macro_source = Some(source.clone());
        self.selected_macro_event_index =
            selected_index.min(macro_file.oem_editor_event_count().saturating_sub(1));
        self.macro_profile_error = None;
        Ok(())
    }

    fn export_macro_profile(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let (directory, default_name, export) = match &source {
            SelectedMacroSource::Profile(name) => {
                let Some(profile) = self
                    .saved_macro_profiles
                    .iter()
                    .find(|profile| profile.name == *name)
                    .map(|profile| profile.profile.clone())
                else {
                    return;
                };
                let directory = match aula_macro_profiles::ensure_directory() {
                    Ok(directory) => directory,
                    Err(error) => {
                        self.macro_profile_error = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                };
                (
                    directory,
                    "aula-macro.mcr",
                    MacroProfileExport::Macro(profile),
                )
            }
            SelectedMacroSource::Tree(name) | SelectedMacroSource::TreeNode { name, .. } => {
                let Some(profile) = self
                    .saved_macro_tree_profiles
                    .iter()
                    .find(|profile| profile.name == *name)
                    .map(|profile| profile.profile.clone())
                else {
                    return;
                };
                let directory = match aula_macro_tree_profiles::ensure_directory() {
                    Ok(directory) => directory,
                    Err(error) => {
                        self.macro_profile_error = Some(error.to_string());
                        cx.notify();
                        return;
                    }
                };
                (
                    directory,
                    "aula-macro-tree.dct",
                    MacroProfileExport::Tree(profile),
                )
            }
        };
        let receiver = cx.prompt_for_new_path(&directory, Some(default_name));
        cx.spawn(async move |view, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.macro_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let result = match export {
                MacroProfileExport::Macro(profile) => aula_macro_profiles::export(&profile, &path),
                MacroProfileExport::Tree(profile) => {
                    aula_macro_tree_profiles::export(&profile, &path)
                }
            };
            match result {
                Ok(()) => {
                    let _ = view.update(cx, |this, cx| {
                        this.macro_profile_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.macro_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_delete_macro_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(source) = self.selected_macro_source.clone() else {
            return;
        };
        let name = match &source {
            SelectedMacroSource::Profile(name)
            | SelectedMacroSource::Tree(name)
            | SelectedMacroSource::TreeNode { name, .. } => name.clone(),
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(tr!("aula.macro_delete_confirmation", name => name.clone()))
                .description(tr!("aula.macro_delete_hint"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.macro_delete"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let name = name.clone();
                    let source = source.clone();
                    let view = view.clone();
                    move |_, _, cx| {
                        let result = match source {
                            SelectedMacroSource::Profile(_) => aula_macro_profiles::delete(&name),
                            SelectedMacroSource::Tree(_) | SelectedMacroSource::TreeNode { .. } => {
                                aula_macro_tree_profiles::delete(&name)
                            }
                        };
                        match result {
                        Ok(()) => {
                            let _ = view.update(cx, |this, cx| {
                                this.saved_macro_profiles = aula_macro_profiles::load_all()
                                    .unwrap_or_else(|error| {
                                        tracing::warn!(%error, "could not reload local AULA macro profiles");
                                        Vec::new()
                                    });
                                this.saved_macro_tree_profiles = aula_macro_tree_profiles::load_all()
                                    .unwrap_or_else(|error| {
                                        tracing::warn!(%error, "could not reload local AULA macro-tree profiles");
                                        Vec::new()
                                    });
                                this.selected_macro_source = None;
                                this.selected_macro_event_index = 0;
                                this.macro_profile_error = None;
                                cx.notify();
                            });
                            true
                        }
                        Err(error) => {
                            let _ = view.update(cx, |this, cx| {
                                this.macro_profile_error = Some(error.to_string());
                                cx.notify();
                            });
                            false
                        }
                        }
                    }
                })
        });
    }

    fn import_keymap_profile(cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("aula.keymap_import_profile")),
        });
        cx.spawn(async move |view, cx| {
            let selected = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = selected else {
                return;
            };
            let oem_profile = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dct"));
            let imported = if oem_profile {
                aula_keymap_profiles::import_oem_profile(&path)
            } else {
                aula_keymap_profiles::import(&path)
            };
            match imported {
                Ok(profile) => {
                    let profiles = aula_keymap_profiles::load_all().unwrap_or_else(|error| {
                        tracing::warn!(%error, "could not reload local AULA keymap profiles");
                        Vec::new()
                    });
                    let _ = view.update(cx, |this, cx| {
                        this.editor.keymap_draft = Some(profile.keymap.clone());
                        this.editor
                            .keymap_oem_fn_actions
                            .clone_from(&profile.oem_fn_actions);
                        this.editor.keymap_action = None;
                        this.selected_keymap_profile = Some(profile.name);
                        this.saved_keymap_profiles = profiles;
                        this.keymap_profile_error = None;
                        this.editor.completed = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn import_wired_keymap(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone().filter(|target| target.wired) else {
            return;
        };
        let selection_epoch = self.selection_epoch;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("aula.wired_keymap_import")),
        });
        cx.spawn(async move |view, cx| {
            let selected = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        if this.selection_epoch != selection_epoch
                            || this.target.as_ref() != Some(&target)
                        {
                            return;
                        }
                        this.editor.wired_keymap_error = Some(error.to_string());
                        this.editor.wired_keymap_status = None;
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = selected else {
                return;
            };
            let is_oem_profile = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("dct"));
            let imported = std::fs::read(&path)
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    if is_oem_profile {
                        aula_wired_keymap_base_from_oem_profile(&bytes)
                            .map_err(|error| error.to_string())
                    } else {
                        AulaWiredKeymapBlob::from_bytes(bytes).map_err(|error| error.to_string())
                    }
                });
            match imported {
                Ok(blob) => {
                    let status = if is_oem_profile {
                        tr!("aula.wired_keymap_profile_loaded").to_string()
                    } else {
                        let layer = match blob.layer() {
                            AulaWiredKeymapLayer::Base => tr!("aula.keymap_base_layer"),
                            AulaWiredKeymapLayer::Fn => tr!("aula.keymap_fn_layer"),
                        };
                        tr!("aula.wired_keymap_report_loaded", layer = layer).to_string()
                    };
                    let _ = view.update(cx, |this, cx| {
                        if this.selection_epoch != selection_epoch
                            || this.target.as_ref() != Some(&target)
                        {
                            return;
                        }
                        this.editor.wired_keymap = Some(blob);
                        this.editor.keymap_index = 0;
                        this.editor.wired_keymap_action = None;
                        this.editor.wired_keymap_error = None;
                        this.editor.wired_keymap_status = Some(status);
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        if this.selection_epoch != selection_epoch
                            || this.target.as_ref() != Some(&target)
                        {
                            return;
                        }
                        this.editor.wired_keymap_error = Some(error);
                        this.editor.wired_keymap_status = None;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn export_wired_keymap(&mut self, cx: &mut Context<Self>) {
        let Some(blob) = self.editor.wired_keymap.clone() else {
            return;
        };
        let directory = match aula_keymap_profiles::ensure_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.editor.wired_keymap_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let receiver = cx.prompt_for_new_path(&directory, Some("aula-wired-keymap.bin"));
        cx.spawn(async move |view, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.editor.wired_keymap_error = Some(error.to_string());
                        this.editor.wired_keymap_status = None;
                        cx.notify();
                    });
                    return;
                }
            };
            match std::fs::write(&path, blob.as_bytes()) {
                Ok(()) => {
                    let _ = view.update(cx, |this, cx| {
                        this.editor.wired_keymap_error = None;
                        this.editor.wired_keymap_status =
                            Some(tr!("aula.wired_keymap_report_saved").to_string());
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.editor.wired_keymap_error = Some(error.to_string());
                        this.editor.wired_keymap_status = None;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn export_keymap_profile(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected_keymap_profile.as_deref() else {
            return;
        };
        let Some((keymap, oem_fn_records, oem_fn_actions)) = self
            .saved_keymap_profiles
            .iter()
            .find(|profile| profile.name == name)
            .map(|profile| {
                (
                    profile.keymap.clone(),
                    profile.oem_fn_records.clone(),
                    profile.oem_fn_actions.clone(),
                )
            })
        else {
            return;
        };
        let directory = match aula_keymap_profiles::ensure_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.keymap_profile_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let receiver = cx.prompt_for_new_path(&directory, Some("aula-keymap-profile.json"));
        cx.spawn(async move |view, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            match aula_keymap_profiles::export(
                &keymap,
                oem_fn_records.as_ref(),
                oem_fn_actions.as_ref(),
                &path,
            ) {
                Ok(()) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn export_oem_keymap_profile(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected_keymap_profile.as_deref() else {
            return;
        };
        let Some((keymap, oem_fn_records, oem_fn_actions)) = self
            .saved_keymap_profiles
            .iter()
            .find(|profile| profile.name == name)
            .map(|profile| {
                (
                    profile.keymap.clone(),
                    profile.oem_fn_records.clone(),
                    profile.oem_fn_actions.clone(),
                )
            })
        else {
            return;
        };
        let template_receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("aula.keymap_export_oem_profile")),
        });
        cx.spawn(async move |view, cx| {
            let template_path = match template_receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(template_path) = template_path else {
                return;
            };
            let output_receiver = match view.update(cx, |_, cx| {
                cx.prompt_for_new_path(
                    template_path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new(".")),
                    Some("profile.dct"),
                )
            }) {
                Ok(receiver) => receiver,
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let output_path = match output_receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            match aula_keymap_profiles::export_oem_profile(
                &keymap,
                oem_fn_records.as_ref(),
                oem_fn_actions.as_ref(),
                &template_path,
                &output_path,
            ) {
                Ok(()) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.keymap_profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_save_keymap_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(keymap) = self.editor.keymap_draft.clone() else {
            return;
        };
        let view = cx.entity().downgrade();
        let selected = self.selected_keymap_profile.clone();
        let oem_fn_records = selected.as_deref().and_then(|name| {
            self.saved_keymap_profiles
                .iter()
                .find(|profile| profile.name == name)
                .and_then(|profile| profile.oem_fn_records.clone())
        });
        let oem_fn_actions = self.editor.keymap_oem_fn_actions.clone();
        let input = cx.new(|cx| {
            let mut input =
                InputState::new(window, cx).placeholder(tr!("aula.keymap_profile_name"));
            if let Some(name) = selected {
                input.set_value(name, window, cx);
            }
            input
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            let keymap = keymap.clone();
            let oem_fn_records = oem_fn_records.clone();
            let oem_fn_actions = oem_fn_actions.clone();
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.keymap_save_profile"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.keymap_profile_name_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    move |_, _, cx| {
                        let name = input.read(cx).value().to_string();
                        match aula_keymap_profiles::save(
                            &name,
                            &keymap,
                            oem_fn_records.as_ref(),
                            oem_fn_actions.as_ref(),
                        ) {
                            Ok(()) => {
                                let _ = view.update(cx, |this, cx| {
                                    this.saved_keymap_profiles =
                                        aula_keymap_profiles::load_all().unwrap_or_else(|error| {
                                            tracing::warn!(%error, "could not reload local AULA keymap profiles");
                                            Vec::new()
                                        });
                                    this.selected_keymap_profile = Some(name.clone());
                                    this.keymap_profile_error = None;
                                    cx.notify();
                                });
                                true
                            }
                            Err(error) => {
                                let _ = view.update(cx, |this, cx| {
                                    this.keymap_profile_error = Some(error.to_string());
                                    cx.notify();
                                });
                                false
                            }
                        }
                    }
                })
        });
    }

    fn open_delete_keymap_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.selected_keymap_profile.clone() else {
            return;
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(tr!("aula.keymap_delete_profile_confirmation", name => name.clone()))
                .description(tr!("aula.keymap_delete_profile_hint"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.keymap_delete_profile"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let name = name.clone();
                    let view = view.clone();
                    move |_, _, cx| match aula_keymap_profiles::delete(&name) {
                        Ok(()) => {
                            let _ = view.update(cx, |this, cx| {
                                this.saved_keymap_profiles =
                                    aula_keymap_profiles::load_all().unwrap_or_else(|error| {
                                        tracing::warn!(%error, "could not reload local AULA keymap profiles");
                                        Vec::new()
                                    });
                                this.selected_keymap_profile = None;
                                this.keymap_profile_error = None;
                                cx.notify();
                            });
                            true
                        }
                        Err(error) => {
                            let _ = view.update(cx, |this, cx| {
                                this.keymap_profile_error = Some(error.to_string());
                                cx.notify();
                            });
                            false
                        }
                    }
                })
        });
    }

    fn keymap_controls(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self.editor.keymap_draft.as_ref();
        let layer_len: usize = if self.editor.keymap_fn_layer {
            AULA_FN_KEY_COUNT
        } else {
            AULA_BASE_KEY_COUNT
        };
        let index = self.editor.keymap_index.min(layer_len.saturating_sub(1));
        let current = draft.and_then(|keymap| {
            if self.editor.keymap_fn_layer {
                keymap.fn_layer().get(index)
            } else {
                keymap.base().get(index)
            }
        });
        let fn_layer = self.editor.keymap_fn_layer;
        let assignable = aula_keymap_slot_is_assignable(index, fn_layer);
        let reserved_change = self
            .editor
            .keymap
            .as_ref()
            .zip(draft)
            .and_then(|(original, draft)| draft.first_reserved_slot_changed(original));
        let reserved_warning = Self::keymap_reserved_warning(reserved_change, cx);
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.keymap_heading")))
            .child(self.keymap_profile_controls(cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(self.keymap_layer_control(enabled && draft.is_some(), cx))
                    .child(self.keymap_index_control(
                        enabled && draft.is_some(),
                        layer_len,
                        fn_layer,
                        cx,
                    )),
            )
            .child(Self::keymap_restore_fn_defaults_button(
                enabled && draft.is_some(),
                cx,
            ))
            .child(Self::keymap_keyboard(
                index,
                enabled && draft.is_some(),
                fn_layer,
                cx,
            ))
            .child(div().text_caption().child(tr!(
                "aula.keymap_selected",
                index = index,
                entry = current
                    .copied()
                    .map_or_else(|| "-- -- -- --".to_owned(), format_key_entry)
            )))
            .child(self.keymap_action_editor_controls(enabled && draft.is_some() && assignable, cx))
            .child(self.keymap_oem_fields_button(cx))
            .child(
                control_button("aula-keymap-stage")
                    .label(tr!("aula.keymap_stage"))
                    .disabled(
                        !enabled
                            || !assignable
                            || draft.is_none()
                            || self.editor.keymap_action.is_none(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editor.stage_keymap_action();
                        cx.notify();
                    })),
            )
            .child(
                control_button("aula-keymap-read")
                    .label(tr!("aula.keymap_read"))
                    .disabled(!enabled)
                    .on_click(cx.listener(|this, _, _, cx| this.read_keymap(cx))),
            )
            .child(
                control_button("aula-keymap-write")
                    .primary()
                    .label(tr!("aula.keymap_write"))
                    .disabled(
                        !enabled || draft.is_none() || self.editor.keymap.is_none() || self.pending,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.write_keymap(cx))),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.keymap_hint")),
            )
            .children(self.keymap_profile_error.as_ref().map(|error| {
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(error.clone())
            }))
            .child(reserved_warning)
    }

    fn wired_keymap_controls(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let index = self
            .editor
            .keymap_index
            .min(AULA_WIRED_KEYMAP_SLOT_COUNT.saturating_sub(1));
        let blob = self.editor.wired_keymap.as_ref();
        let slot = blob.and_then(|blob| blob.slot(index));
        let layer = blob.map_or_else(
            || tr!("aula.wired_keymap_no_report"),
            |blob| match blob.layer() {
                AulaWiredKeymapLayer::Base => tr!("aula.keymap_base_layer"),
                AulaWiredKeymapLayer::Fn => tr!("aula.keymap_fn_layer"),
            },
        );
        let editable = enabled && blob.is_some();
        v_flex()
            .gap_2()
            .child(div().text_body().child(tr!("aula.wired_keymap_heading")))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        control_button("aula-wired-keymap-import")
                            .label(tr!("aula.wired_keymap_import"))
                            .disabled(!enabled || self.pending)
                            .on_click(cx.listener(|this, _, _, cx| this.import_wired_keymap(cx))),
                    )
                    .child(
                        control_button("aula-wired-keymap-export")
                            .label(tr!("aula.wired_keymap_export"))
                            .disabled(!editable || self.pending)
                            .on_click(cx.listener(|this, _, _, cx| this.export_wired_keymap(cx))),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(self.keymap_index_control(
                        editable,
                        AULA_WIRED_KEYMAP_SLOT_COUNT,
                        false,
                        cx,
                    ))
                    .child(
                        div()
                            .text_caption()
                            .child(tr!("aula.wired_keymap_layer", layer = layer)),
                    ),
            )
            .child(Self::keymap_keyboard(index, editable, false, cx))
            .child(div().text_caption().child(tr!(
                "aula.wired_keymap_selected",
                index = index,
                entry = slot.map_or_else(|| "-- -- -- --".to_owned(), format_key_entry)
            )))
            .child(self.wired_keymap_action_control(editable, cx))
            .child(
                control_button("aula-wired-keymap-stage")
                    .label(tr!("aula.wired_keymap_stage"))
                    .disabled(
                        !editable
                            || !aula_keymap_slot_is_assignable(index, false)
                            || self.editor.wired_keymap_action.is_none(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.editor.stage_wired_keymap_action();
                        cx.notify();
                    })),
            )
            .child(
                control_button("aula-wired-keymap-write")
                    .primary()
                    .label(tr!("aula.wired_keymap_write"))
                    .disabled(!editable || self.pending)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.confirm_wired_keymap_write(window, cx);
                    })),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(tr!("aula.wired_keymap_hint")),
            )
            .children(self.editor.wired_keymap_status.as_ref().map(|status| {
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(status.clone())
            }))
            .children(self.editor.wired_keymap_error.as_ref().map(|error| {
                div()
                    .text_caption()
                    .text_color(theme::palette(cx).text_muted)
                    .child(error.clone())
            }))
    }

    fn wired_keymap_action_control(
        &self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.editor.wired_keymap_action;
        let selected_label = selected.map_or_else(
            || tr!("aula.wired_keymap_choose_action").to_string(),
            format_key_entry,
        );
        let action_labels = aula_keymap_action_outputs();
        let actions = aula_wired_keymap_action_outputs()
            .into_iter()
            .map(|(name, label, raw)| {
                let label = action_labels
                    .iter()
                    .find(|output| output.name() == name)
                    .map_or(label, localized_keymap_action_label);
                (label, raw)
            })
            .collect::<Vec<_>>();
        let view = cx.entity().downgrade();
        control_button("aula-wired-keymap-action")
            .label(selected_label)
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for (label, action) in &actions {
                    let view = view.clone();
                    let action = *action;
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(selected == Some(action))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.editor.wired_keymap_action = Some(action);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn keymap_restore_fn_defaults_button(
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        control_button("aula-keymap-restore-fn-defaults")
            .label(tr!("aula.keymap_restore_fn_defaults"))
            .disabled(!enabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.editor.restore_oem_fn_defaults();
                this.keymap_profile_error = None;
                cx.notify();
            }))
    }

    fn keymap_oem_fields_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        control_button("aula-keymap-oem-fields")
            .label(tr!("aula.keymap_oem_fields"))
            .disabled(self.selected_oem_fn_record().is_none())
            .on_click(cx.listener(|this, _, window, cx| {
                this.open_oem_fn_record_editor(window, cx);
            }))
    }

    fn keymap_reserved_warning(
        slot: Option<u8>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        div().children(slot.map(|slot| {
            div()
                .text_caption()
                .text_color(theme::palette(cx).text_muted)
                .child(tr!("aula.keymap_reserved_slot", slot = slot))
        }))
    }

    fn keymap_keyboard(
        index: usize,
        enabled: bool,
        fn_layer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let keys = AULA_F87_KEY_POSITIONS
            .into_iter()
            .map(|position| {
                Self::keymap_keycap(
                    position,
                    usize::from(position.matrix_slot()) == index,
                    enabled,
                    fn_layer,
                    cx,
                )
            })
            .collect::<Vec<_>>();
        h_flex()
            .w_full()
            .overflow_x_scrollbar()
            .id("aula-keymap-keyboard-scroll")
            .child(
                div()
                    .flex_none()
                    .relative()
                    .w(px(AULA_LAYOUT_WIDTH))
                    .h(px(AULA_LAYOUT_HEIGHT))
                    .children(keys),
            )
    }

    fn keymap_layer_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_fn = self.editor.keymap_fn_layer;
        let view = cx.entity().downgrade();
        control_button("aula-keymap-layer")
            .label(if selected_fn {
                tr!("aula.keymap_fn_layer")
            } else {
                tr!("aula.keymap_base_layer")
            })
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |menu, _, _| {
                let view_base = view.clone();
                let menu = menu.item(
                    PopupMenuItem::new(tr!("aula.keymap_base_layer"))
                        .checked(!selected_fn)
                        .on_click(move |_, _, cx| {
                            let _ = view_base.update(cx, |this, cx| {
                                this.editor.keymap_fn_layer = false;
                                this.editor.keymap_index = this.editor.keymap_index.min(125);
                                this.editor.keymap_action = None;
                                cx.notify();
                            });
                        }),
                );
                let view_fn = view.clone();
                menu.item(
                    PopupMenuItem::new(tr!("aula.keymap_fn_layer"))
                        .checked(selected_fn)
                        .on_click(move |_, _, cx| {
                            let _ = view_fn.update(cx, |this, cx| {
                                this.editor.keymap_fn_layer = true;
                                this.editor.keymap_action = None;
                                cx.notify();
                            });
                        }),
                )
            })
    }

    fn keymap_index_control(
        &self,
        enabled: bool,
        count: usize,
        fn_layer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.editor.keymap_index;
        let view = cx.entity().downgrade();
        control_button("aula-keymap-index")
            .label(tr!("aula.keymap_index", index = selected))
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for index in 0..count {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr!("aula.keymap_index", index = index))
                            .disabled(!aula_keymap_slot_is_assignable(index, fn_layer))
                            .checked(selected == index)
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.editor.keymap_index = index;
                                    this.editor.keymap_action = None;
                                    this.editor.wired_keymap_action = None;
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn keymap_action_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.editor.keymap_action;
        let selected_label = selected.map_or_else(
            || tr!("aula.keymap_choose_action").into(),
            |entry| {
                keymap_action_label(entry)
                    .unwrap_or_else(|| tr!("aula.keymap_custom_action").to_string())
            },
        );
        let view = cx.entity().downgrade();
        control_button("aula-keymap-action")
            .label(selected_label)
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for (label, entry) in keymap_actions() {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .checked(selected == Some(entry))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.editor.keymap_action = Some(entry);
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn keymap_action_editor_controls(
        &self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(self.keymap_action_control(enabled, cx))
            .child(
                control_button("aula-keymap-custom-action")
                    .label(tr!("aula.keymap_custom_action"))
                    .disabled(!enabled)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_custom_keymap_action(window, cx);
                    })),
            )
    }

    fn open_custom_keymap_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let current_action = self.editor.keymap_action;
        let input = cx.new(|cx| {
            let mut input =
                InputState::new(window, cx).placeholder(tr!("aula.keymap_raw_action_placeholder"));
            if let Some(action) = current_action {
                input.set_value(format_key_entry(action), window, cx);
            }
            input
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(440.))
                .title(tr!("aula.keymap_custom_action"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.keymap_raw_action_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    move |_, _, cx| {
                        let Some(action) =
                            parse_keymap_action_input(input.read(cx).value().as_ref())
                        else {
                            let _ = view.update(cx, |this, cx| {
                                this.keymap_profile_error =
                                    Some(tr!("aula.keymap_raw_action_invalid").to_string());
                                cx.notify();
                            });
                            return false;
                        };
                        let _ = view.update(cx, |this, cx| {
                            this.editor.keymap_action = Some(action);
                            this.keymap_profile_error = None;
                            cx.notify();
                        });
                        true
                    }
                })
        });
    }

    fn keymap_keycap(
        position: openhid_core::hid::aula::AulaKeyPosition,
        selected: bool,
        enabled: bool,
        fn_layer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let index = usize::from(position.matrix_slot());
        BaseButton::new(("aula-keymap-key", index))
            .role(Role::CheckBox)
            .selected(selected)
            .accessibility_label(format!("{} · keymap slot {index}", position.label()))
            .disabled(!enabled || !aula_keymap_slot_is_assignable(index, fn_layer))
            .absolute()
            .left(px(f32::from(position.x()) * AULA_LAYOUT_SCALE))
            .top(px(f32::from(position.y()) * AULA_LAYOUT_SCALE))
            .w(px(f32::from(position.width()) * AULA_LAYOUT_SCALE))
            .h(px(f32::from(position.height()) * AULA_LAYOUT_SCALE))
            .rounded(px(3.))
            .border_1()
            .border_color(if selected {
                theme::accent()
            } else {
                theme::palette(cx).border
            })
            .bg(theme::palette(cx).control)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.editor.keymap_index = index;
                this.editor.keymap_action = None;
                this.editor.wired_keymap_action = None;
                cx.notify();
            }))
            .child(
                div()
                    .text_size(px(8.))
                    .text_color(theme::palette(cx).text_primary)
                    .overflow_hidden()
                    .child(position.label()),
            )
    }

    fn per_key_profile_controls(&self, editable: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected_profile.clone();
        let selected_for_menu = selected.clone();
        let profiles = self.saved_profiles.clone();
        let view = cx.entity().downgrade();
        let profile_menu = control_button("aula-rgb-profile")
            .label(
                selected
                    .clone()
                    .unwrap_or_else(|| tr!("aula.per_key_choose_profile").to_string()),
            )
            .icon(IconName::ChevronDown)
            .disabled(self.saved_profiles.is_empty())
            .dropdown_menu(move |mut menu, _, _| {
                for saved in &profiles {
                    let name = saved.name.clone();
                    let profile = saved.profile.clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(name.clone())
                            .checked(selected_for_menu.as_deref() == Some(name.as_str()))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.editor.per_key_table = Some(profile.colors().clone());
                                    this.selected_profile = Some(name.clone());
                                    this.profile_error = None;
                                    this.editor.completed = None;
                                    cx.notify();
                                });
                            }),
                    );
                }
                menu
            });
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(profile_menu)
            .child(
                control_button("aula-rgb-profile-import")
                    .label(tr!("aula.per_key_import_profile"))
                    .on_click(cx.listener(|_, _, _, cx| Self::import_profile(cx))),
            )
            .child(
                control_button("aula-rgb-profile-export")
                    .label(tr!("aula.per_key_export_profile"))
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.export_profile(cx))),
            )
            .child(
                control_button("aula-rgb-profile-save")
                    .label(tr!("aula.per_key_save_profile"))
                    .disabled(!editable || self.editor.per_key_table.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_save_profile(window, cx);
                    })),
            )
            .child(
                control_button("aula-rgb-profile-delete")
                    .label(tr!("aula.per_key_delete_profile"))
                    .disabled(selected.is_none())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_delete_profile(window, cx);
                    })),
            )
    }

    fn import_profile(cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("aula.per_key_import_profile")),
        });
        cx.spawn(async move |view, cx| {
            let selected = match receiver.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) | Err(_) => None,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            let Some(path) = selected else {
                return;
            };
            match aula_profiles::import(&path) {
                Ok(profile) => {
                    let profiles = aula_profiles::load_all().unwrap_or_else(|error| {
                        tracing::warn!(%error, "could not reload local AULA RGB profiles");
                        Vec::new()
                    });
                    let _ = view.update(cx, |this, cx| {
                        this.editor.per_key_table = Some(profile.profile.colors().clone());
                        this.selected_profile = Some(profile.name);
                        this.saved_profiles = profiles;
                        this.profile_error = None;
                        this.editor.completed = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn export_profile(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self.selected_profile.as_deref() else {
            return;
        };
        let Some(profile) = self
            .saved_profiles
            .iter()
            .find(|profile| profile.name == name)
            .map(|profile| profile.profile.clone())
        else {
            return;
        };
        let directory = match aula_profiles::ensure_directory() {
            Ok(directory) => directory,
            Err(error) => {
                self.profile_error = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        let receiver = cx.prompt_for_new_path(&directory, Some("aula-rgb-profile.json"));
        cx.spawn(async move |view, cx| {
            let path = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    let _ = view.update(cx, |this, cx| {
                        this.profile_error = Some(error.to_string());
                        cx.notify();
                    });
                    return;
                }
            };
            match aula_profiles::export(&profile, &path) {
                Ok(()) => {
                    let _ = view.update(cx, |this, cx| {
                        this.profile_error = None;
                        cx.notify();
                    });
                }
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.profile_error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_save_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(table) = self.editor.per_key_table.clone() else {
            return;
        };
        let view = cx.entity().downgrade();
        let selected = self.selected_profile.clone();
        let input = cx.new(|cx| {
            let mut input =
                InputState::new(window, cx).placeholder(tr!("aula.per_key_profile_name"));
            if let Some(name) = selected {
                input.set_value(name, window, cx);
            }
            input
        });
        window.open_dialog(cx, move |dialog, window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
            dialog
                .w(px(420.))
                .title(tr!("aula.per_key_save_profile"))
                .child(
                    v_flex().gap_2().child(control_input(&input)).child(
                        div()
                            .text_caption()
                            .text_color(theme::palette(cx).text_muted)
                            .child(tr!("aula.per_key_profile_name_hint")),
                    ),
                )
                .footer(save_cancel_footer())
                .on_ok({
                    let input = input.clone();
                    let view = view.clone();
                    let table = table.clone();
                    move |_, _, cx| {
                        let name = input.read(cx).value().to_string();
                        let profile = AulaPerKeyRgbProfile::new(table.clone());
                        match aula_profiles::save(&name, &profile) {
                            Ok(()) => {
                                let _ = view.update(cx, |this, cx| {
                                    this.saved_profiles = aula_profiles::load_all().unwrap_or_else(|error| {
                                        tracing::warn!(%error, "could not reload local AULA RGB profiles");
                                        Vec::new()
                                    });
                                    this.selected_profile = Some(name.clone());
                                    this.profile_error = None;
                                    cx.notify();
                                });
                                true
                            }
                            Err(error) => {
                                let _ = view.update(cx, |this, cx| {
                                    this.profile_error = Some(error.to_string());
                                    cx.notify();
                                });
                                false
                            }
                        }
                    }
                })
        });
    }

    fn open_delete_profile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.selected_profile.clone() else {
            return;
        };
        let view = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(tr!("aula.per_key_delete_profile_confirmation", name => name.clone()))
                .description(tr!("aula.per_key_profile_delete_hint"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(tr!("aula.per_key_delete_profile"))
                        .ok_variant(ButtonVariant::Danger)
                        .cancel_text(tr!("common.cancel"))
                        .show_cancel(true),
                )
                .on_ok({
                    let name = name.clone();
                    let view = view.clone();
                    move |_, _, cx| {
                    match aula_profiles::delete(&name) {
                        Ok(()) => {
                            let _ = view.update(cx, |this, cx| {
                                this.saved_profiles = aula_profiles::load_all().unwrap_or_else(|error| {
                                    tracing::warn!(%error, "could not reload local AULA RGB profiles");
                                    Vec::new()
                                });
                                this.selected_profile = None;
                                this.profile_error = None;
                                cx.notify();
                            });
                            true
                        }
                        Err(error) => {
                            let _ = view.update(cx, |this, cx| {
                                this.profile_error = Some(error.to_string());
                                cx.notify();
                            });
                            false
                        }
                    }
                    }
                })
        });
    }

    fn per_key_color_control(&self, editable: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.editor.per_key_color;
        let view = cx.entity().downgrade();
        control_button("aula-per-key-color")
            .label(selected.map_or_else(|| tr!("aula.per_key_choose_color"), rgb_label))
            .icon(IconName::ChevronDown)
            .disabled(!editable)
            .dropdown_menu(move |mut menu, _, _| {
                for color in per_key_palette() {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(rgb_label(color))
                            .checked(selected == Some(color))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    if this.available() {
                                        this.editor.per_key_color = Some(color);
                                        this.editor.completed = None;
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn unmapped_led_control(
        &self,
        editable: bool,
        key_label: Option<&'static str>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.editor.per_key_index;
        let selected_is_mapped = key_label.is_some();
        let view = cx.entity().downgrade();
        control_button("aula-rgb-unmapped-led")
            .label(if selected_is_mapped {
                tr!("aula.per_key_unmapped")
            } else {
                tr!("aula.per_key_unmapped_selected", index = selected)
            })
            .icon(IconName::ChevronDown)
            .disabled(!editable)
            .dropdown_menu(move |mut menu, _, _| {
                for led_index in 0..AULA_PER_KEY_LED_COUNT {
                    if AULA_F87_KEY_POSITIONS
                        .iter()
                        .any(|key| usize::from(key.led_index()) == led_index)
                    {
                        continue;
                    }
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr!("aula.per_key_index", index = led_index))
                            .checked(selected == led_index)
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    if this.available() {
                                        this.editor.select_per_key(led_index, false);
                                        this.editor.completed = None;
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn per_key_keycap(
        position: openhid_core::hid::aula::AulaKeyPosition,
        color: AulaRgb,
        selected: bool,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let led_index = usize::from(position.led_index());
        BaseButton::new(("aula-rgb-key", u64::from(position.led_index())))
            .role(Role::CheckBox)
            .selected(selected)
            .accessibility_label(format!("{} · LED {}", position.label(), led_index))
            .aria_toggled(if selected {
                Toggled::True
            } else {
                Toggled::False
            })
            .disabled(!editable)
            .absolute()
            .left(px(f32::from(position.x()) * AULA_LAYOUT_SCALE))
            .top(px(f32::from(position.y()) * AULA_LAYOUT_SCALE))
            .w(px(f32::from(position.width()) * AULA_LAYOUT_SCALE))
            .h(px(f32::from(position.height()) * AULA_LAYOUT_SCALE))
            .rounded(px(3.))
            .border_1()
            .border_color(if selected {
                theme::accent()
            } else {
                theme::palette(cx).border
            })
            .bg(rgb(packed_rgb(color)))
            .cursor_pointer()
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                if this.available() {
                    this.editor
                        .select_per_key(led_index, event.modifiers().secondary());
                    cx.notify();
                }
            }))
            .child(
                div()
                    .text_size(px(8.))
                    .text_color(theme::palette(cx).text_primary)
                    .overflow_hidden()
                    .child(position.label()),
            )
    }

    fn effect_control(&self, enabled: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let effect = self.editor.effect;
        let label = effect.map_or_else(
            || {
                self.editor.current.map_or_else(
                    || tr!("aula.choose_effect"),
                    |state| effect_label(state.effect),
                )
            },
            effect_label,
        );
        let view = cx.entity().downgrade();
        control_button("aula-effect")
            .label(label)
            .icon(IconName::ChevronDown)
            .disabled(!enabled)
            .dropdown_menu(move |mut menu, _, _| {
                for value in LightingChange::effects() {
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(effect_label(value))
                            .checked(effect == Some(value))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    if this.available() && this.editor.current.is_some() {
                                        this.editor.select_effect(value);
                                        cx.notify();
                                    }
                                });
                            }),
                    );
                }
                menu
            })
    }

    fn levels(
        id: &'static str,
        selected: Option<u8>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let values: Vec<_> = LightingChange::levels().collect();
        ButtonGroup::new(id)
            .outline()
            .disabled(!enabled)
            .children(values.iter().map(|value| {
                Button::new((id, u64::from(*value)))
                    .label(value.to_string())
                    .selected(selected == Some(*value))
            }))
            .on_click(cx.listener(move |this, indices: &Vec<usize>, _, cx| {
                if !this.available() || this.editor.current.is_none() {
                    return;
                }
                let Some(value) = indices.first().and_then(|ix| values.get(*ix)).copied() else {
                    return;
                };
                match id {
                    "aula-brightness" => this.editor.brightness = Some(value),
                    "aula-speed" => this.editor.speed = Some(value),
                    _ => return,
                }
                this.editor.completed = None;
                cx.notify();
            }))
    }
}

fn per_key_palette() -> [AulaRgb; 8] {
    [
        AULA_OEM_GAMING_COLOR,
        AulaRgb::new(255, 96, 0),
        AulaRgb::new(255, 192, 0),
        AulaRgb::new(0, 255, 0),
        AulaRgb::new(0, 160, 255),
        AulaRgb::new(0, 0, 255),
        AulaRgb::new(192, 0, 255),
        AulaRgb::new(255, 255, 255),
    ]
}

fn keymap_actions() -> Vec<(String, [u8; 4])> {
    aula_keymap_action_outputs()
        .into_iter()
        .map(|output| (localized_keymap_action_label(&output), output.raw()))
        .collect()
}

fn localized_keymap_action_label(output: &AulaKeymapActionOutput) -> String {
    match output.name() {
        "unbound" => tr!("actions.unbound").to_string(),
        "disable_key" => tr!("aula.keymap_disable_key").to_string(),
        "fn_layer" => tr!("aula.keymap_fn_layer").to_string(),
        "media_play_pause" => tr!("actions.play_pause").to_string(),
        "media_next_track" => tr!("actions.next_track").to_string(),
        "media_previous_track" => tr!("actions.previous_track").to_string(),
        "media_mute" => tr!("actions.mute").to_string(),
        "media_volume_up" => tr!("actions.volume_up").to_string(),
        "media_volume_down" => tr!("actions.volume_down").to_string(),
        "media_play" => tr!("aula.keymap_consumer_play").to_string(),
        "media_pause" => tr!("aula.keymap_consumer_pause").to_string(),
        "media_record" => tr!("aula.keymap_consumer_record").to_string(),
        "media_fast_forward" => tr!("aula.keymap_consumer_fast_forward").to_string(),
        "media_rewind" => tr!("aula.keymap_consumer_rewind").to_string(),
        "media_stop" => tr!("aula.keymap_consumer_stop").to_string(),
        "media_eject" => tr!("aula.keymap_consumer_eject").to_string(),
        "media_random_play" => tr!("aula.keymap_consumer_random_play").to_string(),
        "media_repeat" => tr!("aula.keymap_consumer_repeat").to_string(),
        "media_stop_eject" => tr!("aula.keymap_consumer_stop_eject").to_string(),
        "media_play_skip" => tr!("aula.keymap_consumer_play_skip").to_string(),
        "media_voice_command" => tr!("aula.keymap_consumer_voice_command").to_string(),
        "bass_boost" => tr!("aula.keymap_consumer_bass_boost").to_string(),
        "surround_mode" => tr!("aula.keymap_consumer_surround_mode").to_string(),
        "loudness" => tr!("aula.keymap_consumer_loudness").to_string(),
        "mpx" => tr!("aula.keymap_consumer_mpx").to_string(),
        "display_brightness_up" => tr!("keyboard.brightness_up_key").to_string(),
        "display_brightness_down" => tr!("keyboard.brightness_down_key").to_string(),
        "alt_escape" => tr!("aula.keymap_alt_escape").to_string(),
        "alt_tab" => tr!("aula.keymap_alt_tab").to_string(),
        _ => output.label().to_owned(),
    }
}

fn keymap_action_label(entry: [u8; 4]) -> Option<String> {
    let actions = keymap_actions();
    if let Some((label, _)) = actions.iter().find(|(_, action)| *action == entry) {
        return Some(label.clone());
    }

    let matrix_action = AulaOemFnLayer::matrix_action_from_profile_action(entry);
    if matrix_action == entry {
        return None;
    }
    actions
        .into_iter()
        .find_map(|(label, action)| (action == matrix_action).then_some(label))
        .or_else(|| aula_keymap_action_label(entry))
}

fn format_key_entry(entry: [u8; 4]) -> String {
    format!(
        "{:02x} {:02x} {:02x} {:02x}",
        entry[0], entry[1], entry[2], entry[3]
    )
}

fn format_hex_bytes<const N: usize>(bytes: &[u8; N]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(N * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn parse_keymap_action_input(value: &str) -> Option<[u8; 4]> {
    if let Some(action) = aula_keymap_action_output(value) {
        return Some(action);
    }

    let mut digits = value.bytes().filter(|byte| !byte.is_ascii_whitespace());
    let mut action = [0; 4];
    for byte in &mut action {
        let high = keymap_hex_nibble(digits.next()?)?;
        let low = keymap_hex_nibble(digits.next()?)?;
        *byte = (high << 4) | low;
    }
    digits.next().is_none().then_some(action)
}

fn keymap_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const AULA_LAYOUT_SCALE: f32 = 0.82;
const AULA_LAYOUT_WIDTH: f32 = 520.;
const AULA_LAYOUT_HEIGHT: f32 = 185.;

fn packed_rgb(color: AulaRgb) -> u32 {
    let [red, green, blue] = color.channels();
    (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

fn rgb_label(color: AulaRgb) -> SharedString {
    let [red, green, blue] = color.channels();
    format!("#{red:02X}{green:02X}{blue:02X}").into()
}

fn color_mode_status(
    draft: Option<AulaColorModeChange>,
    current: Option<AulaColorModeState>,
) -> SharedString {
    match draft {
        Some(AulaColorModeChange::SingleColor(color)) => {
            format!("{} · {}", tr!("aula.single_color"), rgb_label(color)).into()
        }
        Some(AulaColorModeChange::Colorful) => tr!("aula.colorful"),
        None => match current {
            Some(AulaColorModeState::SingleColor) => tr!("aula.single_color"),
            Some(AulaColorModeState::Colorful) => tr!("aula.colorful"),
            Some(AulaColorModeState::Palette(value)) => {
                tr!("aula.color_mode_palette", value = value)
            }
            None => tr!("aula.color_mode_unknown"),
        },
    }
}

fn aula_rgb_from_picker(color: gpui::Hsla) -> Option<AulaRgb> {
    let digits = color.to_hex().strip_prefix('#')?.to_owned();
    if digits.len() < 6 {
        return None;
    }
    let channel = |offset| u8::from_str_radix(&digits[offset..offset + 2], 16).ok();
    Some(AulaRgb::new(channel(0)?, channel(2)?, channel(4)?))
}

fn effect_label(effect: u8) -> SharedString {
    if effect == 0 {
        tr!("aula.off")
    } else if effect == 21 {
        tr!("aula.custom_per_key")
    } else if LightingChange::supports_effect(effect) {
        let name = match effect {
            1 => tr!("aula.effect_1"),
            2 => tr!("aula.effect_2"),
            3 => tr!("aula.effect_3"),
            4 => tr!("aula.effect_4"),
            5 => tr!("aula.effect_5"),
            6 => tr!("aula.effect_6"),
            7 => tr!("aula.effect_7"),
            8 => tr!("aula.effect_8"),
            9 => tr!("aula.effect_9"),
            10 => tr!("aula.effect_10"),
            11 => tr!("aula.effect_11"),
            12 => tr!("aula.effect_12"),
            13 => tr!("aula.effect_13"),
            14 => tr!("aula.effect_14"),
            15 => tr!("aula.effect_15"),
            16 => tr!("aula.effect_16"),
            17 => tr!("aula.effect_17"),
            18 => tr!("aula.effect_18"),
            _ => tr!("aula.effect_number", number = effect),
        };
        format!("{effect} · {name}").into()
    } else {
        tr!("aula.unknown_effect", number = effect)
    }
}

impl Render for AulaLightingPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);
        let editable = self.available() && self.editor.current.is_some();
        let wired = self.target.as_ref().is_some_and(|target| target.wired);
        let writable = editable && self.editor.settings_change().is_some();
        let status = self.status();
        v_flex()
            .w_full()
            .gap_4()
            .when_some(self.editor.battery.as_ref(), |panel, battery| {
                panel.child(aula_battery_indicator(battery, pal))
            })
            .child(aula_description(pal, wired))
            .child(aula_basic_settings(self, cx))
            .child(self.side_light_section(editable, cx))
            .when(!wired, |panel| {
                panel
                    .child(self.per_key_controls(editable, cx))
                    .child(self.keymap_controls(self.available(), cx))
                    .child(self.macro_profile_controls(cx))
            })
            .when(wired, |panel| {
                panel.child(self.wired_keymap_controls(self.available(), cx))
            })
            .child(self.transaction_controls(writable, wired, cx))
            .child(
                div()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(tr!("aula.save_hint")),
            )
            .child(div().text_body().text_color(pal.text_muted).child(status))
            .children(self.editor.error.as_ref().map(|error| {
                div()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(error.to_string())
            }))
    }
}

fn aula_basic_settings(
    panel: &AulaLightingPanel,
    cx: &mut Context<AulaLightingPanel>,
) -> impl IntoElement {
    let wired = panel.target.as_ref().is_some_and(|target| target.wired);
    let editable = panel.available() && panel.editor.current.is_some();
    let capabilities = panel.editor.effect.and_then(aula_effect_capabilities);
    let brightness = editable && capabilities.is_some_and(AulaEffectCapabilities::has_brightness);
    let speed = editable && capabilities.is_some_and(AulaEffectCapabilities::has_speed);
    let palette = theme::palette(cx);
    let color_controls = capabilities
        .filter(|_| !wired)
        .filter(|value| value.has_custom_color() || value.has_random_color())
        .map(|value| panel.color_controls(editable, value, cx));
    v_flex()
        .gap_2()
        .child(div().text_body().child(tr!("aula.effect")))
        .child(panel.effect_control(editable, cx))
        .when(brightness, |controls| {
            controls.child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.brightness")))
                    .child(AulaLightingPanel::levels(
                        "aula-brightness",
                        panel.editor.brightness,
                        brightness,
                        cx,
                    )),
            )
        })
        .when(speed, |controls| {
            controls.child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.speed")))
                    .child(AulaLightingPanel::levels(
                        "aula-speed",
                        panel.editor.speed,
                        speed,
                        cx,
                    )),
            )
        })
        .children(color_controls)
        .when(brightness || speed, |controls| {
            controls.child(
                div()
                    .text_caption()
                    .text_color(palette.text_muted)
                    .child(tr!("aula.level_hint")),
            )
        })
        .child(
            v_flex()
                .gap_2()
                .child(div().text_body().child(tr!("aula.sleep")))
                .child(panel.sleep_control(editable, cx))
                .child(
                    div()
                        .text_caption()
                        .text_color(palette.text_muted)
                        .child(tr!("aula.sleep_hint")),
                ),
        )
        .when(!wired, |controls| {
            controls.child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.debounce")))
                    .child(panel.debounce_control(editable, cx))
                    .child(
                        div()
                            .text_caption()
                            .text_color(palette.text_muted)
                            .child(tr!("aula.debounce_hint")),
                    ),
            )
        })
}

fn side_light_effect_label(effect: AulaSideLightEffect) -> SharedString {
    match effect {
        AulaSideLightEffect::Off => tr!("aula.side_light_off"),
        AulaSideLightEffect::RainbowWave => tr!("aula.side_light_rainbow_wave"),
        AulaSideLightEffect::RainbowTogether => tr!("aula.side_light_rainbow_together"),
        AulaSideLightEffect::Solid => tr!("aula.side_light_solid"),
        AulaSideLightEffect::Breathing => tr!("aula.side_light_breathing"),
    }
}

fn aula_battery_indicator(battery: &AulaBattery, palette: theme::Palette) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(BatteryIndicator::summary(&battery.info))
        .child(div().text_caption().text_color(palette.text_muted).child(
            match battery.power_source {
                AulaBatteryPowerSource::Battery => tr!("aula.on_battery"),
                AulaBatteryPowerSource::Cable => tr!("aula.cable_power"),
            },
        ))
}

fn aula_description(palette: theme::Palette, wired: bool) -> impl IntoElement {
    div()
        .text_body()
        .text_color(palette.text_muted)
        .child(if wired {
            tr!("aula.wired_description")
        } else {
            tr!("aula.description")
        })
}

fn macro_tree_node_path_label(tree: &AulaOemMacroTree, node_index: usize) -> String {
    let path = tree
        .node_path_indices(node_index)
        .unwrap_or_else(|_| vec![node_index]);
    path.iter()
        .map(|path_index| {
            let path_node = &tree.nodes()[*path_index];
            path_node
                .name()
                .unwrap_or_else(|| format!("#{}", path_index + 1))
        })
        .collect::<Vec<_>>()
        .join(" / ")
}

fn macro_event_kind_label(kind: AulaOemMacroEventKind) -> String {
    match kind {
        AulaOemMacroEventKind::EmptyRow => tr!("aula.macro_kind_empty_row").to_string(),
        AulaOemMacroEventKind::MouseLeftButton => {
            tr!("aula.macro_kind_mouse_left_button").to_string()
        }
        AulaOemMacroEventKind::MappedInput => tr!("aula.macro_kind_layout_mapped").to_string(),
        AulaOemMacroEventKind::Delay => tr!("aula.macro_kind_delay").to_string(),
        AulaOemMacroEventKind::SpecialPayload => tr!("aula.macro_kind_special_payload").to_string(),
        AulaOemMacroEventKind::Unknown => tr!("aula.macro_kind_unknown").to_string(),
    }
}

fn macro_event_dialog_target(
    macro_file: &AulaOemMacroFile,
    selected_index: usize,
    edit_index: Option<usize>,
) -> Option<(usize, usize, Option<[u8; 12]>)> {
    let visible_index = edit_index.unwrap_or_else(|| {
        selected_index
            .saturating_add(1)
            .min(macro_file.oem_editor_event_count())
    });
    let raw_index = match edit_index {
        Some(index) => macro_file.oem_editor_record_index(index)?,
        None => macro_file.oem_editor_insert_record_index(visible_index),
    };
    let record = edit_index.and_then(|_| macro_file.records().get(raw_index).copied());
    Some((visible_index, raw_index, record))
}

fn macro_event_field_inputs(
    record: Option<[u8; 12]>,
    window: &mut Window,
    cx: &mut Context<AulaLightingPanel>,
) -> [Entity<InputState>; 5] {
    let values = macro_event_field_values(record);

    std::array::from_fn(|index| {
        let value = values[index].clone();
        cx.new(|cx| {
            let placeholder = if index == 4 { "00000000" } else { "0000" };
            let mut input = InputState::new(window, cx).placeholder(placeholder);
            if !value.is_empty() {
                input.set_value(value, window, cx);
            }
            input
        })
    })
}

fn macro_event_field_values(record: Option<[u8; 12]>) -> [String; 5] {
    record.map_or_else(
        || {
            [
                String::new(),
                "0000".to_owned(),
                "0000".to_owned(),
                "0000".to_owned(),
                "00000000".to_owned(),
            ]
        },
        |record| {
            let event = AulaOemMacroEvent::from_record(record);
            let reserved = event.reserved();
            [
                format!("{:04X}", event.event_code()),
                format!("{:04X}", event.auxiliary()),
                format!("{:04X}", event.value()),
                format!("{:02X}{:02X}", reserved[0], reserved[1]),
                format!("{:08X}", event.special_value()),
            ]
        },
    )
}

fn macro_event_field(label: SharedString, input: &Entity<InputState>) -> impl IntoElement {
    v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(div().text_caption().child(label))
        .child(control_input(input))
}

fn oem_fn_combo_inputs(
    combination: AulaOemKeyCombination,
    window: &mut Window,
    cx: &mut App,
) -> [Entity<InputState>; 3] {
    let values = [
        format!("{:02X}", combination.modify()),
        format!("{:02X}", combination.vk1()),
        format!("{:04X}", combination.vk2()),
    ];
    std::array::from_fn(|index| {
        let value = values[index].clone();
        cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(match index {
                2 => "0000",
                _ => "00",
            });
            input.set_value(value, window, cx);
            input
        })
    })
}

fn oem_fn_record_editor_content(
    input: &Entity<InputState>,
    combo_inputs: Option<&[Entity<InputState>; 3]>,
    error: Option<String>,
    cx: &App,
) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(control_input(input))
        .child(
            div()
                .text_caption()
                .text_color(theme::palette(cx).text_muted)
                .child(tr!("aula.keymap_oem_fields_hint")),
        )
        .when_some(combo_inputs, |panel, inputs| {
            panel
                .child(
                    h_flex()
                        .gap_2()
                        .child(macro_event_field(
                            tr!("aula.keymap_combo_modify"),
                            &inputs[0],
                        ))
                        .child(macro_event_field(tr!("aula.keymap_combo_vk1"), &inputs[1]))
                        .child(macro_event_field(tr!("aula.keymap_combo_vk2"), &inputs[2])),
                )
                .child(
                    div()
                        .text_caption()
                        .text_color(theme::palette(cx).text_muted)
                        .child(tr!("aula.keymap_combo_offline_hint")),
                )
        })
        .when_some(error, |element, error| {
            element
                .text_caption()
                .text_color(theme::palette(cx).text_primary)
                .child(error)
        })
}

fn parse_oem_fn_combo_parameter(
    inputs: &[Entity<InputState>; 3],
    cx: &App,
) -> Option<AulaOemKeyCombination> {
    let modify = inputs[0].read(cx).value().to_string();
    let vk1 = inputs[1].read(cx).value().to_string();
    let vk2 = inputs[2].read(cx).value().to_string();
    let ([modify], [vk1], vk2) = (
        parse_hex_field_bytes::<1>(&modify)?,
        parse_hex_field_bytes::<1>(&vk1)?,
        parse_hex_field(&vk2, 4).and_then(|value| u16::try_from(value).ok())?,
    );
    Some(AulaOemKeyCombination::new(modify, vk1, vk2))
}

fn save_oem_fn_profile_record(
    target: &OemFnRecordTarget,
    input: &Entity<InputState>,
    combination_inputs: Option<&[Entity<InputState>; 3]>,
    dialog_error: &Entity<Option<String>>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let error = tr!("aula.keymap_oem_fields_invalid").to_string();
    let value = input.read(cx).value().to_string();
    let Some(record) = parse_hex_field_bytes::<12>(&value) else {
        dialog_error.update(cx, |message, cx| {
            *message = Some(error.clone());
            cx.notify();
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        let _ = target.view.update(cx, |this, cx| {
            this.keymap_profile_error = Some(error);
            cx.notify();
        });
        return false;
    };
    let combination = if let Some(inputs) = combination_inputs {
        let Some(combination) = parse_oem_fn_combo_parameter(inputs, cx) else {
            dialog_error.update(cx, |message, cx| {
                *message = Some(error.clone());
                cx.notify();
            });
            inputs[0].update(cx, |input, cx| input.focus(window, cx));
            return false;
        };
        Some(combination)
    } else {
        None
    };
    let mut updated = false;
    let _ = target.view.update(cx, |this, cx| {
        let saved_profile = this
            .saved_keymap_profiles
            .iter_mut()
            .find(|profile| profile.name == target.profile_name);
        let record_saved = saved_profile
            .and_then(|profile| profile.oem_fn_records.as_mut())
            .is_some_and(|records| {
                records
                    .replace_record(target.record_index, record)
                    .is_some()
                    && combination.is_none_or(|combination| {
                        records
                            .replace_key_combination(target.record_index, combination)
                            .is_some()
                    })
            });
        if record_saved {
            this.keymap_profile_error = None;
            updated = true;
        } else {
            this.keymap_profile_error = Some(error);
        }
        cx.notify();
    });
    updated
}

fn macro_event_form(
    inputs: &[Entity<InputState>; 5],
    inserting: bool,
    error: Option<String>,
    cx: &App,
) -> impl IntoElement {
    let palette = theme::palette(cx);
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(macro_event_type_control(inputs, inserting))
                .child(macro_event_key_control(inputs)),
        )
        .child(
            h_flex()
                .gap_2()
                .child(macro_event_field(tr!("aula.macro_event_code"), &inputs[0]))
                .child(macro_event_field(
                    tr!("aula.macro_event_auxiliary"),
                    &inputs[1],
                )),
        )
        .child(
            h_flex()
                .gap_2()
                .child(macro_event_field(tr!("aula.macro_event_value"), &inputs[2]))
                .child(macro_event_field(
                    tr!("aula.macro_event_reserved"),
                    &inputs[3],
                )),
        )
        .child(macro_event_field(
            tr!("aula.macro_event_special_value"),
            &inputs[4],
        ))
        .child(
            div()
                .text_caption()
                .text_color(palette.text_muted)
                .child(tr!("aula.macro_event_edit_hint")),
        )
        .child(div().when_some(error, |element, error| {
            element
                .text_caption()
                .text_color(palette.text_primary)
                .child(error)
        }))
}

fn macro_event_type_control(inputs: &[Entity<InputState>; 5], inserting: bool) -> impl IntoElement {
    let inputs = inputs.clone();
    control_button("aula-macro-event-type")
        .label(tr!("aula.macro_event_choose_type"))
        .icon(IconName::ChevronDown)
        .dropdown_menu(move |mut menu, _, _| {
            for (code, kind) in AulaOemMacroEvent::selector_events().iter().copied() {
                let item_inputs = inputs.clone();
                menu = menu.item(
                    PopupMenuItem::new(macro_event_selector_label(code, kind)).on_click(
                        move |_, window, cx| {
                            let has_default_payload = item_inputs[1..]
                                .iter()
                                .zip(["0000", "0000", "0000", "00000000"])
                                .all(|(input, expected)| input.read(cx).value() == expected);
                            if inserting
                                && has_default_payload
                                && let Some(record) =
                                    AulaOemMacroEvent::observed_record_template(code)
                            {
                                let values = macro_event_field_values(Some(record));
                                for (input, value) in item_inputs.iter().zip(values) {
                                    input.update(cx, |input, cx| {
                                        input.set_value(value, window, cx);
                                    });
                                }
                            } else {
                                item_inputs[0].update(cx, |input, cx| {
                                    input.set_value(format!("{code:04X}"), window, cx);
                                });
                            }
                        },
                    ),
                );
            }
            menu
        })
}

fn macro_event_key_control(inputs: &[Entity<InputState>; 5]) -> impl IntoElement {
    let inputs = inputs.clone();
    control_button("aula-macro-event-key")
        .label(tr!("aula.macro_event_choose_keyboard_key"))
        .icon(IconName::ChevronDown)
        .dropdown_menu(move |mut menu, _, _| {
            let mut virtual_keys = aula_oem_macro_mapped_virtual_keys().to_vec();
            virtual_keys.sort_unstable();
            for (virtual_key, _) in virtual_keys {
                let item_inputs = inputs.clone();
                menu = menu.item(
                    PopupMenuItem::new(macro_virtual_key_selector_label(virtual_key)).on_click(
                        move |_, window, cx| {
                            item_inputs[0].update(cx, |input, cx| {
                                input.set_value(format!("{virtual_key:04X}"), window, cx);
                            });
                        },
                    ),
                );
            }
            menu
        })
}

fn macro_virtual_key_selector_label(virtual_key: u8) -> String {
    let name = match virtual_key {
        b'0'..=b'9' | b'A'..=b'Z' => char::from(virtual_key).to_string(),
        0x08 => "Backspace".to_owned(),
        0x09 => "Tab".to_owned(),
        0x0d => "Enter".to_owned(),
        0x10 => "Shift".to_owned(),
        0x11 => "Ctrl".to_owned(),
        0x12 => "Alt".to_owned(),
        0x13 => "Pause".to_owned(),
        0x14 => "Caps Lock".to_owned(),
        0x1b => "Escape".to_owned(),
        0x20 => "Space".to_owned(),
        0x21 => "Page Up".to_owned(),
        0x22 => "Page Down".to_owned(),
        0x23 => "End".to_owned(),
        0x24 => "Home".to_owned(),
        0x25 => "Left Arrow".to_owned(),
        0x26 => "Up Arrow".to_owned(),
        0x27 => "Right Arrow".to_owned(),
        0x28 => "Down Arrow".to_owned(),
        0x2c => "Print Screen".to_owned(),
        0x2d => "Insert".to_owned(),
        0x2e => "Delete".to_owned(),
        0x5b => "Left Windows".to_owned(),
        0x5c => "Right Windows".to_owned(),
        0x5d => "Menu".to_owned(),
        0x60..=0x69 => format!("Numpad {}", virtual_key - 0x60),
        0x6a => "Numpad *".to_owned(),
        0x6b => "Numpad +".to_owned(),
        0x6d => "Numpad -".to_owned(),
        0x6e => "Numpad .".to_owned(),
        0x6f => "Numpad /".to_owned(),
        0x70..=0x87 => format!("F{}", virtual_key - 0x70 + 1),
        _ => format!("VK 0x{virtual_key:02X}"),
    };
    format!("{name} (VK 0x{virtual_key:02X})")
}

fn macro_event_selector_label(code: u16, kind: AulaOemMacroEventKind) -> String {
    match kind {
        AulaOemMacroEventKind::SpecialPayload => {
            format!("{} (0x{code:04X})", tr!("aula.macro_kind_special_payload"))
        }
        AulaOemMacroEventKind::MouseLeftButton => {
            format!(
                "{} (0x{code:04X})",
                tr!("aula.macro_kind_mouse_left_button")
            )
        }
        _ => macro_event_kind_label(kind),
    }
}

fn macro_event_field_label(index: usize) -> String {
    match index {
        0 => tr!("aula.macro_event_code").to_string(),
        1 => tr!("aula.macro_event_auxiliary").to_string(),
        2 => tr!("aula.macro_event_value").to_string(),
        3 => tr!("aula.macro_event_reserved").to_string(),
        _ => tr!("aula.macro_event_special_value").to_string(),
    }
}

fn parse_macro_event_fields(values: &[String; 5]) -> Result<[u8; 12], usize> {
    let event_code = u16::try_from(parse_hex_field(&values[0], 4).ok_or(0_usize)?)
        .ok()
        .ok_or(0_usize)?;
    let auxiliary = u16::try_from(parse_hex_field(&values[1], 4).ok_or(1_usize)?)
        .ok()
        .ok_or(1_usize)?;
    let value = u16::try_from(parse_hex_field(&values[2], 4).ok_or(2_usize)?)
        .ok()
        .ok_or(2_usize)?;
    let reserved = parse_hex_field_bytes::<2>(&values[3]).ok_or(3_usize)?;
    let special_value = parse_hex_field(&values[4], 8).ok_or(4_usize)?;
    Ok(
        AulaOemMacroEvent::from_fields(event_code, auxiliary, value, reserved, special_value)
            .to_record(),
    )
}

fn parse_hex_field(value: &str, digits: usize) -> Option<u32> {
    if value.len() != digits || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(value, 16).ok()
}

fn parse_hex_field_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests;
