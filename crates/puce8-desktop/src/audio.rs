//! Sortie audio (E34c1) : flux cpal, tampon partage, dernier echantillon repete si le tampon est vide.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Tampon partage entre l'emulation (producteur) et le callback cpal (consommateur).
pub type Tampon = Arc<Mutex<VecDeque<f32>>>;

/// Cible du tampon : 50 ms au taux du peripherique (2 205 echantillons a 44 100 Hz).
pub fn cible(sample_rate: u32) -> usize {
    (sample_rate / 20) as usize
}

/// Sortie audio active.
pub struct Audio {
    /// Flux cpal : il doit rester vivant (sinon le son coupe).
    _stream: cpal::Stream,
    tampon: Tampon,
    /// Taux du peripherique (Hz) : donne a `Apu::set_sample_rate`.
    pub sample_rate: u32,
}

impl Audio {
    /// Sortie par defaut, format f32 ; None si aucun peripherique ou format non gere.
    pub fn new() -> Option<Audio> {
        let device = cpal::default_host().default_output_device()?;
        let supported = device.default_output_config().ok()?;
        if supported.sample_format() != cpal::SampleFormat::F32 {
            return None;
        }
        let config = supported.config();
        let canaux = usize::from(config.channels.max(1));
        let sample_rate = config.sample_rate.0;
        let tampon: Tampon = Arc::new(Mutex::new(VecDeque::new()));
        let t = Arc::clone(&tampon);
        let mut dernier = 0.0f32;
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut q = t.lock().unwrap_or_else(|e| e.into_inner());
                    for trame in data.chunks_mut(canaux) {
                        if let Some(s) = q.pop_front() {
                            dernier = s;
                        }
                        trame.fill(dernier); // mono duplique sur tous les canaux
                    }
                },
                |e| eprintln!("erreur audio : {e}"),
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(Audio {
            _stream: stream,
            tampon,
            sample_rate,
        })
    }

    /// Nombre d'echantillons en attente.
    pub fn queued(&self) -> usize {
        self.tampon.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Ajoute des echantillons ; `son = false` (muet) : des zeros de meme longueur (la cadence reste la meme).
    pub fn push(&self, samples: &[f32], son: bool) {
        let mut q = self.tampon.lock().unwrap_or_else(|e| e.into_inner());
        if son {
            q.extend(samples.iter().copied());
        } else {
            q.extend(std::iter::repeat_n(0.0, samples.len()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cible_50_ms() {
        assert_eq!(cible(44_100), 2_205);
        assert_eq!(cible(48_000), 2_400);
    }
}
