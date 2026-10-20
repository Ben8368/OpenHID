//! F87 PRO built-in lighting, using agent-owned snapshot-preserving transactions.

use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div,
};
use gpui_component::{
    Disableable as _, IconName, Selectable as _,
    button::{Button, ButtonGroup, ButtonVariants as _},
    h_flex,
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use openhid_core::hid::aula::{
    AulaChange, AulaState, LightingChange, LightingMode, LightingState, SleepTimeout,
    route_identity,
};
use openhid_core::hid::{DeviceRoute, WriteError};
use tokio::sync::oneshot;

use crate::state::{AppState, StateEvent};
use crate::ui::components::control_button;
use crate::ui::theme::{self, Typography as _};

#[derive(Clone, PartialEq, Eq)]
struct Target {
    route: DeviceRoute,
    available: bool,
}

#[derive(Default)]
struct Editor {
    current: Option<LightingState>,
    effect: Option<u8>,
    brightness: Option<u8>,
    speed: Option<u8>,
    current_sleep: Option<SleepTimeout>,
    sleep: Option<SleepTimeout>,
    error: Option<WriteError>,
    completed: Option<LightingMode>,
}

impl Editor {
    fn received_settings(&mut self, state: AulaState) {
        self.received(state.lighting);
        self.current_sleep = state.sleep_timeout;
        self.sleep = state.sleep_timeout;
        self.completed = None;
    }

    fn settings_change(&self) -> Option<AulaChange> {
        self.current?;
        let lighting = self.change().filter(|change| {
            self.current.is_some_and(|current| {
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
        AulaChange::new(
            lighting.or_else(|| sleep.is_none().then(|| self.change()).flatten()),
            sleep,
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
        self.error = None;
    }

    fn select_effect(&mut self, effect: u8) {
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
        LightingChange::new(
            Some(effect),
            (effect != 0).then_some(self.brightness).flatten(),
            (effect != 0).then_some(self.speed).flatten(),
        )
        .ok()
    }
}

/// The editor owns its draft and in-flight identity. Hardware and recovery live
/// in the agent; selecting another route invalidates late replies here.
pub struct AulaLightingPanel {
    target: Option<Target>,
    editor: Editor,
    pending: bool,
    selection_epoch: u64,
    _state_obs: Subscription,
}

impl AulaLightingPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
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
            _state_obs: state_obs,
        };
        this.sync_target(cx);
        this
    }

    fn sync_target(&mut self, cx: &mut Context<Self>) {
        let target = AppState::try_read(cx).and_then(|state| {
            let record = state.current_record()?;
            let route = record.route.as_ref()?;
            route_identity(route)?;
            Some(Target {
                route: route.clone(),
                available: record.online && state.agent_status().is_some(),
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
        let reply = state.read_aula_settings(target.route.clone());
        self.wait(target, None, reply, cx);
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
        let reply = state.set_aula_settings(target.route.clone(), change, mode);
        self.wait(target, Some(mode), reply, cx);
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
                            this.editor.received_settings(state);
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

    fn transaction_controls(&self, writable: bool, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_2()
            .flex_wrap()
            .child(
                control_button("aula-refresh")
                    .label(tr!("aula.refresh"))
                    .disabled(!self.available())
                    .on_click(cx.listener(|this, _, _, cx| this.read(cx))),
            )
            .child(
                control_button("aula-apply")
                    .label(tr!("aula.apply"))
                    .disabled(!writable)
                    .on_click(cx.listener(|this, _, _, cx| this.write(LightingMode::Apply, cx))),
            )
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
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr!("aula.sleep_seconds", seconds = value.seconds()))
                            .checked(selected == Some(value))
                            .on_click(move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
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

fn effect_label(effect: u8) -> SharedString {
    if effect == 0 {
        tr!("aula.off")
    } else if LightingChange::supports_effect(effect) {
        tr!("aula.effect_number", number = effect)
    } else {
        tr!("aula.unknown_effect", number = effect)
    }
}

impl Render for AulaLightingPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = theme::palette(cx);
        let editable = self.available() && self.editor.current.is_some();
        let parameters = editable && self.editor.effect.is_some_and(|effect| effect != 0);
        let writable = editable && self.editor.settings_change().is_some();
        let status = self.status();
        v_flex()
            .w_full()
            .gap_4()
            .child(
                div()
                    .text_body()
                    .text_color(pal.text_muted)
                    .child(tr!("aula.description")),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.effect")))
                    .child(self.effect_control(editable, cx)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.brightness")))
                    .child(Self::levels(
                        "aula-brightness",
                        self.editor.brightness,
                        parameters,
                        cx,
                    )),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.speed")))
                    .child(Self::levels(
                        "aula-speed",
                        self.editor.speed,
                        parameters,
                        cx,
                    )),
            )
            .child(
                div()
                    .text_caption()
                    .text_color(pal.text_muted)
                    .child(tr!("aula.level_hint")),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_body().child(tr!("aula.sleep")))
                    .child(self.sleep_control(editable, cx))
                    .child(
                        div()
                            .text_caption()
                            .text_color(pal.text_muted)
                            .child(tr!("aula.sleep_hint")),
                    ),
            )
            .child(self.transaction_controls(writable, cx))
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

#[cfg(test)]
mod tests;
