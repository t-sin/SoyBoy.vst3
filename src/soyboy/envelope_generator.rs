use crate::soyboy::{
    event::{Event, Triggered},
    parameters::{Parametric, SoyBoyParameter},
    types::AudioProcessor,
    utils::{discrete_loudness, level_from_velocity, linear},
};

#[derive(Debug)]
pub enum EnvelopeState {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
}

pub struct EnvelopeGenerator {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,

    velocity: f64,
    state: EnvelopeState,
    elapsed_samples: u64,
    last_value: f64,
    last_state_value: f64,
}

impl EnvelopeGenerator {
    pub fn new() -> EnvelopeGenerator {
        EnvelopeGenerator {
            attack: 0.05,
            decay: 0.05,
            sustain: 0.3,
            release: 0.1,

            velocity: 0.0,
            state: EnvelopeState::Off,
            elapsed_samples: 1,
            last_value: 0.0,
            last_state_value: 0.0,
        }
    }

    pub fn set_state(&mut self, state: EnvelopeState) {
        match self.state {
            EnvelopeState::Attack => self.last_state_value = self.last_value,
            EnvelopeState::Decay => self.last_state_value = self.last_value,
            EnvelopeState::Sustain => self.last_state_value = self.last_value,
            _ => (),
        }
        self.state = state;
        self.elapsed_samples = 0;
    }

    fn update_state(&mut self, s: f64) {
        match self.state {
            EnvelopeState::Attack => {
                if s > self.attack {
                    self.set_state(EnvelopeState::Decay);
                    self.last_state_value = 1.0;
                }
            }
            EnvelopeState::Decay => {
                if s > self.decay {
                    self.set_state(EnvelopeState::Sustain);
                }
            }
            EnvelopeState::Sustain => (),
            EnvelopeState::Release => {
                if s > self.release {
                    self.set_state(EnvelopeState::Off);
                }
            }
            EnvelopeState::Off => (),
        };
    }

    fn calculate(&mut self, s: f64) -> f64 {
        match self.state {
            EnvelopeState::Attack => linear(s, 1.0 / self.attack),
            EnvelopeState::Decay => {
                let max = self.last_state_value - self.sustain;
                self.last_state_value - max * linear(s, 1.0 / self.decay)
            }
            EnvelopeState::Sustain => self.sustain,
            EnvelopeState::Release => {
                let max = self.last_state_value;
                max - max * linear(s, 1.0 / self.release)
            }
            EnvelopeState::Off => 0.0,
        }
    }
}

impl AudioProcessor<f64> for EnvelopeGenerator {
    fn process(&mut self, sample_rate: f64) -> f64 {
        let s = self.elapsed_samples as f64 / sample_rate;

        self.update_state(s);
        let v = self.calculate(s);
        self.last_value = v;
        self.elapsed_samples += 1;

        discrete_loudness(v * level_from_velocity(self.velocity))
    }

    fn set_freq(&mut self, _freq: f64) {}
}

impl Triggered for EnvelopeGenerator {
    fn trigger(&mut self, event: &Event) {
        match event {
            Event::NoteOn { note: _, velocity } => {
                self.set_state(EnvelopeState::Attack);
                self.velocity = *velocity;
            }
            Event::NoteOff { note: _ } => self.set_state(EnvelopeState::Release),
            _ => (),
        }
    }
}

impl Parametric<SoyBoyParameter> for EnvelopeGenerator {
    fn set_param(&mut self, param: &SoyBoyParameter, value: f64) {
        match param {
            SoyBoyParameter::EgAttack => self.attack = value,
            SoyBoyParameter::EgDecay => self.decay = value,
            SoyBoyParameter::EgSustain => self.sustain = value,
            SoyBoyParameter::EgRelease => self.release = value,
            _ => (),
        }
    }

    fn get_param(&self, param: &SoyBoyParameter) -> f64 {
        match param {
            SoyBoyParameter::EgAttack => self.attack,
            SoyBoyParameter::EgDecay => self.decay,
            SoyBoyParameter::EgSustain => self.sustain,
            SoyBoyParameter::EgRelease => self.release,
            _ => 0.0,
        }
    }
}
