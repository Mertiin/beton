//! Sound loading and playback: register on load, decode on first play, unload after a pause.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type FileBytes = Arc<HashMap<PathBuf, Vec<u8>>>;

use rodio::buffer::SamplesBuffer;
use rodio::source::Source;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::core::asset::{AssetLifetime, ResolveAction};

/// Generational handle to a registered sound. Stale after the store is cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundId(pub(crate) u32, pub(crate) u32);

impl SoundId {
    /// Build an id from a raw index (generation 0). Prefer ids returned by [`Audio`].
    pub const fn from_raw(id: u32) -> Self {
        Self(id, 0)
    }

    fn index(self) -> u32 {
        self.0
    }

    fn generation(self) -> u32 {
        self.1
    }
}

/// Keeps a sound resident while alive, even when nothing is playing it.
#[derive(Clone, Debug)]
pub struct SoundHandle {
    id: SoundId,
    _pin: Arc<()>,
}

impl SoundHandle {
    /// Id to pass to [`Audio::play`] / [`Audio::play_loop`].
    pub fn id(&self) -> SoundId {
        self.id
    }
}

/// Handle to a playing voice. Becomes invalid after [`Audio::stop`] or when a
/// one-shot finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VoiceId {
    index: u32,
    generation: u32,
}

impl VoiceId {
    const INVALID: Self = Self {
        index: u32::MAX,
        generation: 0,
    };
}

struct Clip {
    channels: u16,
    sample_rate: u32,
    samples: Vec<f32>,
}

enum SoundSource {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

struct SoundSlot {
    source: SoundSource,
    generation: u32,
    life: AssetLifetime,
    resident: Option<Clip>,
}

struct VoiceSlot {
    generation: u32,
    looping: bool,
    player: Option<Player>,
    sound: Option<SoundId>,
    /// Set when `play` ran before a device existed; applied in [`Audio::try_open_device`].
    pending_volume: Option<f32>,
}

/// Loads clips and plays them through the default output device.
///
/// Field order matters: `voices` must drop before `sink` so players detach
/// while the mixer is still alive.
pub struct Audio {
    /// Kept first so it is dropped last (reverse field drop order).
    sink: Option<MixerDeviceSink>,
    slots: Vec<SoundSlot>,
    by_path: HashMap<PathBuf, SoundId>,
    voices: Vec<VoiceSlot>,
    files: Option<FileBytes>,
}

impl Audio {
    pub(crate) fn new() -> Self {
        let mut audio = Self::silent();
        let _ = audio.try_open_device();
        audio
    }

    pub(crate) fn silent() -> Self {
        Self {
            sink: None,
            slots: Vec::new(),
            by_path: HashMap::new(),
            voices: Vec::new(),
            files: None,
        }
    }

    /// Attach bytes preloaded by the app (from [`Game::assets`](crate::Game::assets)).
    pub(crate) fn install_files(&mut self, files: FileBytes) {
        self.files = Some(files);
    }

    /// True when an output device is open.
    pub(crate) fn has_device(&self) -> bool {
        self.sink.is_some()
    }

    /// Open the default output device without dropping registered clips or voices.
    ///
    /// Used on the web after a user gesture. Starts any plays that were queued while silent.
    pub(crate) fn try_open_device(&mut self) -> bool {
        if self.sink.is_some() {
            return false;
        }
        match DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                self.sink = Some(sink);
                self.attach_pending_voices();
                true
            }
            Err(err) => {
                crate::core::platform::warn(&format!(
                    "audio: no output device ({err}); sound disabled"
                ));
                false
            }
        }
    }

    /// Register a WAV or Ogg Vorbis path. Checks that the file exists; does not decode yet.
    ///
    /// Uses preloaded [`Game::assets`](crate::Game::assets) bytes when present (required on
    /// the web). Otherwise reads from the filesystem.
    pub fn load(&mut self, path: &Path) -> Result<SoundId, String> {
        let path = path.to_path_buf();
        if let Some(&id) = self.by_path.get(&path) {
            return Ok(id);
        }
        if let Some(bytes) = self.files.as_ref().and_then(|f| f.get(&path).cloned()) {
            let id = self.create(&bytes)?;
            self.by_path.insert(path, id);
            return Ok(id);
        }
        std::fs::metadata(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let id = self.alloc_slot(SoundSource::Path(path.clone()));
        self.by_path.insert(path, id);
        Ok(id)
    }

    /// Register WAV or Ogg Vorbis bytes. Validates by decoding once; PCM is not kept until play.
    pub fn create(&mut self, bytes: &[u8]) -> Result<SoundId, String> {
        let _ = decode_bytes(bytes)?;
        Ok(self.alloc_slot(SoundSource::Bytes(bytes.to_vec())))
    }

    /// Pin a sound so it stays decoded until the handle is dropped.
    pub fn pin(&self, id: SoundId) -> Option<SoundHandle> {
        let slot = self.slots.get(id.index() as usize)?;
        if slot.generation != id.generation() {
            return None;
        }
        Some(SoundHandle {
            id,
            _pin: slot.life.pin().into_arc(),
        })
    }

    /// Decode sounds that are referenced and unload ones unused past the grace period.
    pub(crate) fn resolve(&mut self, dt: f32) {
        let mut used = HashSet::new();
        for voice in &self.voices {
            // Include pending (no device yet) so boot-time loops stay decoded.
            if let Some(id) = voice.sound {
                used.insert(id);
            }
        }
        for index in 0..self.slots.len() {
            let id = SoundId(index as u32, self.slots[index].generation);
            let referenced = used.contains(&id);
            let action = self.slots[index].life.resolve(referenced, dt);
            match action {
                ResolveAction::Keep => {}
                ResolveAction::Load => {
                    if let Err(err) = self.decode_slot(index) {
                        crate::core::platform::warn(&format!(
                            "audio: failed to load #{}: {err}",
                            id.index()
                        ));
                        self.slots[index].life.mark_unloaded();
                    }
                }
                ResolveAction::Unload => {
                    self.slots[index].resident = None;
                    self.slots[index].life.mark_unloaded();
                }
            }
        }
    }

    /// Number of clips currently decoded in memory.
    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|s| s.resident.is_some()).count()
    }

    /// Approximate RAM for decoded clips (`samples.len() * size_of::<f32>()` each).
    pub fn resident_bytes(&self) -> usize {
        self.slots
            .iter()
            .filter_map(|s| s.resident.as_ref())
            .map(|c| c.samples.len().saturating_mul(std::mem::size_of::<f32>()))
            .sum()
    }

    /// CPU bytes kept so an unloaded source can decode again (file bytes or path only).
    pub fn retained_source_bytes(&self) -> usize {
        self.slots
            .iter()
            .map(|s| match &s.source {
                SoundSource::Path(_) => 0,
                SoundSource::Bytes(bytes) => bytes.len(),
            })
            .sum()
    }

    /// Play a clip once. Loads the clip if needed. Returns an invalid voice when
    /// there is no device or the id is unknown.
    pub fn play(&mut self, id: SoundId, volume: f32) -> VoiceId {
        self.start(id, volume, false)
    }

    /// Play a clip in a loop until [`Audio::stop`].
    pub fn play_loop(&mut self, id: SoundId, volume: f32) -> VoiceId {
        self.start(id, volume, true)
    }

    /// Set volume for a playing voice. `1.0` is unity. No-op for unknown voices.
    pub fn set_volume(&mut self, voice: VoiceId, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        let Some(slot) = self.voices.get_mut(voice.index as usize) else {
            return;
        };
        if slot.generation != voice.generation {
            return;
        }
        if let Some(player) = slot.player.as_ref() {
            player.set_volume(volume);
        } else if slot.pending_volume.is_some() {
            slot.pending_volume = Some(volume);
        }
    }

    /// Stop a voice. No-op for unknown or already finished voices.
    pub fn stop(&mut self, voice: VoiceId) {
        let Some(slot) = self.voices.get_mut(voice.index as usize) else {
            return;
        };
        if slot.generation != voice.generation {
            return;
        }
        if let Some(player) = slot.player.take() {
            player.stop();
        }
        slot.sound = None;
        slot.pending_volume = None;
        slot.generation = slot.generation.wrapping_add(1);
    }

    /// Drop finished one-shot players so repeated `play` calls do not leak.
    pub(crate) fn reap_finished(&mut self) {
        for slot in &mut self.voices {
            let done = slot
                .player
                .as_ref()
                .is_some_and(|player| !slot.looping && player.empty());
            if done {
                slot.player = None;
                slot.sound = None;
                slot.pending_volume = None;
                slot.generation = slot.generation.wrapping_add(1);
            }
        }
    }

    fn start(&mut self, id: SoundId, volume: f32, looping: bool) -> VoiceId {
        let volume = volume.clamp(0.0, 1.0);
        if self.ensure_resident(id).is_err() {
            return VoiceId::INVALID;
        }
        if self.sink.is_none() {
            return self.alloc_pending_voice(id, volume, looping);
        }
        match self.connect_player(id, volume, looping) {
            Some(player) => self.alloc_voice(player, looping, id),
            None => VoiceId::INVALID,
        }
    }

    fn connect_player(&self, id: SoundId, volume: f32, looping: bool) -> Option<Player> {
        let clip = self.resident_clip(id)?;
        let channels = NonZero::new(clip.channels)?;
        let sample_rate = NonZero::new(clip.sample_rate)?;
        let samples = clip.samples.clone();
        let sink = self.sink.as_ref()?;
        let buffer = SamplesBuffer::new(channels, sample_rate, samples);
        let player = Player::connect_new(sink.mixer());
        if looping {
            player.append(buffer.repeat_infinite());
        } else {
            player.append(buffer);
        }
        player.set_volume(volume);
        Some(player)
    }

    fn attach_pending_voices(&mut self) {
        let pending: Vec<(usize, SoundId, f32, bool)> = self
            .voices
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let id = slot.sound?;
                let volume = slot.pending_volume?;
                if slot.player.is_some() {
                    return None;
                }
                Some((index, id, volume, slot.looping))
            })
            .collect();
        for (index, id, volume, looping) in pending {
            if self.ensure_resident(id).is_err() {
                continue;
            }
            let Some(player) = self.connect_player(id, volume, looping) else {
                continue;
            };
            let slot = &mut self.voices[index];
            slot.player = Some(player);
            slot.pending_volume = None;
        }
    }

    fn ensure_resident(&mut self, id: SoundId) -> Result<(), String> {
        let slot = self
            .slots
            .get_mut(id.index() as usize)
            .ok_or_else(|| "unknown sound".to_string())?;
        if slot.generation != id.generation() {
            return Err("stale sound id".into());
        }
        if slot.resident.is_some() {
            slot.life.mark_resident();
            return Ok(());
        }
        self.decode_slot(id.index() as usize)
    }

    fn resident_clip(&self, id: SoundId) -> Option<&Clip> {
        let slot = self.slots.get(id.index() as usize)?;
        if slot.generation != id.generation() {
            return None;
        }
        slot.resident.as_ref()
    }

    fn alloc_slot(&mut self, source: SoundSource) -> SoundId {
        let index = self.slots.len() as u32;
        self.slots.push(SoundSlot {
            source,
            generation: 0,
            life: AssetLifetime::new(),
            resident: None,
        });
        SoundId(index, 0)
    }

    fn decode_slot(&mut self, index: usize) -> Result<(), String> {
        let bytes = match &self.slots[index].source {
            SoundSource::Path(path) => std::fs::read(path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?,
            SoundSource::Bytes(bytes) => bytes.clone(),
        };
        let clip = decode_bytes(&bytes)?;
        self.slots[index].resident = Some(clip);
        self.slots[index].life.mark_resident();
        Ok(())
    }

    fn free_voice_slot(&mut self) -> (usize, u32) {
        if let Some((index, slot)) = self
            .voices
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.player.is_none() && slot.sound.is_none())
        {
            let generation = slot.generation;
            return (index, generation);
        }
        let index = self.voices.len();
        self.voices.push(VoiceSlot {
            generation: 0,
            looping: false,
            player: None,
            sound: None,
            pending_volume: None,
        });
        (index, 0)
    }

    fn alloc_voice(&mut self, player: Player, looping: bool, sound: SoundId) -> VoiceId {
        let (index, generation) = self.free_voice_slot();
        let slot = &mut self.voices[index];
        slot.player = Some(player);
        slot.looping = looping;
        slot.sound = Some(sound);
        slot.pending_volume = None;
        VoiceId {
            index: index as u32,
            generation,
        }
    }

    fn alloc_pending_voice(&mut self, sound: SoundId, volume: f32, looping: bool) -> VoiceId {
        let (index, generation) = self.free_voice_slot();
        let slot = &mut self.voices[index];
        slot.player = None;
        slot.looping = looping;
        slot.sound = Some(sound);
        slot.pending_volume = Some(volume);
        VoiceId {
            index: index as u32,
            generation,
        }
    }

}

fn decode_bytes(bytes: &[u8]) -> Result<Clip, String> {
    let decoder = Decoder::try_from(Cursor::new(bytes.to_vec()))
        .map_err(|err| format!("failed to decode audio: {err}"))?;
    let channels = decoder.channels().get();
    let sample_rate = decoder.sample_rate().get();
    let samples: Vec<f32> = decoder.collect();
    if samples.is_empty() {
        return Err("audio contains no samples".into());
    }
    Ok(Clip {
        channels,
        sample_rate,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn silent_wav() -> Vec<u8> {
        let sample_rate = 8_000u32;
        let samples = [0i16; 80];
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + samples.len() * 2);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    #[test]
    fn create_registers_without_resident_clip() {
        let mut audio = Audio::silent();
        let id = audio.create(&silent_wav()).expect("decode wav");
        assert_eq!(id, SoundId::from_raw(0));
        assert_eq!(audio.live_count(), 0);
        audio.ensure_resident(id).expect("load");
        assert_eq!(audio.live_count(), 1);
        let clip = audio.resident_clip(id).unwrap();
        assert_eq!(clip.channels, 1);
        assert_eq!(clip.sample_rate, 8_000);
        assert!(!clip.samples.is_empty());
    }

    #[test]
    fn rejects_garbage_bytes() {
        let mut audio = Audio::silent();
        let err = audio.create(b"not audio").expect_err("garbage");
        assert!(err.contains("failed to decode"), "{err}");
    }

    #[test]
    fn play_controls_do_not_panic_without_device() {
        let mut audio = Audio::silent();
        let id = audio.create(&silent_wav()).expect("decode wav");
        let voice = audio.play(id, 0.5);
        audio.set_volume(voice, 0.25);
        audio.stop(voice);
        let looped = audio.play_loop(id, 0.1);
        audio.stop(looped);
        audio.reap_finished();
    }

    #[test]
    fn play_without_device_keeps_clip_ids_and_pending_loop() {
        let mut audio = Audio::silent();
        let id = audio.create(&silent_wav()).expect("decode wav");
        let pin = audio.pin(id).unwrap();
        let looped = audio.play_loop(id, 0.2);
        assert_ne!(looped, VoiceId::INVALID);
        assert!(audio.voices[looped.index as usize].pending_volume.is_some());
        assert_eq!(audio.voices[looped.index as usize].sound, Some(id));
        // Opening may fail in headless CI; either way clips must survive.
        let _ = audio.try_open_device();
        assert_eq!(pin.id(), id);
        assert_eq!(audio.slots.len(), 1);
        audio.stop(looped);
    }

    #[test]
    fn play_controls_do_not_panic_with_default_device() {
        let mut audio = Audio::new();
        let id = audio.create(&silent_wav()).expect("decode wav");
        let voice = audio.play(id, 0.05);
        audio.set_volume(voice, 0.02);
        audio.stop(voice);
        let looped = audio.play_loop(id, 0.02);
        audio.stop(looped);
        audio.reap_finished();
    }

    #[test]
    fn pin_keeps_clip_through_resolve() {
        let mut audio = Audio::silent();
        let id = audio.create(&silent_wav()).unwrap();
        let pin = audio.pin(id).unwrap();
        audio.resolve(0.0);
        assert_eq!(audio.live_count(), 1);
        drop(pin);
        audio.resolve(2.0);
        assert_eq!(audio.live_count(), 0);
    }
}
