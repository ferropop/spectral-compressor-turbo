use super::{analyzer::Analyzer,relative_slider::RelativeParamSlider,Data};
use crate::response::{self,HPF,LPF,NODE_COUNT};
use nih_plug::prelude::*;
use nih_plug_vizia::vizia::{prelude::*,vg};
use nih_plug_vizia::widgets::{util::ModifiersExt,RawParamEvent,ParamButton,ParamButtonExt};
const LN_MIN:f32=3.4011974;
const LN_MAX:f32=9.998797;
const LN_RANGE:f32=LN_MAX-LN_MIN;
struct DeleteNode;
pub(super) fn plot_bounds(mut b:BoundingBox,scale:f32)->BoundingBox {
    b.x+=8.0*scale;b.w=(b.w-16.0*scale).max(1.0);
    b.y+=28.0*scale;b.h=(b.h-142.0*scale).max(1.0);b
}
fn set(cx:&mut EventContext,ptr:ParamPtr,value:f32) {
    cx.emit(RawParamEvent::BeginSetParameter(ptr));
    cx.emit(RawParamEvent::SetParameterNormalized(ptr,value));
    cx.emit(RawParamEvent::EndSetParameter(ptr));
}
fn x_frequency(x:f32,b:BoundingBox)->f32 {(LN_MIN+((x-b.x)/b.w).clamp(0.0,1.0)*LN_RANGE).exp()}
fn y_amount(y:f32,b:BoundingBox)->f32 {((0.5-(y-b.y)/b.h)*36.0).clamp(-18.0,18.0)}
impl Analyzer {
    pub(super) fn build_response_panel(cx:&mut Context) {
        Label::new(cx,"RESPONSE SHAPER  |  Click-drag: bell  ·  Alt-click: delete  ·  Wheel: Q")
            .position_type(PositionType::SelfDirected).top(Pixels(3.0)).left(Pixels(8.0))
            .id("response-gesture-hint").height(Pixels(20.0)).font_size(10.0).hoverable(false);
        HStack::new(cx,|cx| {
            Label::new(cx,"Response").class("legend-response");
            Label::new(cx,"Down threshold").class("legend-down");
            Label::new(cx,"Up threshold").class("legend-up");
            Label::new(cx,"Shading: gain change");
        }).position_type(PositionType::SelfDirected).top(Pixels(16.0)).left(Pixels(8.0))
            .height(Pixels(11.0)).font_size(9.0).col_between(Pixels(12.0)).hoverable(false);
        Binding::new(cx,Analyzer::selected,|cx,selected| {
            let i=selected.get(cx);
            VStack::new(cx,|cx| {
                HStack::new(cx,|cx| {
                    Label::new(cx,&if i==HPF {"PROCESSING HPF".into()} else if i==LPF {"PROCESSING LPF".into()} else {format!("RESPONSE BELL {}",i+1)})
                        .font_size(12.0).width(Stretch(1.0));
                    if i<NODE_COUNT {
                        ParamButton::new(cx,Data::params,move|p|&p.response_nodes[i].enabled)
                            .with_label("On").disable_scroll_wheel().width(Pixels(42.0));
                        Button::new(cx,|cx|cx.emit(DeleteNode),|cx|Label::new(cx,"Delete")).width(Pixels(62.0));
                    }
                }).height(Pixels(24.0)).col_between(Pixels(6.0));
                HStack::new(cx,|cx| {
                    VStack::new(cx,|cx| {
                        Label::new(cx,"Frequency").font_size(11.0);
                        if i==HPF {RelativeParamSlider::new(cx,Data::params,|p|&p.response_edges.high_pass).height(Pixels(24.0)).width(Stretch(1.0));}
                        else if i==LPF {RelativeParamSlider::new(cx,Data::params,|p|&p.response_edges.low_pass).height(Pixels(24.0)).width(Stretch(1.0));}
                        else {RelativeParamSlider::new(cx,Data::params,move|p|&p.response_nodes[i].frequency).height(Pixels(24.0)).width(Stretch(1.0));}
                    }).width(Stretch(1.0));
                    if i<NODE_COUNT {
                        VStack::new(cx,|cx| {
                            Label::new(cx,"Processing strength").font_size(11.0);
                            RelativeParamSlider::new(cx,Data::params,move|p|&p.response_nodes[i].amount).height(Pixels(24.0)).width(Stretch(1.0));
                        }).width(Stretch(1.0));
                    }
                    VStack::new(cx,|cx| {
                        Label::new(cx,"Q / width").font_size(11.0);
                        if i==HPF {RelativeParamSlider::new(cx,Data::params,|p|&p.response_edges.high_pass_q).height(Pixels(24.0)).width(Stretch(1.0));}
                        else if i==LPF {RelativeParamSlider::new(cx,Data::params,|p|&p.response_edges.low_pass_q).height(Pixels(24.0)).width(Stretch(1.0));}
                        else {RelativeParamSlider::new(cx,Data::params,move|p|&p.response_nodes[i].q).height(Pixels(24.0)).width(Stretch(1.0));}
                    }).width(Stretch(1.0));
                }).height(Pixels(44.0)).col_between(Pixels(8.0));
            }).position_type(PositionType::SelfDirected).top(Stretch(1.0)).bottom(Pixels(6.0))
                .left(Pixels(8.0)).right(Pixels(8.0)).height(Pixels(78.0))
                .class("response-panel");
        });
    }
    fn frequency(&self,i:usize)->&FloatParam {
        if i==HPF {&self.params.response_edges.high_pass} else if i==LPF {&self.params.response_edges.low_pass} else {&self.params.response_nodes[i].frequency}
    }
    fn q(&self,i:usize)->&FloatParam {
        if i==HPF {&self.params.response_edges.high_pass_q} else if i==LPF {&self.params.response_edges.low_pass_q} else {&self.params.response_nodes[i].q}
    }
    fn position(&self,i:usize,b:BoundingBox,scale:f32)->(f32,f32) {
        let x=b.x+(self.frequency(i).value().ln()-LN_MIN)/LN_RANGE*b.w;
        let y=if i<NODE_COUNT {b.y+(0.5-self.params.response_nodes[i].amount.value()/36.0)*b.h} else {b.y+b.h*0.5};
        (x,y.clamp(b.y+(8.0*scale).min(b.h*0.5),b.y+b.h-(8.0*scale).min(b.h*0.5)))
    }
    fn hit(&self,x:f32,y:f32,b:BoundingBox,scale:f32)->Option<usize> {
        // Bells take priority when a bell overlaps a filter handle.
        (0..NODE_COUNT+2).filter(|&i|i>=NODE_COUNT||self.params.response_nodes[i].enabled.value())
            .find(|&i| {let(nx,ny)=self.position(i,b,scale);(nx-x).hypot(ny-y)<=12.0*scale})
    }
    pub(super) fn response_event(&mut self,cx:&mut EventContext,event:&mut Event) {
        event.map(|_:&DeleteNode,meta| {
            if self.selected<NODE_COUNT {set(cx,self.params.response_nodes[self.selected].enabled.as_ptr(),0.0);self.selected=HPF;}
            meta.consume();
        });
        event.map(|window,meta| match window {
            WindowEvent::MouseDown(button) if *button==MouseButton::Left||*button==MouseButton::Right => {
                let b=plot_bounds(cx.bounds(),cx.scale_factor());let(x,y)=(cx.mouse().cursorx,cx.mouse().cursory);
                // Test handles first: edge handles remain clickable across their full hit circle.
                let hit=self.hit(x,y,b,cx.scale_factor());
                if hit.is_none() && (x<b.x||x>b.x+b.w||y<b.y||y>b.y+b.h) {return;}
                if cx.modifiers().command() {
                    if let Some(i)=hit {
                        self.selected=i;
                        let p=if i<NODE_COUNT {&self.params.response_nodes[i].amount} else {self.frequency(i)};
                        set(cx,p.as_ptr(),p.default_normalized_value());
                    }
                    meta.consume();return;
                }
                if cx.modifiers().contains(Modifiers::ALT) {
                    if let Some(i)=hit {if i<NODE_COUNT {set(cx,self.params.response_nodes[i].enabled.as_ptr(),0.0);self.selected=HPF;}}
                    meta.consume();return;
                }
                if *button!=MouseButton::Left {if let Some(i)=hit {self.selected=i;}meta.consume();return;}
                let index=hit.or_else(||(0..NODE_COUNT).find(|&i|!self.params.response_nodes[i].enabled.value()));
                if let Some(i)=index {
                    self.selected=i;
                    if hit.is_none() {
                        let node=&self.params.response_nodes[i];
                        set(cx,node.frequency.as_ptr(),node.frequency.preview_normalized(x_frequency(x,b)));
                        set(cx,node.amount.as_ptr(),node.amount.preview_normalized(y_amount(y,b)));
                        set(cx,node.q.as_ptr(),node.q.default_normalized_value());
                        set(cx,node.enabled.as_ptr(),1.0);
                    }
                    self.drag_ln=if hit.is_none(){x_frequency(x,b).ln()}else{self.frequency(i).value().ln()};
                    self.drag_db=if i<NODE_COUNT {if hit.is_none(){y_amount(y,b)}else{self.params.response_nodes[i].amount.value()}}else{0.0};
                    self.last_pointer=(x,y);self.dragging=true;
                    cx.emit(RawParamEvent::BeginSetParameter(self.frequency(i).as_ptr()));
                    if i<NODE_COUNT {cx.emit(RawParamEvent::BeginSetParameter(self.params.response_nodes[i].amount.as_ptr()));}
                    cx.capture();cx.focus();meta.consume();
                }
            }
            WindowEvent::MouseMove(x,y) if self.dragging => {
                let b=plot_bounds(cx.bounds(),cx.scale_factor());let fine=if cx.modifiers().contains(Modifiers::SHIFT){0.1}else{1.0};
                self.drag_ln=(self.drag_ln+(*x-self.last_pointer.0)/b.w*LN_RANGE*fine).clamp(LN_MIN,LN_MAX);
                self.drag_db=(self.drag_db-(*y-self.last_pointer.1)/b.h*36.0*fine).clamp(-18.0,18.0);
                self.last_pointer=(*x,*y);let i=self.selected;let p=self.frequency(i);
                cx.emit(RawParamEvent::SetParameterNormalized(p.as_ptr(),p.preview_normalized(self.drag_ln.exp())));
                if i<NODE_COUNT {let p=&self.params.response_nodes[i].amount;cx.emit(RawParamEvent::SetParameterNormalized(p.as_ptr(),p.preview_normalized(self.drag_db)));}
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Left) if self.dragging => {
                self.dragging=false;let i=self.selected;
                cx.emit(RawParamEvent::EndSetParameter(self.frequency(i).as_ptr()));
                if i<NODE_COUNT {cx.emit(RawParamEvent::EndSetParameter(self.params.response_nodes[i].amount.as_ptr()));}
                cx.release();meta.consume();
            }
            WindowEvent::MouseScroll(_,delta) => {
                let b=plot_bounds(cx.bounds(),cx.scale_factor());let(x,y)=(cx.mouse().cursorx,cx.mouse().cursory);
                if x<b.x||x>b.x+b.w||y<b.y||y>b.y+b.h {return;}
                let p=self.q(self.selected);let value=(p.value()*2.0_f32.powf(*delta*0.125)).clamp(0.2,10.0);
                set(cx,p.as_ptr(),p.preview_normalized(value));meta.consume();
            }
            _=>{}
        });
    }
    pub(super) fn draw_response(&self,cx:&mut DrawContext,canvas:&mut Canvas) {
        let scale=cx.scale_factor();let b=plot_bounds(cx.bounds(),scale);
        let palette=self.params.palette.lock().unwrap().active();
        let mut baseline=vg::Path::new();baseline.move_to(b.x,b.y+b.h*0.5);baseline.line_to(b.x+b.w,b.y+b.h*0.5);
        canvas.stroke_path(&baseline,&vg::Paint::color(vg::Color::rgbaf(0.5,0.5,0.5,0.45)).with_line_width(scale));
        let curve=response::ResponseCurve::new(&self.params.response_nodes,&self.params.response_edges);
        let mut path=vg::Path::new();
        for i in 0..=256 {let t=i as f32/256.0;let y=b.y+(0.5-curve.db(LN_MIN+t*LN_RANGE)/36.0).clamp(0.0,1.0)*b.h;
            if i==0 {path.move_to(b.x,y);}else{path.line_to(b.x+t*b.w,y);}}
        canvas.stroke_path(&path,&vg::Paint::color(palette.color(3)).with_line_width(2.0*scale));
        let mut text=vg::Paint::color(palette.color(1));text.set_font_size(11.0*scale);
        let font=self.label_font.get().or_else(||canvas.add_font_mem(nih_plug_vizia::assets::fonts::NOTO_SANS_REGULAR).ok());
        if let Some(font)=font {self.label_font.set(Some(font));text.set_font(&[font]);}
        for i in 0..NODE_COUNT+2 {
            if i<NODE_COUNT&&!self.params.response_nodes[i].enabled.value(){continue;}
            let(x,y)=self.position(i,b,scale);let mut node=vg::Path::new();node.circle(x,y,6.0*scale);
            canvas.fill_path(&node,&vg::Paint::color(if i==self.selected {palette.color(1)} else {palette.color(2)}));
            if i>=NODE_COUNT {let label=if i==HPF{"HPF"}else{"LPF"};let tx=if i==HPF{x+10.0*scale}else{x-30.0*scale};let _=canvas.fill_text(tx,y-10.0*scale,label,&text);}
        }
        for (frequency,label) in [(30.0_f32,"30"),(100.0,"100"),(1000.0,"1k"),(10000.0,"10k"),(22000.0,"22k")] {
            let x=b.x+(frequency.ln()-LN_MIN)/LN_RANGE*b.w;let tx=(x-8.0*scale).clamp(b.x,b.x+b.w-20.0*scale);
            let _=canvas.fill_text(tx,b.y+b.h+16.0*scale,label,&text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    const RESET_MODIFIER: Modifiers = Modifiers::LOGO;
    #[cfg(not(target_os = "macos"))]
    const RESET_MODIFIER: Modifiers = Modifiers::CTRL;
    use crate::SpectralCompressor;
    use nih_plug_vizia::vizia::context::backend::BackendContext;
    use std::sync::{Arc,Mutex};
    struct Apply(Arc<Mutex<Vec<(ParamPtr,i32)>>>);
    impl Model for Apply {
        fn event(&mut self,_cx:&mut EventContext,event:&mut Event) {
            event.map(|raw,_|match raw {
                RawParamEvent::BeginSetParameter(p)=>self.0.lock().unwrap().push((*p,1)),
                RawParamEvent::EndSetParameter(p)=>self.0.lock().unwrap().push((*p,-1)),
                RawParamEvent::SetParameterNormalized(p,v)=>crate::test_support::set_raw(*p,*v),
                RawParamEvent::ParametersChanged=>{},
            });
        }
    }
    fn fixture()->(SpectralCompressor,Context,Entity,Arc<Mutex<Vec<(ParamPtr,i32)>>>) {
        let plugin=SpectralCompressor::default();let mut cx=Context::default();
        Data {params:plugin.params.clone(),editor_mode:plugin.params.editor_mode.clone(),analyzer_data:plugin.analyzer_output_data.clone(),sample_rate:plugin.sample_rate.clone()}.build(&mut cx);
        let trace=Arc::new(Mutex::new(Vec::new()));Apply(trace.clone()).build(&mut cx);
        let e=Analyzer::new(&mut cx,Data::analyzer_data,Data::sample_rate,Data::params).entity();
        (plugin,cx,e,trace)
    }
    fn geometry(backend:&mut BackendContext,e:Entity) {
        backend.set_scale_factor(1.0);
        EventContext::new_with_current(backend.context(),e).set_bounds(BoundingBox{x:0.0,y:0.0,w:600.0,h:500.0});
    }
    fn move_to(backend:&mut BackendContext,e:Entity,x:f32,y:f32) {
        let dragging=EventContext::new_with_current(backend.context(),e).get_view::<Analyzer>().unwrap().dragging;
        if !dragging {backend.emit_origin(WindowEvent::MouseMove(x,y));backend.process_events();}
        send(backend,e,WindowEvent::MouseMove(x,y));
    }
    fn send(backend:&mut BackendContext,e:Entity,event:WindowEvent) {
        backend.send_event(Event::new(event).target(e).origin(e).propagate(Propagation::Direct));backend.process_events();
    }
    fn point(hz:f32,db:f32)->(f32,f32) {
        let b=BoundingBox {x:8.0,y:28.0,w:584.0,h:358.0};
        (b.x+(hz.ln()-LN_MIN)/LN_RANGE*b.w,b.y+(0.5-db/36.0)*b.h)
    }
    #[test]
    fn extreme_nodes_remain_inside_plot_and_can_be_regrabbed_after_release() {
        for scale in [0.5_f32,1.0,2.0,4.0] {
            for (outside,expected) in [(-1000.0_f32,18.0),(1500.0,-18.0)] {
                let(p,mut cx,e,_)=fixture();let mut backend=BackendContext::new_with_event_manager(&mut cx);
                backend.set_scale_factor(scale as f64);
                EventContext::new_with_current(backend.context(),e).set_bounds(BoundingBox{x:0.0,y:0.0,w:600.0*scale,h:500.0*scale});
                let (x,y)=point(1000.0,0.0);move_to(&mut backend,e,x*scale,y*scale);
                send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
                move_to(&mut backend,e,x*scale,outside*scale);send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));
                assert_eq!(p.params.response_nodes[0].amount.value(),expected);
                let b=plot_bounds(BoundingBox{x:0.0,y:0.0,w:600.0*scale,h:500.0*scale},scale);
                let(nx,ny)={let mut context=EventContext::new_with_current(backend.context(),e);context.get_view::<Analyzer>().unwrap().position(0,b,scale)};
                assert!(ny-6.0*scale>=b.y && ny+6.0*scale<=b.y+b.h);
                move_to(&mut backend,e,nx,ny);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
                move_to(&mut backend,e,nx,ny+if expected>0.0 {25.0*scale}else{-25.0*scale});
                send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));
                assert!(p.params.response_nodes[0].amount.value().abs()<18.0,"Could not regrab edge node at scale {scale}");
                assert!(!p.params.response_nodes[1].enabled.value(),"Regrab created a second node");
            }
        }
    }
    #[test]
    fn production_graph_constructs_its_gesture_hint_and_filter_controls() {
        let(_,mut cx,e,_)=fixture();
        assert!(cx.resolve_entity_identifier("response-gesture-hint").is_some());
        let mut context=EventContext::new_with_current(&mut cx,e);
        fn count(cx:&mut EventContext)->usize {
            let mut result=if cx.get_view::<super::super::relative_slider::RelativeParamSlider>().is_some(){1}else{0};
            let mut i=0;while let Some(child)=cx.nth_child(i){result+=cx.with_current(child,count);i+=1;}result
        }
        assert_eq!(count(&mut context),2);
    }
    #[test]
    fn retina_pointer_coordinates_create_and_drag_the_same_node() {
        let(p,mut cx,e,_)=fixture();let mut backend=BackendContext::new_with_event_manager(&mut cx);
        backend.set_scale_factor(2.0);EventContext::new_with_current(backend.context(),e).set_bounds(BoundingBox{x:0.0,y:0.0,w:1200.0,h:1000.0});
        let(x,y)=point(1000.0,-12.0);move_to(&mut backend,e,x*2.0,y*2.0);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
        let(x,y)=point(2000.0,-6.0);move_to(&mut backend,e,x*2.0,y*2.0);send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));
        let n=&p.params.response_nodes[0];assert!(n.enabled.value());assert!((n.frequency.value()-2000.0).abs()<0.05);assert!((n.amount.value()+6.0).abs()<1e-4);
    }
    #[test]
    fn click_drag_creates_and_moves_a_bell_immediately_and_balances_host_gestures() {
        let(p,mut cx,e,trace)=fixture();let mut backend=BackendContext::new_with_event_manager(&mut cx);geometry(&mut backend,e);
        let(x,y)=point(1000.0,-12.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
        let ec=EventContext::new_with_current(backend.context(),e);println!("bell bounds {:?}, cursor {} {}",ec.bounds(),ec.mouse().cursorx,ec.mouse().cursory);
        let n=&p.params.response_nodes[0];assert!(n.enabled.value());assert!(((n.frequency.value().ln()-1000.0_f32.ln())/LN_RANGE*584.0).abs()<1.0);assert!((n.amount.value()+12.0).abs()/36.0*358.0<1.0,"actual amount {}",n.amount.value());
        let(x,y)=point(2000.0,-6.0);move_to(&mut backend,e,x,y);
        assert!(((n.frequency.value().ln()-2000.0_f32.ln())/LN_RANGE*584.0).abs()<1.0);assert!((n.amount.value()+6.0).abs()/36.0*358.0<1.0);
        send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));let value=n.frequency.value();move_to(&mut backend,e,500.0,50.0);assert_eq!(n.frequency.value(),value);
        let mut gestures=std::collections::HashMap::<ParamPtr,i32>::new();
        for (p,delta) in trace.lock().unwrap().iter(){*gestures.entry(*p).or_default()+=delta;}
        assert!(gestures.values().all(|v|*v==0));
    }
    #[test]
    fn wheel_changes_selected_q_away_from_node_and_alt_click_deletes_only_bells() {
        let(p,mut cx,e,_)=fixture();let mut backend=BackendContext::new_with_event_manager(&mut cx);geometry(&mut backend,e);
        let(x,y)=point(1000.0,6.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));
        move_to(&mut backend,e,500.0,80.0);send(&mut backend,e,WindowEvent::MouseScroll(0.0,1.0));
        assert!(p.params.response_nodes[0].q.value()>1.08);
        *backend.modifiers()=Modifiers::ALT;move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
        assert!(!p.params.response_nodes[0].enabled.value());
        for hz in [30.0,22000.0] {let(x,y)=point(hz,0.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));}
        assert_eq!(p.params.response_edges.high_pass.value(),30.0);assert_eq!(p.params.response_edges.low_pass.value(),22000.0);
    }
    #[test]
    fn permanent_filter_handles_drag_only_horizontally_and_platform_reset_to_extremes() {
        let(p,mut cx,e,_)=fixture();let mut backend=BackendContext::new_with_event_manager(&mut cx);geometry(&mut backend,e);
        for (start,end,hpf) in [(30.0,300.0,true),(22000.0,6000.0,false)] {
            let(x,y)=point(start,0.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Left));
            let ec=EventContext::new_with_current(backend.context(),e);
            println!("filter press start {start}, bounds {:?}, cursor {} {}, dragging {}, selected {}",ec.bounds(),ec.mouse().cursorx,ec.mouse().cursory,ec.get_view::<Analyzer>().unwrap().dragging,ec.get_view::<Analyzer>().unwrap().selected);
            let(x,y)=point(end,12.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseUp(MouseButton::Left));
            let param=if hpf {&p.params.response_edges.high_pass}else{&p.params.response_edges.low_pass};assert!(((param.value().ln()-end.ln())/LN_RANGE*584.0).abs()<1.0,"filter {hpf} actual {} expected {end}",param.value());
            assert!(p.params.response_nodes.iter().all(|n|!n.enabled.value()));
            *backend.modifiers()=RESET_MODIFIER;let(x,y)=point(end,0.0);move_to(&mut backend,e,x,y);send(&mut backend,e,WindowEvent::MouseDown(MouseButton::Right));
            assert!((param.value()-start).abs()<0.1);*backend.modifiers()=Modifiers::empty();
        }
    }
}
