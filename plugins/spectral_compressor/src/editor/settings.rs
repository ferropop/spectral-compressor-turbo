// SPDX-License-Identifier: GPL-3.0-or-later
// Turbo modifications copyright (C) 2026 ferropop
use super::Data;
use crate::{palette::{self,Palette,PaletteState},SpectralCompressorParams};
use nih_plug_vizia::{vizia::prelude::*,widgets::{ParamButton,ParamButtonExt,RawParamEvent}};
use std::sync::{Arc,Mutex};
#[derive(Clone,Lens)]
pub struct Settings {
    pub open:bool,pub current:String,pub name:String,pub background:String,pub text:String,pub primary:String,pub secondary:String,pub custom_names:Vec<String>,pub status:String,
    #[lens(ignore)] state:Arc<Mutex<PaletteState>>,
    #[lens(ignore)] observed:PaletteState,
}
#[derive(Clone)]
pub enum SettingsEvent {Toggle,Close,Select(String),Edit(usize,String),Name(String),Apply,Save,Delete}
impl Settings {
    pub fn new(params:&SpectralCompressorParams)->Self {
        let observed=params.palette.lock().unwrap().clone();let p=observed.active();
        Self{open:false,current:p.name.clone(),name:"My palette".into(),background:format!("#{:06X}",p.colors[0]),text:format!("#{:06X}",p.colors[1]),primary:format!("#{:06X}",p.colors[2]),secondary:format!("#{:06X}",p.colors[3]),custom_names:observed.custom.iter().map(|p|p.name.clone()).collect(),status:String::new(),state:params.palette.clone(),observed}
    }
    fn sync(&mut self,cx:&mut EventContext) {
        let state=self.state.lock().unwrap().clone();if state==self.observed{return;}
        let p=state.active();self.current=p.name.clone();
        self.background=format!("#{:06X}",p.colors[0]);self.text=format!("#{:06X}",p.colors[1]);self.primary=format!("#{:06X}",p.colors[2]);self.secondary=format!("#{:06X}",p.colors[3]);
        self.name=if palette::builtins().iter().any(|b|b.name==p.name){"My palette".into()}else{p.name};
        self.custom_names=state.custom.iter().map(|p|p.name.clone()).collect();self.observed=state;
        let _=cx.reload_styles();cx.needs_redraw();
    }
    fn draft(&self,name:&str)->Result<Palette,String> {
        let mut colors=[0;4];for(i,text)in [&self.background,&self.text,&self.primary,&self.secondary].iter().enumerate(){colors[i]=palette::parse_hex(text).ok_or("Use a six-digit hex color, such as #8BFCFB.")?;}
        let p=Palette::new(name,colors);if !p.readable(){return Err("Choose text and accents with more contrast against the background.".into());}Ok(p)
    }
}
impl Model for Settings {
    fn event(&mut self,cx:&mut EventContext,event:&mut Event) {
        event.map(|raw:&RawParamEvent,_| {if matches!(raw,RawParamEvent::ParametersChanged){self.sync(cx);}});
        event.map(|message:&SettingsEvent,meta| {
            match message {
                SettingsEvent::Toggle=>self.open=!self.open,
                SettingsEvent::Close=>self.open=false,
                SettingsEvent::Select(name)=>{let choice=self.state.lock().unwrap().presets().into_iter().find(|p|p.name==*name);if let Some(p)=choice{self.state.lock().unwrap().active=p;self.status.clear();self.sync(cx);}},
                SettingsEvent::Edit(i,text)=>match i {0=>self.background=text.clone(),1=>self.text=text.clone(),2=>self.primary=text.clone(),_=>self.secondary=text.clone()},
                SettingsEvent::Name(name)=>self.name=name.clone(),
                SettingsEvent::Apply=>match self.draft("Custom") {Ok(p)=>{let name=self.name.clone();self.state.lock().unwrap().active=p;self.status="Applied. Save a name to keep this preset.".into();self.sync(cx);self.name=name;},Err(e)=>self.status=e},
                SettingsEvent::Save=>match self.draft(self.name.trim()) {Ok(p)=>{
                    let result={let mut state=self.state.lock().unwrap();state.save(p).and_then(|_|palette::save_library(&state))};
                    self.status=match result{Ok(())=>"Palette saved.".into(),Err(e)=>e};self.sync(cx);
                },Err(e)=>self.status=e},
                SettingsEvent::Delete=>{self.state.lock().unwrap().delete(&self.current);let result=palette::save_library(&self.state.lock().unwrap());self.status=result.err().unwrap_or_else(||"Custom palette deleted.".into());self.sync(cx);},
            }
            meta.consume();
        });
    }
}
pub fn build(cx:&mut Context) {
    Popup::new(cx,Settings::open,true,|cx| {
        VStack::new(cx,|cx| {
            HStack::new(cx,|cx| {
                Label::new(cx,"SETTINGS · PALETTE MANAGER").font_size(17.0).width(Stretch(1.0));
                Button::new(cx,|cx|cx.emit(SettingsEvent::Close),|cx|Label::new(cx,"Close")).id("close-settings").width(Pixels(60.0));
            }).height(Pixels(28.0));
            Label::new(cx,Settings::current.map(|name|format!("Active palette: {name}"))).font_size(11.0).height(Pixels(18.0));
            HStack::new(cx,|cx|for p in palette::builtins() {
                let name=p.name.clone();Button::new(cx,move|cx|cx.emit(SettingsEvent::Select(name.clone())),move|cx|Label::new(cx,&p.name))
                    .class("palette-preset").width(Stretch(1.0)).height(Pixels(27.0));
            }).height(Pixels(27.0)).col_between(Pixels(6.0));
            ScrollView::new(cx,0.0,0.0,false,true,|cx| {
                List::new(cx,Settings::custom_names,|cx,_,item| {
                    Button::new(cx,move|cx|cx.emit(SettingsEvent::Select(item.get(cx))),move|cx|Label::new(cx,item))
                        .height(Pixels(24.0)).width(Stretch(1.0));
                });
            }).height(Pixels(52.0));
            HStack::new(cx,|cx| {
                color_field(cx,"Background",Settings::background,0);
                color_field(cx,"Text",Settings::text,1);
                color_field(cx,"Threshold / meters",Settings::primary,2);
                color_field(cx,"Response",Settings::secondary,3);
            }).height(Pixels(65.0)).col_between(Pixels(8.0));
            HStack::new(cx,|cx| {
                Textbox::new(cx,Settings::name).on_edit(|cx,text|cx.emit(SettingsEvent::Name(text))).width(Stretch(1.0));
                Button::new(cx,|cx|cx.emit(SettingsEvent::Apply),|cx|Label::new(cx,"Apply")).width(Pixels(58.0));
                Button::new(cx,|cx|cx.emit(SettingsEvent::Save),|cx|Label::new(cx,"Save preset")).width(Pixels(92.0));
                Button::new(cx,|cx|cx.emit(SettingsEvent::Delete),|cx|Label::new(cx,"Delete custom"))
                    .disabled(Settings::current.map(|name|palette::builtins().iter().any(|p|p.name==*name)))
                    .width(Pixels(108.0));
            }).height(Pixels(28.0)).col_between(Pixels(6.0));
            Label::new(cx,Settings::status).font_size(11.0).height(Pixels(28.0)).width(Stretch(1.0)).text_wrap(true);
            ParamButton::new(cx,Data::params,|p|&p.smart_gain).with_label("Smart gain averaging")
                .disable_scroll_wheel().id("smart-gain-toggle").height(Pixels(28.0));
            Label::new(cx,"Smart: average 3 seconds and track gently; relearn quickly after processing edits.\nOff: original 400 ms momentary matching.")
                .text_wrap(true).font_size(11.0).height(Pixels(34.0)).width(Stretch(1.0));
            Label::new(cx,"Delta monitors dry − processed mix before Output Gain. Auto Gain keeps measuring the normal processed signal.")
                .text_wrap(true).font_size(11.0).height(Pixels(32.0)).width(Stretch(1.0));
        }).row_between(Pixels(8.0)).child_space(Pixels(12.0));
    }).on_blur(|cx|cx.emit(SettingsEvent::Close)).class("palette-settings")
        .left(Pixels(8.0)).top(Pixels(86.0)).width(Pixels(664.0)).height(Pixels(462.0)).z_index(2000);
}
fn color_field<L:Lens<Target=String>>(cx:&mut Context,label:&str,lens:L,index:usize) {
    VStack::new(cx,|cx| {
        Label::new(cx,label).font_size(10.0).height(Pixels(16.0));
        Element::new(cx).height(Pixels(15.0)).background_color(lens.map(|text| {
            let c=palette::parse_hex(text).unwrap_or(0);Color::rgb((c>>16)as u8,(c>>8)as u8,c as u8)
        }));
        Textbox::new(cx,lens).on_edit(move|cx,text|cx.emit(SettingsEvent::Edit(index,text))).height(Pixels(26.0)).width(Stretch(1.0)).font_size(11.0);
    }).width(Stretch(1.0)).row_between(Pixels(3.0));
}
#[cfg(test)] mod tests {
    use super::*;
    use crate::SpectralCompressor;
    use nih_plug_vizia::vizia::context::backend::BackendContext;
    #[test] fn production_settings_button_opens_manager_and_presets_apply_live() {
        let plugin=SpectralCompressor::default();let mut cx=Context::default();
        Data{params:plugin.params.clone(),editor_mode:plugin.params.editor_mode.clone(),analyzer_data:plugin.analyzer_output_data.clone(),sample_rate:plugin.sample_rate.clone()}.build(&mut cx);
        Settings::new(&plugin.params).build(&mut cx);
        cx.add_stylesheet(palette::PaletteStyle(plugin.params.palette.clone())).unwrap();
        super::super::main_column(&mut cx);build(&mut cx);
        let button=cx.resolve_entity_identifier("open-settings").unwrap();
        let mut backend=BackendContext::new_with_event_manager(&mut cx);
        backend.send_event(Event::new(WindowEvent::Press{mouse:false}).target(button).origin(button).propagate(Propagation::Direct));backend.process_events();
        assert!(backend.context().data::<Settings>().unwrap().open);
        backend.emit_origin(SettingsEvent::Select("Paper".into()));backend.process_events();
        assert_eq!(plugin.params.palette.lock().unwrap().active.name,"Paper");
        backend.emit_origin(SettingsEvent::Select("CGA".into()));backend.process_events();
        assert_eq!(plugin.params.palette.lock().unwrap().active.colors,[0,0xffffff,0x8bfcfb,0xdc40f0]);
        backend.emit_origin(SettingsEvent::Close);backend.process_events();assert!(!backend.context().data::<Settings>().unwrap().open);
    }
}
