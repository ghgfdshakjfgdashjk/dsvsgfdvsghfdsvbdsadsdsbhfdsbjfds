use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::engine::Engine;
use crate::settings::{Profile, Settings};

pub struct Clickers {
    engines: RwLock<Vec<Arc<Engine>>>,

    roster: AtomicU64,

    closing: AtomicBool,
}

impl Clickers {
    pub fn new(profiles: &[Profile]) -> Self {
        let engines = profiles
            .iter()
            .map(|p| Arc::new(Engine::new(p.clone())))
            .collect();
        Clickers {
            engines: RwLock::new(engines),
            roster: AtomicU64::new(0),
            closing: AtomicBool::new(false),
        }
    }

    pub fn sync(&self, profiles: &[Profile]) {
        let mut engines = self.engines.write().unwrap();
        let before = engines.len();

        while engines.len() > profiles.len() {
            if let Some(engine) = engines.pop() {
                engine.shutdown();
            }
        }

        for (index, profile) in profiles.iter().enumerate() {
            match engines.get(index) {
                Some(engine) => {
                    engine.update_settings(profile.clone());

                    if !profile.enabled {
                        engine.set_active(false);
                    }
                }
                None => engines.push(Arc::new(Engine::new(profile.clone()))),
            }
        }

        if engines.len() != before {
            self.roster.fetch_add(1, Ordering::Release);
        }
    }

    pub fn roster(&self) -> u64 {
        self.roster.load(Ordering::Acquire)
    }

    pub fn snapshot(&self) -> Vec<Arc<Engine>> {
        self.engines.read().unwrap().clone()
    }

    pub fn get(&self, index: usize) -> Option<Arc<Engine>> {
        self.engines.read().unwrap().get(index).cloned()
    }

    pub fn stop_all(&self) {
        for engine in self.snapshot() {
            engine.set_active(false);
        }
    }

    pub fn shutdown(&self) {
        self.closing.store(true, Ordering::Release);
        for engine in self.snapshot() {
            engine.shutdown();
        }
    }

    pub fn is_closing(&self) -> bool {
        self.closing.load(Ordering::Acquire)
    }
}

pub struct Shared {
    pub panic_vk: AtomicU32,

    pub guard_px: AtomicU32,
    pub guard_screen: AtomicBool,
    pub guard_chrome: AtomicBool,
}

impl Shared {
    pub fn new(settings: &Settings) -> Self {
        let shared = Shared {
            panic_vk: AtomicU32::new(0),
            guard_px: AtomicU32::new(0),
            guard_screen: AtomicBool::new(false),
            guard_chrome: AtomicBool::new(true),
        };
        shared.update(settings);
        shared
    }

    pub fn update(&self, settings: &Settings) {
        self.panic_vk.store(settings.panic_vk, Ordering::Relaxed);
        self.guard_px.store(
            if settings.edge_guard_enabled {
                settings.edge_guard_px.round().clamp(1.0, 200.0) as u32
            } else {
                0
            },
            Ordering::Relaxed,
        );
        self.guard_screen
            .store(settings.edge_guard_mode == "screen", Ordering::Relaxed);
        self.guard_chrome
            .store(settings.edge_guard_chrome, Ordering::Relaxed);
    }
}
