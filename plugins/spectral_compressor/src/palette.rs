// SPDX-License-Identifier: GPL-3.0-or-later
// Turbo modifications copyright (C) 2026 ferropop
//! UI-only palettes. Four base colors, with surface tints for readable controls.
use serde::{Deserialize,Serialize};
use std::{path::PathBuf,sync::{Arc,Mutex}};
use nih_plug_vizia::vizia::{util::IntoCssStr,vg};
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Palette {pub name:String,pub colors:[u32;4]}
impl Palette {
    pub fn new(name:&str,colors:[u32;4])->Self {Self{name:name.into(),colors}}
    pub fn color(&self,i:usize)->vg::Color {let c=self.colors[i];vg::Color::rgb((c>>16) as u8,(c>>8) as u8,c as u8)}
    pub fn tint(&self,i:usize,t:f32)->u32 {blend(self.colors[0],self.colors[i],t)}
    pub fn readable(&self)->bool {self.colors.iter().all(|c|*c<=0xffffff) && contrast(self.colors[0],self.colors[1])>=4.5 && contrast(self.colors[0],self.colors[2])>=3.0 && contrast(self.colors[0],self.colors[3])>=3.0}
    pub fn css(&self)->String {
        let replacements=[("#181c22",self.colors[0]),("#e5e9ef",self.colors[1]),("#394350",self.colors[2]),("#f0f3f7",self.colors[1]),("#aab7c7",self.tint(1,0.75)),("#f4c069",self.colors[3]),("#85909f",self.tint(1,0.55)),("#252d38",self.tint(1,0.07)),("#69798d",self.colors[2]),("#354152",self.tint(2,0.18)),("#41536b",self.tint(2,0.25)),("#9db3d1",self.colors[2]),("#202630",self.tint(1,0.04)),("#edf0f5",self.colors[1]),("#293343",self.tint(2,0.12)),("#40546c",self.tint(2,0.23)),("#76c1f2",self.colors[2]),("#12171d",self.colors[0]),("#899fb8",self.tint(1,0.45)),("#b5bec8",self.tint(1,0.8)),("#202731",self.tint(1,0.07)),("#244c3c",self.tint(3,0.25)),("#79d6a8",self.colors[3]),("#d0f4e2",self.colors[1]),("#2b604a",self.tint(3,0.35)),("#c0cede",self.colors[1])];
        // Replace via temporary tokens so a chosen color equal to an old color isn't replaced twice.
        let mut css=include_str!("editor/theme.css").to_string();
        for (i,(from,_)) in replacements.iter().enumerate(){css=css.replace(from,&format!("@P{i}@"));}
        for (i,(_,color)) in replacements.iter().enumerate(){css=css.replace(&format!("@P{i}@"),&format!("#{color:06x}"));}
        css
    }
}
pub fn builtins()->Vec<Palette> {vec![
    Palette::new("CGA",[0x000000,0xffffff,0x8bfcfb,0xdc40f0]),
    Palette::new("Turbo Dark",[0x181c22,0xe5e9ef,0x76c1f2,0xf4c069]),
    Palette::new("Amber",[0x15100c,0xfbe7c2,0xffbc65,0xd996e3]),
    Palette::new("Ocean",[0x071c23,0xeaf6f8,0x61d9db,0xb6a5ff]),
    Palette::new("Paper",[0xfafafa,0x12171d,0x146c94,0xa54575]),
]}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct PaletteState {pub active:Palette,pub custom:Vec<Palette>}
impl Default for PaletteState {
    fn default()->Self {Self {active:builtins().remove(0),custom:load_library()}}
}
impl PaletteState {
    pub fn active(&self)->Palette {if self.active.readable(){self.active.clone()}else{builtins().remove(0)}}
    pub fn presets(&self)->Vec<Palette> {let mut p=builtins();p.extend(self.custom.iter().filter(|p|p.readable()).cloned());p}
    pub fn save(&mut self,palette:Palette)->Result<(),String> {
        if !palette.readable(){return Err("Increase text/accent contrast against the background.".into());}
        if palette.name.trim().is_empty() || builtins().iter().any(|p|p.name==palette.name){return Err("Choose a custom name, different from a built-in preset.".into());}
        if let Some(p)=self.custom.iter_mut().find(|p|p.name==palette.name){*p=palette.clone();}else{self.custom.push(palette.clone());}
        self.active=palette;Ok(())
    }
    pub fn delete(&mut self,name:&str) {self.custom.retain(|p|p.name!=name);if self.active.name==name{self.active=builtins().remove(0);}}
}
pub struct PaletteStyle(pub Arc<Mutex<PaletteState>>);
impl IntoCssStr for PaletteStyle {fn get_style(&self)->Result<String,std::io::Error>{Ok(self.0.lock().unwrap().active().css())}}
pub fn parse_hex(text:&str)->Option<u32> {let text=text.trim().trim_start_matches('#');if text.len()!=6{return None;}u32::from_str_radix(text,16).ok().filter(|c|*c<=0xffffff)}
fn blend(a:u32,b:u32,t:f32)->u32 {let mut out=0;for shift in [0,8,16] {let x=((a>>shift)&255) as f32;let y=((b>>shift)&255) as f32;out|=((x+(y-x)*t).round() as u32)<<shift;}out}
fn luminance(c:u32)->f64 {let mut l=0.0;for(shift,w)in [(16,0.2126),(8,0.7152),(0,0.0722)] {let v=((c>>shift)&255) as f64/255.0;l+=w*if v<=0.04045 {v/12.92}else{((v+0.055)/1.055).powf(2.4)};}l}
fn contrast(a:u32,b:u32)->f64 {let(a,b)=(luminance(a),luminance(b));(a.max(b)+0.05)/(a.min(b)+0.05)}
fn library_path()->Option<PathBuf> {
    #[cfg(windows)] {std::env::var_os("APPDATA").map(|p|PathBuf::from(p).join("ferropop/Spectral Compressor Turbo/palettes.json"))}
    #[cfg(target_os="macos")] {std::env::var_os("HOME").map(|p|PathBuf::from(p).join("Library/Application Support/ferropop/Spectral Compressor Turbo/palettes.json"))}
    #[cfg(all(not(windows),not(target_os="macos")))] {std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(||std::env::var_os("HOME").map(|p|PathBuf::from(p).join(".config"))).map(|p|p.join("ferropop/spectral-compressor-turbo/palettes.json"))}
}
fn load_library()->Vec<Palette> {library_path().and_then(|p|std::fs::read(p).ok()).and_then(|bytes|serde_json::from_slice::<Vec<Palette>>(&bytes).ok()).unwrap_or_default().into_iter().filter(Palette::readable).collect()}
pub fn save_library(state:&PaletteState)->Result<(),String> {
    let path=library_path().ok_or("Could not locate your settings folder.")?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e|e.to_string())?;
    let bytes=serde_json::to_vec_pretty(&state.custom).map_err(|e|e.to_string())?;
    let temporary=path.with_extension("tmp");std::fs::write(&temporary,bytes).map_err(|e|e.to_string())?;
    // Windows cannot rename over an existing file. Copy then remove only our temporary file.
    std::fs::copy(&temporary,&path).map_err(|e|e.to_string())?;let _=std::fs::remove_file(temporary);Ok(())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn cga_matches_attachment_and_all_presets_have_readable_contrast(){assert_eq!(builtins()[0].colors,[0,0xffffff,0x8bfcfb,0xdc40f0]);for p in builtins(){assert!(p.readable(),"{}",p.name);assert!(!p.css().contains("@P"));}}
    #[test] fn custom_palette_save_recall_delete_and_state_round_trip(){let mut s=PaletteState{active:builtins()[0].clone(),custom:vec![]};let mut p=builtins()[3].clone();p.name="Mine".into();s.save(p.clone()).unwrap();let bytes=serde_json::to_vec(&s).unwrap();let mut restored:PaletteState=serde_json::from_slice(&bytes).unwrap();assert_eq!(restored.active,p);assert_eq!(restored.presets().last(),Some(&p));restored.delete("Mine");assert_eq!(restored.active.name,"CGA");assert!(restored.custom.is_empty());}
    #[test] fn invalid_hex_and_unreadable_palettes_are_rejected(){assert_eq!(parse_hex("#8BFCFB"),Some(0x8bfcfb));assert_eq!(parse_hex("#xyzxyz"),None);assert_eq!(parse_hex("#FFF"),None);let p=Palette::new("Bad",[0,0,0,0]);assert!(!p.readable());let s=PaletteState{active:p,custom:vec![]};assert_eq!(s.active().name,"CGA");}
}
