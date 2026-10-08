// Spectral Compressor: an FFT based compressor
// Copyright (C) 2021-2024 Robbert van der Helm
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use atomic_float::AtomicF32;
use crossbeam::atomic::AtomicCell;
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use self::analyzer::Analyzer;
use self::mode_button::EditorModeButton;
use self::relative_slider::{RelativeParamSlider, RelativeParamSliderExt};
use crate::analyzer::AnalyzerData;
use crate::{SpectralCompressor, SpectralCompressorParams};

mod analyzer;
mod mode_button;
mod relative_slider;
mod response_graph;
mod zoom;
mod settings;

/// The entire GUI's width, in logical pixels.
const EXPANDED_GUI_WIDTH: u32 = 1360;
/// The width of the GUI's main part containing the controls.
const COLLAPSED_GUI_WIDTH: u32 = 680;
/// The entire GUI's height, in logical pixels.
const GUI_HEIGHT: u32 = 615;
// I couldn't get `LayoutType::Grid` to work as expected, so we'll fake a 4x4 grid with
// hardcoded column widths
const COLUMN_WIDTH: Units = Pixels(330.0);

const CREDIT_FONT: &str = "Turbo Credit";

/// The editor's mode. Essentially just a boolean to indicate whether the analyzer is shown or
/// not.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorMode {
    // These serialization names are hardcoded so the variants can be renamed them later without
    // breaking preset compatibility
    #[serde(rename = "collapsed")]
    Collapsed,
    #[default]
    #[serde(rename = "analyzer-visible")]
    AnalyzerVisible,
}

#[derive(Clone, Lens)]
pub struct Data {
    pub(crate) params: Arc<SpectralCompressorParams>,

    /// Determines which parts of the GUI are visible, and in turn decides the GUI's size.
    pub(crate) editor_mode: Arc<AtomicCell<EditorMode>>,

    pub(crate) analyzer_data: Arc<Mutex<triple_buffer::Output<AnalyzerData>>>,
    /// Used by the analyzer to determine which FFT bins belong to which frequencies.
    pub(crate) sample_rate: Arc<AtomicF32>,
}

impl Model for Data {}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state(editor_mode: Arc<AtomicCell<EditorMode>>) -> Arc<ViziaState> {
    ViziaState::new(move || match editor_mode.load() {
        EditorMode::Collapsed => (COLLAPSED_GUI_WIDTH, GUI_HEIGHT),
        EditorMode::AnalyzerVisible => (EXPANDED_GUI_WIDTH, GUI_HEIGHT),
    })
}

pub(crate) fn create(editor_state: Arc<ViziaState>, editor_data: Data) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_thin(cx);
        assets::register_noto_sans_regular(cx);
        cx.add_font_mem(include_bytes!("editor/fonts/TurboCredit-Regular.ttf"));

        if let Err(err) = cx.add_stylesheet(crate::palette::PaletteStyle(editor_data.params.palette.clone())) {
            nih_error!("Failed to load stylesheet: {err:?}")
        }

        editor_data.clone().build(cx);
        settings::Settings::new(&editor_data.params).build(cx);

        HStack::new(cx, |cx| {
            main_column(cx);

            let analyzer_visible = Data::editor_mode
                .map(|editor_mode| editor_mode.load() == EditorMode::AnalyzerVisible);
            Binding::new(cx, analyzer_visible, |cx, analyzer_visible| {
                if analyzer_visible.get(cx) {
                    analyzer_column(cx);
                }
            });
        });

        settings::build(cx);
        ResizeHandle::new(cx);
    })
}

fn main_column(cx: &mut Context) {
    VStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "Spectral Compressor Turbo").class("plugin-title")
                    .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
                    .font_weight(FontWeightKeyword::Regular).font_size(23.0);
                Label::new(cx, SpectralCompressor::VERSION).class("version")
                    .font_size(10.0).top(Stretch(1.0)).bottom(Pixels(3.0));
                Element::new(cx).width(Stretch(1.0));
                ParamButton::new(cx,Data::params,|p|&p.delta).with_label("DELTA")
                    .disable_scroll_wheel().id("delta-toggle").font_size(11.0)
                    .height(Pixels(25.0)).width(Pixels(58.0));
                Button::new(cx,|cx|cx.emit(settings::SettingsEvent::Toggle),|cx|Label::new(cx,"Settings"))
                    .id("open-settings").font_size(11.0).height(Pixels(25.0)).width(Pixels(70.0));
            }).height(Pixels(30.0)).col_between(Pixels(7.0));
            HStack::new(cx, |cx| {
                Label::new(cx,"by ＦＥＲＲＯ").class("creator-credit")
                    .font_family(vec![FamilyOwned::Name(String::from(CREDIT_FONT))])
                    .font_weight(FontWeightKeyword::Regular).font_size(12.0);
                Label::new(cx,"www.instagram.com/ferropop").class("social-credit").font_size(11.0);
                Element::new(cx).width(Stretch(1.0));
                EditorModeButton::new(cx,Data::editor_mode,Data::editor_mode.map(|m|if m.load()==EditorMode::AnalyzerVisible {"Hide response"}else{"Show response"}))
                    .font_size(11.0).height(Pixels(23.0));
                zoom::GuiZoom::new(cx);
            }).height(Pixels(25.0)).col_between(Pixels(12.0));
        }).class("title-area").height(Pixels(65.0))
            .right(Pixels(12.0)).bottom(Pixels(8.0)).left(Pixels(12.0)).top(Pixels(10.0));

        HStack::new(cx, |cx| {
            make_column(cx, "Globals", |cx| {
                relative_ui(cx, Data::params.map(|p| p.global.clone()));
            });

            make_column(cx, "Threshold", |cx| {
                ParamButton::new(cx,Data::params,|p|&p.auto_gain_enabled)
                    .with_label("Auto Gain Compensation").disable_scroll_wheel()
                    .id("auto-gain-toggle").height(Pixels(25.0)).font_size(12.0)
                    .top(Pixels(10.0)).left(Pixels(8.0)).right(Pixels(8.0)).bottom(Pixels(6.0));
                relative_ui(cx, Data::params.map(|p| p.threshold.clone()));

                Label::new(
                    cx,
                    "Parameter ranges and overal gain staging are still subject to change. If you \
                     use this in a project, make sure to bounce things to audio just in case \
                     they'll sound different later.",
                )
                .text_wrap(true)
                .font_size(11.0)
                .left(Pixels(15.0))
                .right(Pixels(8.0))
                .width(Stretch(1.0));
            });
        })
        .size(Auto);

        HStack::new(cx, |cx| {
            make_column(cx, "Upwards", |cx| {
                // We don't want to show the 'Upwards' prefix here, but it should still be in
                // the parameter name so the parameter list makes sense
                let upwards_compressor_params = Data::params.map(|p| p.compressors.upwards.clone());
                GenericUi::new_custom(cx, upwards_compressor_params, |cx, param_ptr| {
                    HStack::new(cx, |cx| {
                        Label::new(
                            cx,
                            unsafe { param_ptr.name() }
                                .strip_prefix("Upwards ")
                                .expect("Expected parameter name prefix, this is a bug"),
                        )
                        .class("label");

                        relative_widget(cx, upwards_compressor_params, param_ptr);
                    })
                    .class("row");
                });
            });

            make_column(cx, "Downwards", |cx| {
                let downwards_compressor_params =
                    Data::params.map(|p| p.compressors.downwards.clone());
                GenericUi::new_custom(cx, downwards_compressor_params, |cx, param_ptr| {
                    HStack::new(cx, |cx| {
                        Label::new(
                            cx,
                            unsafe { param_ptr.name() }
                                .strip_prefix("Downwards ")
                                .expect("Expected parameter name prefix, this is a bug"),
                        )
                        .class("label");

                        relative_widget(cx, downwards_compressor_params, param_ptr);
                    })
                    .class("row");
                });
            });
        })
        .size(Auto);
    })
    .width(Pixels(COLLAPSED_GUI_WIDTH as f32))
    .row_between(Pixels(10.0))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

// Keep every section on the same widget factory, including the Ratio controls.
fn relative_ui<L, PsRef, Ps>(cx: &mut Context, params: L)
where
    L: Lens<Target = PsRef> + Clone,
    PsRef: AsRef<Ps> + 'static,
    Ps: Params + 'static,
{
    GenericUi::new_custom(cx, params, move |cx, param_ptr| {
        HStack::new(cx, |cx| {
            Label::new(cx, unsafe { param_ptr.name() }).class("label");
            relative_widget(cx, params, param_ptr);
        })
        .class("row");
    });
}

fn relative_widget<L, PsRef, Ps>(cx: &mut Context, params: L, param_ptr: ParamPtr)
where
    L: Lens<Target = PsRef>,
    PsRef: AsRef<Ps> + 'static,
    Ps: Params + 'static,
{
    unsafe {
        match param_ptr {
            ParamPtr::FloatParam(p) => RelativeParamSlider::new(cx, params, move |_| &*p),
            ParamPtr::IntParam(p) => RelativeParamSlider::new(cx, params, move |_| &*p),
            ParamPtr::BoolParam(p) => RelativeParamSlider::new(cx, params, move |_| &*p),
            ParamPtr::EnumParam(p) => RelativeParamSlider::new(cx, params, move |_| &*p),
        }
    }
    .set_style(match unsafe { param_ptr.step_count() } {
        Some(step_count) if step_count <= 1 => ParamSliderStyle::CurrentStepLabeled { even: true },
        Some(step_count) if step_count <= 2 => ParamSliderStyle::CurrentStep { even: true },
        Some(_) => ParamSliderStyle::FromLeft,
        None => ParamSliderStyle::Centered,
    })
    .class("widget");
}

fn analyzer_column(cx: &mut Context) {
    Analyzer::new(cx, Data::analyzer_data, Data::sample_rate, Data::params)
        // These arbitrary 12 pixels are to align with the analyzer toggle botton
        .space(Pixels(12.0))
        .bottom(Pixels(12.0))
        .left(Pixels(2.0))
        .top(Pixels(12.0));
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    #[cfg(target_os = "macos")]
    const RESET_MODIFIER: Modifiers = Modifiers::LOGO;
    #[cfg(not(target_os = "macos"))]
    const RESET_MODIFIER: Modifiers = Modifiers::CTRL;
    #[cfg(target_os = "macos")]
    const OTHER_MODIFIER: Modifiers = Modifiers::CTRL;
    #[cfg(not(target_os = "macos"))]
    const OTHER_MODIFIER: Modifiers = Modifiers::LOGO;
    use nih_plug_vizia::vizia::context::backend::BackendContext;
    use nih_plug_vizia::widgets::RawParamEvent;

    #[derive(Debug)]
    enum Message {
        Begin(ParamPtr),
        Set(ParamPtr, f32),
        End(ParamPtr),
    }

    struct Recorder(Arc<Mutex<Vec<Message>>>);
    impl Model for Recorder {
        fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
            event.map(|raw, _| {
                let message = match raw {
                    RawParamEvent::BeginSetParameter(p) => Message::Begin(*p),
                    RawParamEvent::SetParameterNormalized(p, value) => Message::Set(*p, *value),
                    RawParamEvent::EndSetParameter(p) => Message::End(*p),
                    RawParamEvent::ParametersChanged => return,
                };
                self.0.lock().unwrap().push(message);
            });
        }
    }

    fn inspect(cx: &mut EventContext, sliders: &mut Vec<(Entity, String)>) {
        assert!(cx.get_view::<ParamSlider>().is_none(), "Original absolute slider remains in editor");
        if let Some(slider) = cx.get_view::<RelativeParamSlider>() {
            sliders.push((cx.current(), slider.parameter_name().to_string()));
        }
        let mut index = 0;
        while let Some(child) = cx.nth_child(index) {
            cx.with_current(child, |cx| inspect(cx, sliders));
            index += 1;
        }
    }

    /// Uses the production editor builder, not a separately constructed widget.
    #[test]
    fn production_editor_wires_every_visible_parameter_to_relative_sliders() {
        let plugin = SpectralCompressor::default();
        let mut cx = Context::default();
        Data {
            params: plugin.params.clone(),
            editor_mode: plugin.params.editor_mode.clone(),
            analyzer_data: plugin.analyzer_output_data.clone(),
            sample_rate: plugin.sample_rate.clone(),
        }.build(&mut cx);
        main_column(&mut cx);
        let mut sliders = Vec::new();
        inspect(&mut EventContext::new_with_current(&mut cx, Entity::root()), &mut sliders);
        let visible: Vec<_> = plugin.params.param_map().into_iter()
            .filter(|(_, p, _)| !unsafe { p.flags() }.contains(ParamFlags::HIDE_IN_GENERIC_UI))
            .collect();
        assert_eq!(sliders.len(), visible.len());
        for (_, ptr, _) in visible {
            assert_eq!(sliders.iter().filter(|(_, name)| name == unsafe { ptr.name() }).count(), 1);
        }
    }

    #[test]
    fn production_gain_toggle_is_a_host_parameter_button() {
        let plugin = SpectralCompressor::default();
        let mut cx = Context::default();
        Data { params: plugin.params.clone(), editor_mode: plugin.params.editor_mode.clone(),
            analyzer_data: plugin.analyzer_output_data.clone(), sample_rate: plugin.sample_rate.clone() }.build(&mut cx);
        let trace = Arc::new(Mutex::new(Vec::new()));
        Recorder(trace.clone()).build(&mut cx);
        main_column(&mut cx);
        let toggle = cx.resolve_entity_identifier("auto-gain-toggle").unwrap();
        assert!(EventContext::new_with_current(&mut cx, toggle).get_view::<ParamButton>().is_some());
        let mut backend = BackendContext::new_with_event_manager(&mut cx);
        backend.send_event(Event::new(WindowEvent::MouseDown(MouseButton::Left))
            .target(toggle).origin(toggle).propagate(Propagation::Direct));
        backend.process_events();
        let messages = trace.lock().unwrap();
        let ptr = plugin.params.auto_gain_enabled.as_ptr();
        assert_eq!(messages.len(),3);
        assert!(matches!(messages[0], Message::Begin(p) if p==ptr));
        assert!(matches!(messages[1], Message::Set(p,v) if p==ptr && v==1.0));
        assert!(matches!(messages[2], Message::End(p) if p==ptr));

    }

    #[test]
    fn platform_reset_click_resets_every_toggle_to_its_default() {
        let plugin=SpectralCompressor::default();
        for (_,ptr,_) in plugin.params.param_map() {
            let ParamPtr::BoolParam(p)=ptr else {continue;};
            for button in [MouseButton::Left,MouseButton::Right] {
                crate::test_support::set_raw(ptr,1.0-unsafe {ptr.default_normalized_value()});
                let mut cx=Context::default();
                Data {params:plugin.params.clone(),editor_mode:plugin.params.editor_mode.clone(),analyzer_data:plugin.analyzer_output_data.clone(),sample_rate:plugin.sample_rate.clone()}.build(&mut cx);
                struct Apply;
                impl Model for Apply {
                    fn event(&mut self,_cx:&mut EventContext,event:&mut Event) {
                        event.map(|raw,_| {if let RawParamEvent::SetParameterNormalized(p,v)=raw {crate::test_support::set_raw(*p,*v);}});
                    }
                }
                Apply.build(&mut cx);
                let e=ParamButton::new(&mut cx,Data::params,move |_|unsafe {&*p}).entity();
                let mut backend=BackendContext::new_with_event_manager(&mut cx);*backend.modifiers()=RESET_MODIFIER;
                backend.send_event(Event::new(WindowEvent::MouseDown(button)).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
                assert_eq!(unsafe {ptr.unmodulated_normalized_value()},unsafe {ptr.default_normalized_value()});
            }
        }
    }

    #[test]
    fn platform_reset_click_resets_every_numeric_parameter_without_starting_a_drag() {
        for button in [MouseButton::Left,MouseButton::Right] {
            let plugin=SpectralCompressor::default();
            for (_,ptr,_) in plugin.params.param_map() {
                if matches!(ptr,ParamPtr::BoolParam(_)){continue;}
                let default=unsafe {ptr.default_normalized_value()};
                crate::test_support::set_raw(ptr,if default<0.5 {0.9}else{0.1});
                let mut cx=Context::default();
                Data {params:plugin.params.clone(),editor_mode:plugin.params.editor_mode.clone(),analyzer_data:plugin.analyzer_output_data.clone(),sample_rate:plugin.sample_rate.clone()}.build(&mut cx);
                struct Apply;
                impl Model for Apply {
                    fn event(&mut self,_cx:&mut EventContext,event:&mut Event) {
                        event.map(|raw,_| {if let RawParamEvent::SetParameterNormalized(p,v)=raw {crate::test_support::set_raw(*p,*v);}});
                    }
                }
                Apply.build(&mut cx);relative_widget(&mut cx,Data::params,ptr);
                let mut sliders=Vec::new();inspect(&mut EventContext::new_with_current(&mut cx,Entity::root()),&mut sliders);
                let e=sliders[0].0;let mut backend=BackendContext::new_with_event_manager(&mut cx);
                *backend.modifiers()=RESET_MODIFIER;
                backend.send_event(Event::new(WindowEvent::MouseDown(button)).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
                let actual=unsafe {ptr.unmodulated_normalized_value()};assert!((actual-default).abs()<1e-4,"{} did not reset",unsafe {ptr.name()});
                backend.send_event(Event::new(WindowEvent::MouseMove(90.0,30.0)).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
                assert_eq!(unsafe {ptr.unmodulated_normalized_value()},actual);
            }
        }
    }

    #[test]
    fn production_slider_controls_ignore_clicks_and_send_relative_drag_values() {
        let params = SpectralCompressor::default().params;
        let targets: Vec<String> = params.param_map().into_iter()
            .filter(|(_, p, _)| !unsafe { p.flags() }.contains(ParamFlags::HIDE_IN_GENERIC_UI))
            .map(|(_, p, _)| unsafe { p.name() }.to_string()).collect();
        assert_eq!(targets.len(), 20);
        for target in targets {
            for grab in [10.0, 50.0, 90.0] {
                let plugin = SpectralCompressor::default();
                let mut cx = Context::default();
                Data {
                    params: plugin.params.clone(),
                    editor_mode: plugin.params.editor_mode.clone(),
                    analyzer_data: plugin.analyzer_output_data.clone(),
                    sample_rate: plugin.sample_rate.clone(),
                }.build(&mut cx);
                let trace = Arc::new(Mutex::new(Vec::new()));
                Recorder(trace.clone()).build(&mut cx);
                main_column(&mut cx);
                let mut sliders = Vec::new();
                inspect(&mut EventContext::new_with_current(&mut cx, Entity::root()), &mut sliders);
                let entity = sliders.iter().find(|(_, name)| name == &target).unwrap().0;
                let ptr = plugin.params.param_map().into_iter()
                    .find(|(_, p, _)| unsafe { p.name() } == target).unwrap().1;
                let initial = unsafe { ptr.unmodulated_normalized_value() };
                let direction = if initial >= 0.5 { -1.0 } else { 1.0 };
                let delta = unsafe { ptr.step_count() }
                    .filter(|steps| *steps > 0)
                    .map(|steps| (1.0 / steps as f32).max(0.1))
                    .unwrap_or(0.1);
                let mut backend = BackendContext::new_with_event_manager(&mut cx);
                backend.cache().set_width(entity, 100.0);
                backend.cache().set_height(entity, 30.0);
                backend.emit_origin(WindowEvent::MouseMove(grab, 15.0));
                backend.process_events();
                let send = |backend: &mut BackendContext, message| {
                    backend.send_event(Event::new(message).target(entity).origin(entity)
                        .propagate(Propagation::Direct));
                    backend.process_events();
                };
                for press in [WindowEvent::MouseDown(MouseButton::Left),
                              WindowEvent::MouseDoubleClick(MouseButton::Left),
                              WindowEvent::MouseTripleClick(MouseButton::Left)] {
                    send(&mut backend, press);
                    send(&mut backend, WindowEvent::MouseMove(grab, 25.0));
                    send(&mut backend, WindowEvent::MouseUp(MouseButton::Left));
                }
                send(&mut backend, WindowEvent::MouseDown(MouseButton::Right));
                send(&mut backend, WindowEvent::MouseUp(MouseButton::Right));
                *backend.modifiers() = OTHER_MODIFIER;
                send(&mut backend, WindowEvent::MouseDown(MouseButton::Left));
                send(&mut backend, WindowEvent::MouseUp(MouseButton::Left));
                *backend.modifiers() = Modifiers::empty();
                assert!(trace.lock().unwrap().is_empty(), "{target} changed on click at {grab}");
                send(&mut backend, WindowEvent::MouseDown(MouseButton::Left));
                assert!(trace.lock().unwrap().is_empty());
                send(&mut backend, WindowEvent::MouseMove(grab + direction * delta * 100.0, 15.0));
                send(&mut backend, WindowEvent::MouseMove(grab + direction * delta * 200.0, 15.0));
                send(&mut backend, WindowEvent::MouseUp(MouseButton::Left));
                send(&mut backend, WindowEvent::MouseMove(grab + 50.0, 15.0));
                let messages = trace.lock().unwrap();
                assert_eq!(messages.len(), 4, "{target}: {messages:?}");
                assert!(matches!(messages[0], Message::Begin(p) if p == ptr));
                assert!(matches!(messages[3], Message::End(p) if p == ptr));
                for (index, amount) in [(1, delta), (2, delta * 2.0)] {
                    let value = (initial + direction * amount).clamp(0.0, 1.0);
                    let expected = unsafe { ptr.preview_normalized(ptr.preview_plain(value)) };
                    assert!(matches!(messages[index], Message::Set(p, value)
                        if p == ptr && (value - expected).abs() < 1e-5));
                }
                println!("{target}, grab {grab}: clicks unchanged; {messages:?}");
            }
        }
    }
}

fn make_column(cx: &mut Context, title: &str, contents: impl FnOnce(&mut Context)) {
    VStack::new(cx, |cx| {
        Label::new(cx, title)
            .font_family(vec![FamilyOwned::Name(String::from(assets::NOTO_SANS))])
            .font_weight(FontWeightKeyword::Regular)
            .font_size(21.0)
            .left(Stretch(1.0))
            // This should align nicely with the right edge of the slider
            .right(Pixels(7.0))
            .bottom(Pixels(-10.0));

        contents(cx);
    })
    .width(COLUMN_WIDTH)
    .height(Auto);
}
