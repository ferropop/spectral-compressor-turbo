//! EQ-like sensitivity curve: scales compressor gain changes, never audio bins.
use nih_plug::prelude::*;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
pub const NODE_COUNT: usize = 8;
pub const HPF: usize = NODE_COUNT;
pub const LPF: usize = NODE_COUNT + 1;
pub const MIN_HZ: f32 = 30.0;
pub const MAX_HZ: f32 = 22000.0;

#[derive(Params)]
pub struct ResponseNode {
    #[id="response_on"] pub enabled: BoolParam,
    #[id="response_freq"] pub frequency: FloatParam,
    #[id="response_amount"] pub amount: FloatParam,
    #[id="response_q"] pub q: FloatParam,
}
fn frequency(name:String,default:f32,dirty:Arc<AtomicBool>)->FloatParam {
    FloatParam::new(name,default,FloatRange::Skewed {min:MIN_HZ,max:MAX_HZ,factor:FloatRange::skew_factor(-2.0)})
        .hide_in_generic_ui()
        .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
        .with_string_to_value(formatters::s2v_f32_hz_then_khz())
        .with_callback(Arc::new(move |_| {dirty.store(true,Ordering::Release);} ))
}
fn q(name:String,dirty:Arc<AtomicBool>)->FloatParam {
    FloatParam::new(name,1.0,FloatRange::Skewed {min:0.2,max:10.0,factor:FloatRange::skew_factor(-1.0)})
        .hide_in_generic_ui().with_value_to_string(Arc::new(|v|format!("{v:.2}")))
        .with_callback(Arc::new(move |_| {dirty.store(true,Ordering::Release);} ))
}
impl ResponseNode {
    pub fn new(index:usize,dirty:Arc<AtomicBool>)->Self {
        let number=index+1; let flag=dirty.clone();
        let enabled=BoolParam::new(format!("Response {number} On"),false).hide_in_generic_ui()
            .with_callback(Arc::new(move |_| {flag.store(true,Ordering::Release);}));
        let flag=dirty.clone();
        let amount=FloatParam::new(format!("Response {number} Strength"),0.0,FloatRange::Linear {min:-18.0,max:18.0})
            .with_unit(" dB").hide_in_generic_ui().with_value_to_string(Arc::new(|v|format!("{v:+.1}")))
            .with_callback(Arc::new(move |_| {flag.store(true,Ordering::Release);}));
        Self {enabled,amount,frequency:frequency(format!("Response {number} Frequency"),1000.0,dirty.clone()),q:q(format!("Response {number} Q"),dirty)}
    }
}
#[derive(Params)]
pub struct ResponseEdges {
    #[id="response_hp_freq"] pub high_pass: FloatParam,
    #[id="response_hp_q"] pub high_pass_q: FloatParam,
    #[id="response_lp_freq"] pub low_pass: FloatParam,
    #[id="response_lp_q"] pub low_pass_q: FloatParam,
}
impl ResponseEdges {
    pub fn new(dirty:Arc<AtomicBool>)->Self {
        Self { high_pass:frequency("Response HPF Frequency".into(),MIN_HZ,dirty.clone()),
            low_pass:frequency("Response LPF Frequency".into(),MAX_HZ,dirty.clone()),
            high_pass_q:q("Response HPF Q".into(),dirty.clone()),low_pass_q:q("Response LPF Q".into(),dirty) }
    }
}
#[derive(Clone,Copy,Default)]
struct Bell {center:f32,width:f32,amount:f32}
pub struct ResponseCurve {bells:[Bell;NODE_COUNT],count:usize,hp:Option<(f32,f32)>,lp:Option<(f32,f32)>}
impl ResponseCurve {
    pub fn new(nodes:&[ResponseNode;NODE_COUNT],edges:&ResponseEdges)->Self {
        let mut curve=Self {bells:[Bell::default();NODE_COUNT],count:0,hp:None,lp:None};
        for node in nodes {
            if node.enabled.value() {
                curve.bells[curve.count]=Bell {center:node.frequency.value().ln(),width:node.q.value()*2.0/std::f32::consts::LN_2,amount:node.amount.value()};
                curve.count+=1;
            }
        }
        // Extreme positions are a true bypass, so the default curve is neutral
        // for all bins, including DC and frequencies above the visible graph.
        if edges.high_pass.value()>MIN_HZ+0.01 {curve.hp=Some((edges.high_pass.value().ln(),4.0*edges.high_pass_q.value()));}
        if edges.low_pass.value()<MAX_HZ-0.1 {curve.lp=Some((edges.low_pass.value().ln(),4.0*edges.low_pass_q.value()));}
        curve
    }
    pub fn weight(&self,ln_frequency:f32)->f32 {
        let mut db=0.0;
        if ln_frequency.is_finite() {
            for bell in &self.bells[..self.count] {
                let distance=(ln_frequency-bell.center)*bell.width;
                db+=bell.amount*(-0.5*distance*distance).exp();
            }
        }
        let mut weight=util::db_to_gain(db.clamp(-36.0,18.0));
        if let Some((cutoff,slope))=self.hp {
            weight*=if ln_frequency.is_finite() {(1.0/(1.0+((cutoff-ln_frequency)*slope).exp())).sqrt()} else {0.0};
        }
        if let Some((cutoff,slope))=self.lp {
            if ln_frequency.is_finite() {weight*=(1.0/(1.0+((ln_frequency-cutoff)*slope).exp())).sqrt();}
        }
        weight
    }
    pub fn db(&self,ln_frequency:f32)->f32 {20.0*self.weight(ln_frequency).max(0.0001).log10()}
}
pub fn apply_response(gain_db:f32,weight:f32)->f32 {
    if weight==1.0 {gain_db} else {(gain_db*weight).clamp(-120.0,36.0)}
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{set_raw,Setter};
    #[test] fn curve_controls_processing_not_audio_gain() {
        assert_eq!(apply_response(0.0,4.0),0.0);
        assert_eq!(apply_response(-12.0,0.25),-3.0);
        assert_eq!(apply_response(-3.0,4.0),-12.0);
        assert_eq!(apply_response(6.0,0.25),1.5);
        assert_eq!(apply_response(-150.0,1.0),-150.0);
    }
    #[test] fn bells_and_filters_shape_only_the_requested_frequency_regions() {
        let dirty=Arc::new(AtomicBool::new(true));
        let nodes=std::array::from_fn(|i|ResponseNode::new(i,dirty.clone()));let edges=ResponseEdges::new(dirty);
        let flat=ResponseCurve::new(&nodes,&edges);
        for freq in [0.0_f32,20.0,1000.0,22000.0,48000.0] {assert_eq!(flat.weight(freq.ln()),1.0);}
        set_raw(nodes[0].enabled.as_ptr(),1.0);
        Setter.set_parameter_normalized(&nodes[0].amount,nodes[0].amount.preview_normalized(-12.0),false);
        let dip=ResponseCurve::new(&nodes,&edges);
        assert!((dip.weight(1000.0_f32.ln())-util::db_to_gain(-12.0)).abs()<1e-5);
        assert!(dip.weight(100.0_f32.ln())>0.99);
        Setter.set_parameter_normalized(&nodes[0].q,nodes[0].q.preview_normalized(4.0),false);
        assert!(ResponseCurve::new(&nodes,&edges).weight(1500.0_f32.ln())>dip.weight(1500.0_f32.ln()));
        Setter.set_parameter_normalized(&edges.high_pass,edges.high_pass.preview_normalized(300.0),false);
        Setter.set_parameter_normalized(&edges.low_pass,edges.low_pass.preview_normalized(6000.0),false);
        let restricted=ResponseCurve::new(&nodes,&edges);
        assert!(restricted.weight(60.0_f32.ln())<0.05);
        assert!(restricted.weight(20000.0_f32.ln())<0.1);
        assert!(restricted.weight(3000.0_f32.ln())>0.96);
    }
}
