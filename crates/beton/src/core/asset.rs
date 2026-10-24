//! Shared lifetime rules for textures and sounds: load on use, unload after a pause.

use std::sync::Arc;

/// Seconds an unused resident asset stays in memory before unload.
pub(crate) const UNUSED_GRACE_SECS: f32 = 2.0;

/// CPU-side state machine for one asset slot (no GPU / PCM payload).
///
/// Used by unit tests and mirrored by the texture and audio stores.
#[derive(Debug)]
pub(crate) struct AssetLifetime {
    /// Shared with every outstanding [`Pin`] for this slot. Count `> 1` means pinned.
    pin: Arc<()>,
    resident: bool,
    unused_for: f32,
}

/// Owning pin that keeps an asset referenced without drawing or playing it.
#[derive(Clone, Debug)]
pub(crate) struct Pin {
    inner: Arc<()>,
}

impl AssetLifetime {
    pub(crate) fn new() -> Self {
        Self {
            pin: Arc::new(()),
            resident: false,
            unused_for: 0.0,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn is_resident(&self) -> bool {
        self.resident
    }

    pub(crate) fn is_pinned(&self) -> bool {
        Arc::strong_count(&self.pin) > 1
    }

    pub(crate) fn pin(&self) -> Pin {
        Pin {
            inner: Arc::clone(&self.pin),
        }
    }

    /// Apply one resolve step. Returns whether the payload should load or unload.
    pub(crate) fn resolve(&mut self, referenced: bool, dt: f32) -> ResolveAction {
        let referenced = referenced || self.is_pinned();
        if referenced {
            self.unused_for = 0.0;
            if !self.resident {
                self.resident = true;
                return ResolveAction::Load;
            }
            return ResolveAction::Keep;
        }
        if !self.resident {
            return ResolveAction::Keep;
        }
        self.unused_for += dt;
        if self.unused_for >= UNUSED_GRACE_SECS {
            self.resident = false;
            self.unused_for = 0.0;
            ResolveAction::Unload
        } else {
            ResolveAction::Keep
        }
    }

    /// Mark resident after a successful external load (tests / mirrors).
    pub(crate) fn mark_resident(&mut self) {
        self.resident = true;
        self.unused_for = 0.0;
    }

    /// Mark unloaded after dropping the payload.
    pub(crate) fn mark_unloaded(&mut self) {
        self.resident = false;
        self.unused_for = 0.0;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResolveAction {
    Keep,
    Load,
    Unload,
}

impl Pin {
    pub(crate) fn into_arc(self) -> Arc<()> {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_starts_non_resident() {
        let life = AssetLifetime::new();
        assert!(!life.is_resident());
        assert!(!life.is_pinned());
    }

    #[test]
    fn first_reference_loads() {
        let mut life = AssetLifetime::new();
        assert_eq!(life.resolve(true, 0.016), ResolveAction::Load);
        assert!(life.is_resident());
    }

    #[test]
    fn grace_keeps_then_unloads() {
        let mut life = AssetLifetime::new();
        assert_eq!(life.resolve(true, 0.0), ResolveAction::Load);
        assert_eq!(life.resolve(false, 1.0), ResolveAction::Keep);
        assert!(life.is_resident());
        assert_eq!(life.resolve(false, 1.5), ResolveAction::Unload);
        assert!(!life.is_resident());
    }

    #[test]
    fn reference_resets_timer() {
        let mut life = AssetLifetime::new();
        assert_eq!(life.resolve(true, 0.0), ResolveAction::Load);
        assert_eq!(life.resolve(false, 1.5), ResolveAction::Keep);
        assert_eq!(life.resolve(true, 0.016), ResolveAction::Keep);
        assert_eq!(life.resolve(false, 1.5), ResolveAction::Keep);
        assert!(life.is_resident());
    }

    #[test]
    fn pin_prevents_unload() {
        let mut life = AssetLifetime::new();
        let _pin = life.pin();
        assert_eq!(life.resolve(false, 0.0), ResolveAction::Load);
        assert_eq!(life.resolve(false, 5.0), ResolveAction::Keep);
        assert!(life.is_resident());
    }

    #[test]
    fn drop_pin_allows_grace_unload() {
        let mut life = AssetLifetime::new();
        let pin = life.pin();
        assert_eq!(life.resolve(false, 0.0), ResolveAction::Load);
        drop(pin);
        assert_eq!(life.resolve(false, 2.0), ResolveAction::Unload);
    }
}
