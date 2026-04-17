use std::collections::HashMap;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Animation manager - tracks active animations with timing and easing
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct AnimationState {
    #[allow(dead_code)]
    pub start_time: Instant,
    #[allow(dead_code)]
    pub start_value: f32,
    #[allow(dead_code)]
    pub end_value: f32,
    #[allow(dead_code)]
    pub duration_ms: u64,
}

pub struct AnimationManager {
    active: HashMap<String, AnimationState>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    /// Start a new animation or replace existing one
    pub fn start(&mut self, name: impl Into<String>, start: f32, end: f32, duration_ms: u64) {
        self.active.insert(name.into(), AnimationState {
            start_time: Instant::now(),
            start_value: start,
            end_value: end,
            duration_ms,
        });
    }

    /// Get current interpolated value of an animation, None if not active or complete
    #[allow(dead_code)]
    pub fn get(&mut self, name: &str) -> Option<f32> {
        if let Some(state) = self.active.get(name) {
            let elapsed_ms = state.start_time.elapsed().as_millis() as u64;
            if elapsed_ms >= state.duration_ms {
                self.active.remove(name);
                None
            } else {
                let progress = (elapsed_ms as f32) / (state.duration_ms as f32);
                Some(state.start_value + (state.end_value - state.start_value) * progress)
            }
        } else {
            None
        }
    }

    /// Check if animation is active
    #[allow(dead_code)]
    pub fn is_active(&self, name: &str) -> bool {
        self.active.contains_key(name)
    }

    /// Stop all animations
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.active.clear();
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}
