//! K-weighted matching: fast 400 ms relearn, calm 3 s steady tracking.
//! Controller and smoother for the single Output Gain stage after dry/wet mixing.
use nih_plug::prelude::Buffer;

const WINDOW_SECONDS: f64 = 0.4;
const SMOOTH_SECONDS: f64 = 0.04;
const SMART_WINDOW_SECONDS: f64 = 3.0;
const STEADY_SMOOTH_SECONDS: f64 = 1.5;
const STEADY_DB_PER_SECOND: f64 = 0.5;
const STEADY_DEADBAND_DB: f64 = 0.1;
const MAX_CORRECTION_DB: f64 = 36.0;
const MIN_POWER: f64 = 1.171465304582663e-7; // -70 LUFS: 10^((-70 + .691)/10)

#[derive(Clone, Copy, Default)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    z: [f64; 2],
}
impl Biquad {
    fn tick(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

#[derive(Clone, Copy, Default)]
struct KWeight {
    shelf: Biquad,
    high_pass: Biquad,
}
impl KWeight {
    fn new(rate: f64) -> Self {
        // BS.1770 filter design, also used by libebur128's reference implementation.
        let k = (std::f64::consts::PI * 1681.974450955533 / rate).tan();
        let q = 0.7071752369554196;
        let vh = 10.0_f64.powf(3.999843853973347 / 20.0);
        let vb = vh.powf(0.4996667741545416);
        let a0 = 1.0 + k / q + k * k;
        let shelf = Biquad {
            b: [(vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [0.0; 2],
        };
        let k = (std::f64::consts::PI * 38.13547087602444 / rate).tan();
        let q = 0.5003270373238773;
        let a0 = 1.0 + k / q + k * k;
        let high_pass = Biquad {
            b: [1.0, -2.0, 1.0],
            a: [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
            z: [0.0; 2],
        };
        Self { shelf, high_pass }
    }
    fn tick(&mut self, x: f32) -> f64 {
        self.high_pass.tick(self.shelf.tick(x as f64))
    }
}

pub(crate) struct AutoGain {
    rate: f64,
    input_filters: [KWeight; 2],
    output_filters: [KWeight; 2],
    delay: Vec<[f32; 2]>,
    delay_position: usize,
    latency: usize,
    reference_power: Vec<f64>,
    energies: Vec<[f64; 2]>,
    energy_position: usize,
    filled: usize,
    sums: [f64; 2],
    hop: usize,
    target_db: f64,
    valid_target: bool,
    current_db: f64,
    current_gain: f32,
    smoothing: f64,
    pub(crate) published_db: f32,
    slow_energies:Vec<[f64;2]>,slow_position:usize,slow_filled:usize,slow_sums:[f64;2],
    learning:usize,stable_smoothing:f64,last_enabled:bool,delta_mix:f32,delta_step:f32,
    calibrated:bool,catchup:usize,
}
impl Default for AutoGain {
    fn default() -> Self {
        Self { rate: 48000.0, input_filters: [KWeight::default(); 2],
            output_filters: [KWeight::default(); 2], delay: Vec::new(), delay_position: 0,
            latency: usize::MAX, reference_power: Vec::new(), energies: Vec::new(),
            energy_position: 0, filled: 0, sums: [0.0; 2], hop: 0,
            target_db: 0.0, valid_target: false, current_db: 0.0, current_gain: 1.0,
            smoothing: 0.0, published_db: 0.0,
            slow_energies:Vec::new(),slow_position:0,slow_filled:0,slow_sums:[0.0;2],
            learning:0,stable_smoothing:0.0,last_enabled:false,delta_mix:0.0,delta_step:0.0,calibrated:false,catchup:0 }
    }
}
impl AutoGain {
    pub(crate) fn configure(&mut self, rate: f32, max_block: usize, max_latency: usize, gain_db: f32) {
        self.rate = rate as f64;
        self.delay.resize(max_latency + 1, [0.0; 2]);
        self.reference_power.resize(max_block, 0.0);
        self.energies.resize((self.rate * WINDOW_SECONDS).round().max(1.0) as usize, [0.0; 2]);
        self.slow_energies.resize((self.rate*SMART_WINDOW_SECONDS).round().max(1.0) as usize,[0.0;2]);
        self.stable_smoothing=1.0-(-1.0/(self.rate*STEADY_SMOOTH_SECONDS)).exp();
        self.delta_step=(1.0/(self.rate*0.005)) as f32;self.delta_mix=0.0;self.last_enabled=false;
        self.smoothing = 1.0 - (-1.0 / (self.rate * SMOOTH_SECONDS)).exp();
        self.reset_analysis();
        self.set_gain(gain_db);
    }

    pub(crate) fn gain(&self) -> f32 { self.current_gain }

    pub(crate) fn set_gain(&mut self, db: f32) {
        self.current_db = if db.is_finite() { (db as f64).clamp(-50.0,50.0) } else { 0.0 };
        self.target_db = self.current_db;
        self.current_gain = 10.0_f64.powf(self.current_db / 20.0) as f32;
        self.published_db = self.current_db as f32;
    }

    pub(crate) fn reset_analysis(&mut self) {
        self.input_filters = [KWeight::new(self.rate); 2];
        self.output_filters = [KWeight::new(self.rate); 2];
        self.delay.fill([0.0; 2]);
        self.energies.fill([0.0; 2]);
        self.delay_position = 0;
        self.energy_position = 0;
        self.filled = 0;
        self.sums = [0.0; 2];
        self.hop = 0;
        self.valid_target = false;
        self.latency = usize::MAX;
        self.notify_processing_change();
    }

    /// Invalidate only the long meter in O(1); continuous drags retain fast tracking.
    pub(crate) fn notify_processing_change(&mut self) {
        self.slow_position=0;self.slow_filled=0;self.slow_sums=[0.0;2];
        self.learning=self.rate.round() as usize;self.calibrated=false;self.catchup=0;
    }

    /// Capture before any compressor/gain/mix processing. No allocations here.
    pub(crate) fn capture_input(&mut self, buffer: &Buffer, latency: usize) {
        if latency != self.latency {
            self.reset_analysis();
            self.latency = latency;
        }
        let channels = buffer.as_slice_immutable();
        for i in 0..buffer.samples() {
            let mut frame = [0.0; 2];
            for (c, samples) in channels.iter().enumerate() { frame[c] = samples[i]; }
            self.delay[self.delay_position] = frame;
            let read = (self.delay_position + self.delay.len() - latency) % self.delay.len();
            let delayed = self.delay[read];
            let mut power = 0.0;
            for c in 0..buffer.channels() {
                let weighted = self.input_filters[c].tick(delayed[c]);
                power += weighted * weighted;
            }
            self.reference_power[i] = power;
            self.delay_position = (self.delay_position + 1) % self.delay.len();
        }
    }

    fn tick(&mut self,input_power:f64,output_power:f64,enabled:bool,smart:bool)->f32 {
        if enabled && !self.last_enabled {self.notify_processing_change();}
        self.last_enabled=enabled;
        let old=self.energies[self.energy_position];let new=[input_power,output_power];
        self.energies[self.energy_position]=new;
        self.energy_position=(self.energy_position+1)%self.energies.len();
        self.filled=(self.filled+1).min(self.energies.len());
        let old_slow=if self.slow_filled==self.slow_energies.len(){self.slow_energies[self.slow_position]}else{[0.0;2]};
        self.slow_energies[self.slow_position]=new;
        self.slow_position=(self.slow_position+1)%self.slow_energies.len();
        self.slow_filled=(self.slow_filled+1).min(self.slow_energies.len());
        for c in 0..2 {
            self.sums[c]=(self.sums[c]+new[c]-old[c]).max(0.0);
            self.slow_sums[c]=(self.slow_sums[c]+new[c]-old_slow[c]).max(0.0);
        }
        // Once a full post-edit short-term window exists, converge once to its
        // average. This is not retriggered by transients or each loop repetition.
        if smart && !self.calibrated && self.slow_filled==self.slow_energies.len() {
            self.calibrated=true;self.catchup=(self.rate*0.4).round() as usize;
        }
        let fast=!smart || self.learning>0;
        let quick=fast || self.catchup>0;
        if self.hop==0 {
            let divisor=if fast {self.energies.len()}else{self.slow_filled.max(1)} as f64;
            let sums=if fast{self.sums}else{self.slow_sums};
            let input=sums[0]/divisor;let output=sums[1]/divisor;
            self.valid_target=self.filled==self.energies.len()
                && (fast||self.slow_filled>=self.energies.len())
                && self.sums[0]/self.energies.len() as f64>MIN_POWER
                && self.sums[1]/self.energies.len() as f64>MIN_POWER
                && input>MIN_POWER && output>MIN_POWER && input.is_finite() && output.is_finite();
            if enabled && self.valid_target {self.target_db=(10.0*(input/output).log10()).clamp(-MAX_CORRECTION_DB,MAX_CORRECTION_DB);}
        }
        self.hop=(self.hop+1)%64;
        if enabled && self.valid_target {
            let difference=self.target_db-self.current_db;
            let step=if quick {self.smoothing*difference} else if difference.abs()<=STEADY_DEADBAND_DB {0.0}
                else {(self.stable_smoothing*difference).clamp(-STEADY_DB_PER_SECOND/self.rate,STEADY_DB_PER_SECOND/self.rate)};
            self.current_db+=step;
            self.current_gain=10.0_f64.powf(self.current_db/20.0) as f32;
        }
        if enabled {self.learning=self.learning.saturating_sub(1);self.catchup=self.catchup.saturating_sub(1);}
        self.current_gain
    }

    /// Observe the complete raw post-plugin signal, then apply the output correction.
    pub(crate) fn process_output(&mut self, buffer: &mut Buffer, enabled: bool, smart:bool, delta:bool, dry:impl Fn(usize,usize)->f32) -> f32 {
        let count = buffer.samples();
        let channels = buffer.as_slice();
        for i in 0..count {
            let mut power = 0.0;
            for (c, samples) in channels.iter().enumerate() {
                let weighted = self.output_filters[c].tick(samples[i]);
                power += weighted * weighted;
            }
            let gain = self.tick(self.reference_power[i], power, enabled, smart);
            self.delta_mix=if delta {(self.delta_mix+self.delta_step).min(1.0)}else{(self.delta_mix-self.delta_step).max(0.0)};
            for (c,samples) in channels.iter_mut().enumerate() {
                // Metering above always sees the normal signal, independently of audition.
                if self.delta_mix==1.0 {samples[i]=dry(i,c)-samples[i];}
                else if self.delta_mix>0.0 {samples[i]+=(dry(i,c)-2.0*samples[i])*self.delta_mix;}
                if gain!=1.0 {samples[i]*=gain;}
            }
        }
        self.published_db = self.current_db as f32;
        self.published_db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_mode_relearns_parameter_steps_in_one_second() {
        for rate in [44100.0,48000.0,96000.0] {
            for difference in [-24.0,-12.0,12.0,24.0] {
                let mut a=AutoGain::default();a.configure(rate,1024,32768,0.0);
                for _ in 0..(rate*4.0) as usize {a.tick(0.01,0.01,true,true);}
                a.notify_processing_change();let out=0.01*10.0_f64.powf(-difference/10.0);
                for _ in 0..rate as usize {a.tick(0.01,out,true,true);}
                assert!((a.current_db-difference).abs()<0.15,"{rate}, {difference}, {}",a.current_db);
            }
        }
    }
    #[test]
    fn smart_mode_reduces_repeating_kick_pumping_and_matches_average_energy() {
        let rate=48000.0;let mut smart=AutoGain::default();let mut fast=AutoGain::default();
        smart.configure(rate,1024,32768,0.0);fast.configure(rate,1024,32768,0.0);
        let(mut s_min,mut s_max,mut f_min,mut f_max)=(f64::INFINITY,f64::NEG_INFINITY,f64::INFINITY,f64::NEG_INFINITY);
        for n in 0..(rate*35.0) as usize {
            let phase=(n as f64/rate as f64)%1.0;let input=if phase<0.1 {0.1}else{0.002};
            let output=input*if phase<0.1 {4.0}else{0.25};
            smart.tick(input,output,true,true);fast.tick(input,output,true,false);
            if n>(rate*30.0) as usize {s_min=s_min.min(smart.current_db);s_max=s_max.max(smart.current_db);f_min=f_min.min(fast.current_db);f_max=f_max.max(fast.current_db);}
        }
        let target=10.0_f64*((0.1_f64*0.1+0.002*0.9)/(0.4*0.1+0.0005*0.9)).log10();
        assert!((smart.current_db-target).abs()<0.2);
        assert!(s_max-s_min<0.1,"Smart ripple {} dB",s_max-s_min);
        assert!(f_max-f_min>5.0,"Momentary control should expose the regression");
    }
    #[test]
    fn k_weight_coefficients_match_itu_48k_tables() {
        let k = KWeight::new(48000.0);
        for (actual, expected) in k.shelf.b.iter().zip([1.53512485958697, -2.69169618940638, 1.19839281085285]) {
            assert!((actual - expected).abs() < 1e-12);
        }
        for (actual, expected) in k.high_pass.a.iter().zip([-1.99004745483398, 0.99007225036621]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn steps_settle_within_one_second_and_gain_moves_smoothly() {
        for rate in [44100.0, 48000.0, 96000.0] {
            for difference in [-24.0, -12.0, 12.0, 24.0] {
                let mut a = AutoGain::default(); a.configure(rate, 1024, 32768, 0.0);
                let in_power = 0.01;
                for _ in 0..rate as usize { a.tick(in_power, in_power, true, false); }
                let out_power = in_power * 10.0_f64.powf(-difference / 10.0);
                let mut previous = a.current_db;
                for _ in 0..rate as usize {
                    a.tick(in_power, out_power, true, false);
                    assert!((a.current_db - previous).abs() < 0.02);
                    previous = a.current_db;
                }
                assert!((a.current_db - difference).abs() < 0.12, "rate {rate}, target {difference}, actual {}", a.current_db);
            }
        }
    }

    #[test]
    fn switching_off_freezes_the_exact_applied_gain_despite_new_levels() {
        let mut a = AutoGain::default(); a.configure(48000.0, 1024, 32768, 0.0);
        for _ in 0..48000 { a.tick(0.01, 0.001, true, false); }
        let db = a.current_db; let gain = a.current_gain;
        for _ in 0..96000 { assert_eq!(a.tick(0.001, 0.2, false, false), gain); }
        assert_eq!(a.current_db, db);
        for _ in 0..48000 { a.tick(0.001, 0.2, true, false); }
        assert!((a.current_db + 23.0102999566).abs() < 0.12);
    }

    #[test]
    fn silence_and_missing_output_do_not_cause_runaway_gain() {
        for powers in [[0.0,0.0], [0.01,0.0], [1e-12,1e-15]] {
            let mut a=AutoGain::default(); a.configure(48000.0,1024,32768,7.0);
            for _ in 0..96000 { a.tick(powers[0],powers[1],true,true); }
            assert_eq!(a.current_db,7.0);
        }
    }

    #[test]
    fn persisted_gain_is_sanitized_and_analysis_resets_preserve_it() {
        let mut a=AutoGain::default(); a.configure(48000.0,1024,32768,8.0);
        a.reset_analysis(); assert_eq!(a.current_db,8.0);
        a.set_gain(f32::NAN); assert_eq!(a.current_db,0.0);
        a.set_gain(999.0); assert_eq!(a.current_db,50.0);
    }
}
