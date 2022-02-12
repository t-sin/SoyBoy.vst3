use rand::prelude::*;

use crate::soyboy::{
    event::{Event, Triggered},
    parameters::{Parameter, Parametric},
    types::{i4, AudioProcessor},
};

const WAVETABLE_SIZE: usize = 32;
const WAVETABLE_SIZE_F64: f64 = WAVETABLE_SIZE as f64;

pub struct WaveTableOscillator {
    phase: f64,
    pitch: f64,
    pub freq: f64,

    table: [i4; WAVETABLE_SIZE],
    index: usize,
}

impl WaveTableOscillator {
    pub fn new() -> Self {
        let mut osc = WaveTableOscillator {
            phase: 0.0,
            freq: 0.0,
            pitch: 0.0,

            table: [i4::from(0.0); WAVETABLE_SIZE],
            index: 0,
        };

        osc.initialize_table();
        osc
    }

    fn randomize_table(&mut self) {
        for e in self.table.iter_mut() {
            let v = random::<f64>() * i4::max();
            *e = i4::from(v);
        }
    }

    fn initialize_table(&mut self) {
        let mut phase: f64 = 0.0;
        for e in self.table.iter_mut() {
            let v = (phase * 2.0 * std::f64::consts::PI).sin() * i4::max();
            *e = i4::from(v);
            phase += 1.0 / WAVETABLE_SIZE as f64;
        }
    }
}

impl Triggered for WaveTableOscillator {
    fn trigger(&mut self, event: &Event) {
        match event {
            Event::PitchBend { ratio } => {
                self.pitch = *ratio;
            } //Event::SetTable { idx } => {}
            _ => (),
        }
    }
}

impl Parametric<Parameter> for WaveTableOscillator {
    fn set_param(&mut self, param: &Parameter, value: f64) {
        match param {
            Parameter::OscWtTableIndex => self.index = value as usize % self.table.len(),
            Parameter::OscWtTableValue => self.table[self.index] = i4::from(value),
            _ => (),
        }
    }

    fn get_param(&self, param: &Parameter) -> f64 {
        match param {
            Parameter::OscWtTableIndex => self.index as f64,
            Parameter::OscWtTableValue => f64::from(self.table[self.index]),
            _ => 0.0,
        }
    }
}

impl AudioProcessor<i4> for WaveTableOscillator {
    fn process(&mut self, sample_rate: f64) -> i4 {
        let v = self.table[self.phase as usize];

        let phase_diff = ((self.freq * self.pitch) / sample_rate) * WAVETABLE_SIZE_F64;
        self.phase = (self.phase + phase_diff) % WAVETABLE_SIZE_F64;

        i4::from(v)
    }

    fn set_freq(&mut self, freq: f64) {
        self.freq = freq;
    }
}
