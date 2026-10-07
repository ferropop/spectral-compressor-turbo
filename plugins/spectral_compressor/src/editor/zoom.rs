//! Uniform GUI zoom; the renderer and host size share one scale value.
use nih_plug_vizia::vizia::prelude::*;
#[derive(Lens)]
pub struct GuiZoom {scale:f64}
#[derive(Clone,Copy)]
enum ZoomEvent {Step(f64),Reset,Read}
impl GuiZoom {
    pub fn new(cx:&mut Context)->Handle<'_,Self> {
        Self {scale:cx.user_scale_factor()}.build(cx,|cx| {
            HStack::new(cx,|cx| {
                Button::new(cx,|cx|cx.emit(ZoomEvent::Step(-0.1)),|cx|Label::new(cx,"−"))
                    .id("zoom-out").width(Pixels(20.0));
                Button::new(cx,|cx|cx.emit(ZoomEvent::Reset),|cx| {
                    Label::new(cx,GuiZoom::scale.map(|s|format!("{}%",(s*100.0).round() as u32)))
                }).id("zoom-reset").width(Pixels(44.0));
                Button::new(cx,|cx|cx.emit(ZoomEvent::Step(0.1)),|cx|Label::new(cx,"+"))
                    .id("zoom-in").width(Pixels(20.0));
            }).height(Pixels(22.0)).col_between(Pixels(2.0)).font_size(10.0);
            let timer=cx.add_timer(std::time::Duration::from_millis(33),None,|cx,action| {
                if matches!(action,TimerAction::Tick(_)){cx.emit(ZoomEvent::Read);}
            });cx.start_timer(timer);
        }).width(Pixels(88.0)).height(Pixels(22.0))
    }
}
impl View for GuiZoom {
    fn event(&mut self,cx:&mut EventContext,event:&mut Event) {
        event.map(|event,meta| {
            match event {
                ZoomEvent::Step(step)=>cx.set_user_scale_factor((cx.user_scale_factor()+step).clamp(0.5,2.0)),
                ZoomEvent::Reset=>cx.set_user_scale_factor(1.0),
                ZoomEvent::Read=>{},
            }
            self.scale=cx.user_scale_factor();meta.consume();
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug_vizia::vizia::context::backend::BackendContext;
    #[test]
    fn buttons_update_actual_renderer_scale_and_preserve_base_layout_dimensions() {
        let mut cx=Context::new(WindowSize::new(1360,565),1.0);
        let e=GuiZoom::new(&mut cx).entity();let mut backend=BackendContext::new_with_event_manager(&mut cx);backend.set_scale_factor(2.0);
        for (event,expected) in [(ZoomEvent::Step(0.2),1.2),(ZoomEvent::Step(-0.4),0.8),(ZoomEvent::Reset,1.0)] {
            backend.send_event(Event::new(event).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
            assert!((backend.user_scale_factor()-expected).abs()<1e-8);
            assert!((backend.scale_factor() as f64-2.0*expected).abs()<1e-6);
            assert_eq!(backend.window_size().width,1360);assert_eq!(backend.window_size().height,565);
        }
    }
    #[test]
    fn restoring_a_saved_scale_and_changing_zoom_keeps_retina_dpi() {
        let mut cx=Context::new(WindowSize::new(680,565),1.5);
        let e=GuiZoom::new(&mut cx).entity();let mut backend=BackendContext::new_with_event_manager(&mut cx);backend.set_scale_factor(3.0);
        backend.send_event(Event::new(ZoomEvent::Step(-0.25)).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
        assert_eq!(backend.user_scale_factor(),1.25);assert_eq!(backend.scale_factor(),2.5);
    }
}
