use super::*;

/// A mono, 16-bit, 44.1 kHz WAV file holding `samples`.
fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&44100u32.to_le_bytes());
    bytes.extend_from_slice(&(44100u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn mixer() -> (Mixer, MixerControl) {
    let (builder, control) = Mixer::new();
    (builder.build(), control)
}

/// Lets the mixer take in every message sent so far.
fn receive(mixer: &mut Mixer) {
    mixer.fill_audio_buffer(&mut [0.0; 2], 1);
}

fn hash_of(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn two_loads_of_one_file_share_its_samples() {
    let (mut mixer, control) = mixer();
    let clip = wav(&[1000, -1000, 2000, -2000]);
    let first = control.load(&clip);
    let second = control.load(&clip.clone());
    receive(&mut mixer);
    assert_ne!(first, second);
    assert!(Arc::ptr_eq(&mixer.sounds[&first], &mixer.sounds[&second]));
}

#[test]
fn different_files_keep_their_own_samples() {
    let (mut mixer, control) = mixer();
    let first = control.load(&wav(&[1000, -1000]));
    let second = control.load(&wav(&[3000, -3000]));
    receive(&mut mixer);
    assert!(!Arc::ptr_eq(&mixer.sounds[&first], &mixer.sounds[&second]));
    assert_ne!(mixer.sounds[&first], mixer.sounds[&second]);
}

#[test]
fn a_hash_match_on_other_bytes_decodes_its_own_samples() {
    let (mut mixer, control) = mixer();
    let clip = wav(&[1000, -1000]);
    let other = wav(&[3000, -3000]);
    control.decoded.borrow_mut().insert(
        hash_of(&clip),
        vec![Decoded {
            bytes: other.clone().into(),
            samples: load_samples_from_file(&other).unwrap().into(),
            sounds: vec![u32::MAX],
        }],
    );
    let sound = control.load(&clip);
    receive(&mut mixer);
    assert_eq!(
        *mixer.sounds[&sound],
        *load_samples_from_file(&clip).unwrap()
    );
}

#[test]
fn shared_samples_still_stop_by_sound_id() {
    let (mut mixer, control) = mixer();
    let clip = wav(&[1000, -1000, 2000, -2000]);
    let first = control.load(&clip);
    let second = control.load(&clip);
    let looped = || PlaySoundParams {
        looped: true,
        volume: 1.0,
    };
    control.play(first, looped());
    control.play(second, looped());
    control.stop_all(first);
    receive(&mut mixer);
    let playing: Vec<u32> = mixer.mixer_state.iter().map(|s| s.sound_id).collect();
    assert_eq!(playing, vec![second]);
}

#[test]
fn deleting_every_sound_of_a_file_frees_its_samples() {
    let (mut mixer, control) = mixer();
    let clip = wav(&[1000, -1000, 2000, -2000]);
    let first = control.load(&clip);
    let second = control.load(&clip.clone());
    receive(&mut mixer);
    let samples = Arc::downgrade(&mixer.sounds[&first]);
    control.delete(first);
    let third = control.load(&clip.clone());
    receive(&mut mixer);
    assert!(Arc::ptr_eq(&mixer.sounds[&second], &mixer.sounds[&third]));
    control.delete(second);
    control.delete(third);
    receive(&mut mixer);
    assert!(samples.upgrade().is_none());
    assert!(control.decoded.borrow().is_empty());
}

#[test]
fn voices_of_a_shared_file_mix_as_separate_loads_did() {
    let clip = wav(&[1000, -1000, 2000, -2000, 3000, -3000, 4000, -4000]);
    let (mut shared, sharing) = mixer();
    let (mut apart, separate) = mixer();
    let ids = [sharing.load(&clip), sharing.load(&clip)];
    for &id in &ids {
        let samples: Arc<[f32]> = load_samples_from_file(&clip).unwrap().into();
        separate.send(AudioMessage::AddSound(id, samples));
    }
    let mixed = |mixer: &mut Mixer, frames: usize| {
        let mut buffer = vec![0.0; frames * 2];
        mixer.fill_audio_buffer(&mut buffer, frames);
        buffer
    };
    for control in [&sharing, &separate] {
        control.play(
            ids[0],
            PlaySoundParams {
                looped: true,
                volume: 1.0,
            },
        );
    }
    let alone = mixed(&mut shared, 3);
    assert!(alone.iter().any(|&sample| sample != 0.0));
    assert_eq!(alone, mixed(&mut apart, 3));
    for control in [&sharing, &separate] {
        control.play(
            ids[1],
            PlaySoundParams {
                looped: false,
                volume: 0.5,
            },
        );
    }
    assert_eq!(mixed(&mut shared, 13), mixed(&mut apart, 13));
}
