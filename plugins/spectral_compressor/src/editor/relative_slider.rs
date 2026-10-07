//! Relative-drag slider, adapted from NIH-plug's ISC-licensed ParamSlider.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;

use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::{util::ModifiersExt, ParamSliderStyle};

/// Shift reduces the normalized change per pixel to one tenth of normal speed.
const GRANULAR_DRAG_MULTIPLIER: f32 = 0.1;

/// A slider that integrates with NIH-plug's [`Param`] types. Use the
/// [`set_style()`][RelativeParamSliderExt::set_style()] method to change how the value gets displayed.
#[derive(Lens)]
pub struct RelativeParamSlider {
    param_base: ParamWidgetBase,

    /// Will be set to `true` when the field gets Alt+Click'ed which will replace the label with a
    /// text box.
    text_input_active: bool,
    /// Will be set to `true` if we're dragging the parameter. Resetting the parameter or entering a
    /// text value should not initiate a drag.
    drag_active: bool,
    /// Pointer displacement and unsnapped value for the current drag.
    relative_drag: Option<RelativeDrag>,
    gesture_started: bool,

    // These fields are set through modifiers:
    /// Whether or not to listen to scroll events for changing the parameter's value in steps.
    use_scroll_wheel: bool,
    /// The number of (fractional) scrolled lines that have not yet been turned into parameter
    /// change events. This is needed to support trackpads with smooth scrolling.
    scrolled_lines: f32,
    /// What style to use for the slider.
    style: ParamSliderStyle,
    /// A specific label to use instead of displaying the parameter's value.
    label_override: Option<String>,
}

enum ParamSliderEvent {
    /// Text input has been cancelled without submitting a new value.
    CancelTextInput,
    /// A new value has been sent by the text input dialog after pressing Enter.
    TextInput(String),
}

/// Accumulates pointer displacement independently of host update timing and
/// discrete parameter snapping. Coordinates and width use the same physical
/// units, so the drag sensitivity is stable on Retina and scaled windows.
#[derive(Debug, Clone, Copy)]
pub struct RelativeDrag {
    last_x: f32,
    value: f32,
    width: f32,
}

impl RelativeDrag {
    fn new(x: f32, value: f32, width: f32) -> Self {
        Self { last_x: x, value, width: width.max(1.0) }
    }

    fn advance(&mut self, x: f32, fine: bool) -> Option<f32> {
        let delta = x - self.last_x;
        self.last_x = x;
        if delta == 0.0 {
            return None;
        }
        let sensitivity = if fine { GRANULAR_DRAG_MULTIPLIER } else { 1.0 };
        self.value = (self.value + delta / self.width * sensitivity).clamp(0.0, 1.0);
        Some(self.value)
    }
}

impl RelativeParamSlider {
    #[cfg(test)]
    pub(super) fn parameter_name(&self) -> &str {
        self.param_base.name()
    }

    /// Creates a new [`RelativeParamSlider`] for the given parameter. To accommodate VIZIA's mapping system,
    /// you'll need to provide a lens containing your `Params` implementation object (check out how
    /// the `Data` struct is used in `gain_gui_vizia`) and a projection function that maps the
    /// `Params` object to the parameter you want to display a widget for. Parameter changes are
    /// handled by emitting [`ParamEvent`][nih_plug_vizia::widgets::ParamEvent]s, handled by
    /// the VIZIA wrapper.
    ///
    /// See [`RelativeParamSliderExt`] for additional options.
    pub fn new<L, Params, P, FMap>(
        cx: &mut Context,
        params: L,
        params_to_param: FMap,
    ) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        // We'll visualize the difference between the current value and the default value if the
        // default value lies somewhere in the middle and the parameter is continuous. Otherwise
        // this approach looks a bit jarring.
        Self {
            param_base: ParamWidgetBase::new(cx, params, params_to_param),

            text_input_active: false,
            drag_active: false,
            relative_drag: None,
            gesture_started: false,

            use_scroll_wheel: true,
            scrolled_lines: 0.0,
            style: ParamSliderStyle::Centered,
            label_override: None,
        }
        .build(
            cx,
            ParamWidgetBase::build_view(params, params_to_param, move |cx, param_data| {
                Binding::new(cx, RelativeParamSlider::style, move |cx, style| {
                    let style = style.get(cx);

                    // Can't use `.to_string()` here as that would include the modulation.
                    let unmodulated_normalized_value_lens =
                        param_data.make_lens(|param| param.unmodulated_normalized_value());
                    let display_value_lens = param_data.make_lens(|param| {
                        param.normalized_value_to_string(param.unmodulated_normalized_value(), true)
                    });

                    // The resulting tuple `(start_t, delta)` corresponds to the start and the
                    // signed width of the bar. `start_t` is in `[0, 1]`, and `delta` is in
                    // `[-1, 1]`.
                    let fill_start_delta_lens =
                        unmodulated_normalized_value_lens.map(move |current_value| {
                            Self::compute_fill_start_delta(
                                style,
                                param_data.param(),
                                *current_value,
                            )
                        });

                    // If the parameter is being modulated by the host (this only works for CLAP
                    // plugins with hosts that support this), then this is the difference
                    // between the 'true' value and the current value after modulation has been
                    // applied. This follows the same format as `fill_start_delta_lens`.
                    let modulation_start_delta_lens = param_data.make_lens(move |param| {
                        Self::compute_modulation_fill_start_delta(style, param)
                    });

                    // This is used to draw labels for `CurrentStepLabeled`
                    let make_preview_value_lens = move |normalized_value| {
                        param_data.make_lens(move |param| {
                            param.normalized_value_to_string(normalized_value, true)
                        })
                    };

                    // Only draw the text input widget when it gets focussed. Otherwise, overlay the
                    // label with the slider. Creating the textbox based on
                    // `ParamSliderInternal::text_input_active` lets us focus the textbox when it gets
                    // created.
                    Binding::new(
                        cx,
                        RelativeParamSlider::text_input_active,
                        move |cx, text_input_active| {
                            if text_input_active.get(cx) {
                                Self::text_input_view(cx, display_value_lens);
                            } else {
                                ZStack::new(cx, |cx| {
                                    Self::slider_fill_view(
                                        cx,
                                        fill_start_delta_lens,
                                        modulation_start_delta_lens,
                                    );
                                    Self::slider_label_view(
                                        cx,
                                        param_data.param(),
                                        style,
                                        display_value_lens,
                                        make_preview_value_lens,
                                        RelativeParamSlider::label_override,
                                    );
                                })
                                .hoverable(false);
                            }
                        },
                    );
                });
            }),
        )
    }

    /// Create a text input that's shown in place of the slider.
    fn text_input_view(cx: &mut Context, display_value_lens: impl Lens<Target = String>) {
        Textbox::new(cx, display_value_lens)
            .class("value-entry")
            .on_submit(|cx, string, success| {
                if success {
                    cx.emit(ParamSliderEvent::TextInput(string))
                } else {
                    cx.emit(ParamSliderEvent::CancelTextInput);
                }
            })
            .on_cancel(|cx| {
                cx.emit(ParamSliderEvent::CancelTextInput);
            })
            .on_build(|cx| {
                cx.emit(TextEvent::StartEdit);
                cx.emit(TextEvent::SelectAll);
            })
            // `.child_space(Stretch(1.0))` no longer works
            .class("align_center")
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0))
            .height(Stretch(1.0))
            .width(Stretch(1.0));
    }

    /// Create the fill part of the slider.
    fn slider_fill_view(
        cx: &mut Context,
        fill_start_delta_lens: impl Lens<Target = (f32, f32)>,
        modulation_start_delta_lens: impl Lens<Target = (f32, f32)>,
    ) {
        // The filled bar portion. This can be visualized in a couple different ways depending on
        // the current style property. See [`ParamSliderStyle`].
        Element::new(cx)
            .class("fill")
            .height(Stretch(1.0))
            .left(fill_start_delta_lens.map(|(start_t, _)| Percentage(start_t * 100.0)))
            .width(fill_start_delta_lens.map(|(_, delta)| Percentage(delta * 100.0)))
            // Hovering is handled on the param slider as a whole, this
            // should not affect that
            .hoverable(false);

        // If the parameter is being modulated, then we'll display another
        // filled bar showing the current modulation delta
        // VIZIA's bindings make this a bit, uh, difficult to read
        Element::new(cx)
            .class("fill")
            .class("fill--modulation")
            .height(Stretch(1.0))
            .visibility(modulation_start_delta_lens.map(|(_, delta)| *delta != 0.0))
            // Widths cannot be negative, so we need to compensate the start
            // position if the width does happen to be negative
            .width(modulation_start_delta_lens.map(|(_, delta)| Percentage(delta.abs() * 100.0)))
            .left(modulation_start_delta_lens.map(|(start_t, delta)| {
                if *delta < 0.0 {
                    Percentage((start_t + delta) * 100.0)
                } else {
                    Percentage(start_t * 100.0)
                }
            }))
            .hoverable(false);
    }

    /// Create the text part of the slider. Shown on top of the fill using a `ZStack`.
    fn slider_label_view<P: Param, L: Lens<Target = String>>(
        cx: &mut Context,
        param: &P,
        style: ParamSliderStyle,
        display_value_lens: impl Lens<Target = String>,
        make_preview_value_lens: impl Fn(f32) -> L,
        label_override_lens: impl Lens<Target = Option<String>>,
    ) {
        let step_count = param.step_count();

        // Either display the current value, or display all values over the
        // parameter's steps
        // TODO: Do the same thing as in the iced widget where we draw the
        //       text overlapping the fill area slightly differently. We can
        //       set the cip region directly in vizia.
        match (style, step_count) {
            (ParamSliderStyle::CurrentStepLabeled { .. }, Some(step_count)) => {
                HStack::new(cx, |cx| {
                    // There are step_count + 1 possible values for a
                    // discrete parameter
                    for value in 0..step_count + 1 {
                        let normalized_value = value as f32 / step_count as f32;
                        let preview_lens = make_preview_value_lens(normalized_value);

                        Label::new(cx, preview_lens)
                            .class("value")
                            .class("value--multiple")
                            .child_space(Stretch(1.0))
                            .height(Stretch(1.0))
                            .width(Stretch(1.0))
                            .hoverable(false);
                    }
                })
                .height(Stretch(1.0))
                .width(Stretch(1.0))
                .hoverable(false);
            }
            _ => {
                Binding::new(cx, label_override_lens, move |cx, label_override_lens| {
                    // If the label override is set then we'll use that. If not, the parameter's
                    // current display value (before modulation) is used.
                    match label_override_lens.get(cx) {
                        Some(label_override) => Label::new(cx, &label_override),
                        None => Label::new(cx, display_value_lens),
                    }
                    .class("value")
                    .class("value--single")
                    .child_space(Stretch(1.0))
                    .height(Stretch(1.0))
                    .width(Stretch(1.0))
                    .hoverable(false);
                });
            }
        };
    }

    /// Calculate the start position and width of the slider's fill region based on the selected
    /// style, the parameter's current value, and the parameter's step sizes. The resulting tuple
    /// `(start_t, delta)` corresponds to the start and the signed width of the bar. `start_t` is in
    /// `[0, 1]`, and `delta` is in `[-1, 1]`.
    fn compute_fill_start_delta<P: Param>(
        style: ParamSliderStyle,
        param: &P,
        current_value: f32,
    ) -> (f32, f32) {
        let default_value = param.default_normalized_value();
        let step_count = param.step_count();
        let draw_fill_from_default = matches!(style, ParamSliderStyle::Centered)
            && step_count.is_none()
            && (0.45..=0.55).contains(&default_value);

        match style {
            ParamSliderStyle::Centered if draw_fill_from_default => {
                let delta = (default_value - current_value).abs();

                // Don't draw the filled portion at all if it could have been a
                // rounding error since those slivers just look weird
                (
                    default_value.min(current_value),
                    if delta >= 1e-3 { delta } else { 0.0 },
                )
            }
            ParamSliderStyle::FromMidPoint => {
                let delta = (0.5 - current_value).abs();

                // Don't draw the filled portion at all if it could have been a
                // rounding error since those slivers just look weird
                (
                    0.5_f32.min(current_value),
                    if delta >= 1e-3 { delta } else { 0.0 },
                )
            }
            ParamSliderStyle::Centered | ParamSliderStyle::FromLeft => (0.0, current_value),
            ParamSliderStyle::CurrentStep { even: true }
            | ParamSliderStyle::CurrentStepLabeled { even: true }
                if step_count.is_some() =>
            {
                // Assume the normalized value is distributed evenly
                // across the range.
                let step_count = step_count.unwrap() as f32;
                let discrete_values = step_count + 1.0;
                let previous_step = (current_value * step_count) / discrete_values;

                (previous_step, discrete_values.recip())
            }
            ParamSliderStyle::CurrentStep { .. } | ParamSliderStyle::CurrentStepLabeled { .. } => {
                let previous_step = param.previous_normalized_step(current_value, false);
                let next_step = param.next_normalized_step(current_value, false);

                (
                    (previous_step + current_value) / 2.0,
                    ((next_step - current_value) + (current_value - previous_step)) / 2.0,
                )
            }
        }
    }

    /// The same as `compute_fill_start_delta`, but just showing the modulation offset.
    fn compute_modulation_fill_start_delta<P: Param>(
        style: ParamSliderStyle,
        param: &P,
    ) -> (f32, f32) {
        match style {
            // Don't show modulation for stepped parameters since it wouldn't
            // make a lot of sense visually
            ParamSliderStyle::CurrentStep { .. } | ParamSliderStyle::CurrentStepLabeled { .. } => {
                (0.0, 0.0)
            }
            ParamSliderStyle::Centered
            | ParamSliderStyle::FromMidPoint
            | ParamSliderStyle::FromLeft => {
                let modulation_start = param.unmodulated_normalized_value();

                (
                    modulation_start,
                    param.modulated_normalized_value() - modulation_start,
                )
            }
        }
    }


}

impl View for RelativeParamSlider {
    fn element(&self) -> Option<&'static str> {
        Some("param-slider")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|param_slider_event, meta| match param_slider_event {
            ParamSliderEvent::CancelTextInput => {
                self.text_input_active = false;
                cx.set_active(false);

                meta.consume();
            }
            ParamSliderEvent::TextInput(string) => {
                if let Some(normalized_value) = self.param_base.string_to_normalized_value(string) {
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, normalized_value);
                    self.param_base.end_set_parameter(cx);
                }

                self.text_input_active = false;

                meta.consume();
            }
        });

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left)
            | WindowEvent::MouseDoubleClick(MouseButton::Left)
            | WindowEvent::MouseTripleClick(MouseButton::Left) => {
                if cx.modifiers().contains(Modifiers::CTRL) {
                    self.drag_active = false;
                    self.relative_drag = None;
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, self.param_base.default_normalized_value());
                    self.param_base.end_set_parameter(cx);
                    meta.consume();
                    return;
                } else if cx.modifiers().alt() {
                    self.text_input_active = true;
                    cx.set_active(true);
                } else if !self.text_input_active {
                    self.drag_active = true;
                    self.gesture_started = false;
                    self.relative_drag = Some(RelativeDrag::new(
                        cx.mouse().cursorx,
                        self.param_base.unmodulated_normalized_value(),
                        cx.cache.get_width(cx.current()) - 2.0 * cx.border_width(),
                    ));
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    // Deliberately emit neither a value change nor an automation
                    // gesture here. Grabbing anywhere on the bar only arms a drag.
                }
                meta.consume();
            }
            WindowEvent::MouseDown(MouseButton::Right)
            | WindowEvent::MouseDoubleClick(MouseButton::Right)
            | WindowEvent::MouseTripleClick(MouseButton::Right) => {
                if cx.modifiers().contains(Modifiers::CTRL) {
                    self.param_base.begin_set_parameter(cx);
                    self.param_base.set_normalized_value(cx, self.param_base.default_normalized_value());
                    self.param_base.end_set_parameter(cx);
                }
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                if self.drag_active {
                    self.drag_active = false;
                    self.relative_drag = None;
                    cx.release();
                    cx.set_active(false);
                    if self.gesture_started {
                        self.param_base.end_set_parameter(cx);
                        self.gesture_started = false;
                    }
                    meta.consume();
                }
            }
            WindowEvent::MouseMove(x, _y) => {
                if let Some(drag) = self.relative_drag.as_mut() {
                    if let Some(value) = drag.advance(*x, cx.modifiers().shift()) {
                        if self.param_base.preview_plain(value)
                            != self.param_base.unmodulated_plain_value()
                        {
                            if !self.gesture_started {
                                self.param_base.begin_set_parameter(cx);
                                self.gesture_started = true;
                            }
                            // Relative motion uses the parameter's normalized
                            // range directly, including discrete parameter steps.
                            self.param_base.set_normalized_value(cx, value);
                        }
                    }
                }
            }
            // Modifier changes alter only the sensitivity of future motion.
            // Releasing Shift must never snap to the absolute pointer position.
            WindowEvent::MouseScroll(_scroll_x, scroll_y) if self.use_scroll_wheel => {
                // With a regular scroll wheel `scroll_y` will only ever be -1 or 1, but with smooth
                // scrolling trackpads being a thing `scroll_y` could be anything.
                self.scrolled_lines += scroll_y;

                if self.scrolled_lines.abs() >= 1.0 {
                    let use_finer_steps = cx.modifiers().shift();

                    // Scrolling while dragging needs to be taken into account here
                    if !self.drag_active || !self.gesture_started {
                        self.param_base.begin_set_parameter(cx);
                        if self.drag_active {
                            self.gesture_started = true;
                        }
                    }

                    let mut current_value = self.param_base.unmodulated_normalized_value();

                    while self.scrolled_lines >= 1.0 {
                        current_value = self
                            .param_base
                            .next_normalized_step(current_value, use_finer_steps);
                        self.param_base.set_normalized_value(cx, current_value);
                        self.scrolled_lines -= 1.0;
                    }

                    while self.scrolled_lines <= -1.0 {
                        current_value = self
                            .param_base
                            .previous_normalized_step(current_value, use_finer_steps);
                        self.param_base.set_normalized_value(cx, current_value);
                        self.scrolled_lines += 1.0;
                    }

                    if let Some(drag) = self.relative_drag.as_mut() {
                        drag.value = current_value;
                    }
                    if !self.drag_active {
                        self.param_base.end_set_parameter(cx);
                    }
                }

                meta.consume();
            }
            _ => {}
        });
    }
}

/// Extension methods for [`RelativeParamSlider`] handles.
pub trait RelativeParamSliderExt {
    /// Change how the [`RelativeParamSlider`] visualizes the current value.
    fn set_style(self, style: ParamSliderStyle) -> Self;
}

impl RelativeParamSliderExt for Handle<'_, RelativeParamSlider> {
    fn set_style(self, style: ParamSliderStyle) -> Self {
        self.modify(|param_slider: &mut RelativeParamSlider| param_slider.style = style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::prelude::{FloatParam, FloatRange, IntParam, IntRange, Params};
    use nih_plug_vizia::vizia::context::backend::BackendContext;
    use nih_plug_vizia::widgets::RawParamEvent;
    use std::sync::{Arc, Mutex};

    fn near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
    }

    #[test]
    fn relative_motion_is_independent_of_grab_position() {
        for grab in [0.0, 25.0, 90.0, 350.0] {
            let mut drag = RelativeDrag::new(grab, 0.6, 100.0);
            assert_eq!(drag.advance(grab, false), None);
            near(drag.advance(grab + 10.0, false).unwrap(), 0.7);
            near(drag.advance(grab + 20.0, false).unwrap(), 0.8);
            near(drag.advance(grab, false).unwrap(), 0.6);
            near(drag.advance(grab - 10.0, false).unwrap(), 0.5);
        }
    }

    #[test]
    fn fine_modifier_transitions_do_not_jump() {
        let mut drag = RelativeDrag::new(70.0, 0.5, 100.0);
        near(drag.advance(80.0, false).unwrap(), 0.6);
        assert_eq!(drag.advance(80.0, true), None);
        near(drag.advance(90.0, true).unwrap(), 0.61);
        assert_eq!(drag.advance(90.0, false), None);
        near(drag.advance(100.0, false).unwrap(), 0.71);
    }

    #[test]
    fn limits_clamp_and_reverse_without_an_overshoot_dead_zone() {
        let mut drag = RelativeDrag::new(0.0, 0.95, 100.0);
        near(drag.advance(1000.0, false).unwrap(), 1.0);
        near(drag.advance(990.0, false).unwrap(), 0.9);
        near(drag.advance(-1000.0, false).unwrap(), 0.0);
        near(drag.advance(-990.0, false).unwrap(), 0.1);
    }

    #[test]
    fn pointer_and_width_scale_together() {
        let mut normal = RelativeDrag::new(40.0, 0.3, 180.0);
        let mut retina = RelativeDrag::new(80.0, 0.3, 360.0);
        for (x, fine) in [(50.0, false), (65.0, true), (90.0, false)] {
            near(normal.advance(x, fine).unwrap(), retina.advance(x * 2.0, fine).unwrap());
        }
    }

    #[test]
    fn small_moves_accumulate_for_discrete_parameters() {
        let param = IntParam::new("Steps", 5, IntRange::Linear { min: 0, max: 10 });
        let mut drag = RelativeDrag::new(90.0, param.unmodulated_normalized_value(), 100.0);
        for x in 91..=94 {
            assert_eq!(param.preview_plain(drag.advance(x as f32, false).unwrap()), 5);
        }
        assert_eq!(param.preview_plain(drag.advance(96.0, false).unwrap()), 6);
    }

    #[derive(Params)]
    struct TestParams {
        #[id = "value"]
        value: FloatParam,
    }

    #[derive(Lens)]
    struct TestData {
        params: Arc<TestParams>,
    }
    impl Model for TestData {}

    #[derive(Debug, PartialEq)]
    enum Recorded {
        Begin,
        Set(f32),
        End,
    }

    struct Recorder(Arc<Mutex<Vec<Recorded>>>);
    impl Model for Recorder {
        fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
            event.map(|raw, _| {
                let value = match raw {
                    RawParamEvent::BeginSetParameter(_) => Recorded::Begin,
                    RawParamEvent::SetParameterNormalized(_, value) => Recorded::Set(*value),
                    RawParamEvent::EndSetParameter(_) => Recorded::End,
                    RawParamEvent::ParametersChanged => return,
                };
                self.0.lock().unwrap().push(value);
            });
        }
    }

    /// Sends events through the real Vizia widget and records its host messages.
    /// The recorder does not emulate DSP or claim native OS input acceptance.
    fn widget_trace(grab: f32, clicks_only: bool) -> Vec<Recorded> {
        let mut cx = Context::default();
        let params = Arc::new(TestParams {
            value: FloatParam::new("Value", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 }),
        });
        TestData { params }.build(&mut cx);
        let trace = Arc::new(Mutex::new(Vec::new()));
        Recorder(trace.clone()).build(&mut cx);
        let entity = RelativeParamSlider::new(&mut cx, TestData::params, |p| &p.value).entity();
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
        if clicks_only {
            for press in [WindowEvent::MouseDown(MouseButton::Left),
                          WindowEvent::MouseDoubleClick(MouseButton::Left),
                          WindowEvent::MouseTripleClick(MouseButton::Left)] {
                send(&mut backend, press);
                send(&mut backend, WindowEvent::MouseMove(grab, 25.0));
                send(&mut backend, WindowEvent::MouseUp(MouseButton::Left));
            }
        } else {
            send(&mut backend, WindowEvent::MouseDown(MouseButton::Left));
            assert!(trace.lock().unwrap().is_empty(), "Mouse-down changed the parameter");
            send(&mut backend, WindowEvent::MouseMove(grab + 10.0, 15.0));
            send(&mut backend, WindowEvent::MouseMove(grab + 20.0, 15.0));
            send(&mut backend, WindowEvent::MouseUp(MouseButton::Left));
            send(&mut backend, WindowEvent::MouseMove(grab + 50.0, 15.0));
        }
        let result = trace.lock().unwrap().drain(..).collect();
        result
    }

    #[test]
    fn real_widget_clicks_and_vertical_motion_emit_no_host_changes() {
        assert!(widget_trace(10.0, true).is_empty());
        assert!(widget_trace(90.0, true).is_empty());
    }

    #[test]
    fn real_widget_brackets_relative_changes_in_one_automation_gesture() {
        for grab in [10.0, 90.0] {
            let trace = widget_trace(grab, false);
            assert_eq!(trace.len(), 4, "Unexpected host trace: {trace:?}");
            assert_eq!(trace[0], Recorded::Begin);
            match trace[1] { Recorded::Set(value) => near(value, 0.6), _ => panic!("No first value") }
            match trace[2] { Recorded::Set(value) => near(value, 0.7), _ => panic!("No second value") }
            assert_eq!(trace[3], Recorded::End);
        }
    }
}
