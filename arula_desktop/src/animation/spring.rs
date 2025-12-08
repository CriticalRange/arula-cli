use crate::constants::{SPRING_STIFFNESS, SPRING_DAMPING, SPRING_THRESHOLD};

/// A spring-based animation value for smooth transitions.
#[derive(Debug, Clone, Copy)]
pub struct Spring {
    pub position: f32,
    pub velocity: f32,
    pub target: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            position: 0.0,
            velocity: 0.0,
            target: 0.0,
            stiffness: SPRING_STIFFNESS,
            damping: SPRING_DAMPING,
        }
    }
}

impl Spring {
    /// Creates a new spring with custom parameters.
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self {
            stiffness,
            damping,
            ..Default::default()
        }
    }

    /// Updates the spring physics. Returns true if still animating.
    pub fn update(&mut self) -> bool {
        let force = (self.target - self.position) * self.stiffness;
        self.velocity = (self.velocity + force) * self.damping;
        self.position += self.velocity;
        
        // Clamp position to valid range to prevent oscillation overshoot
        self.position = self.position.clamp(0.0, 1.0);
        
        // If very close to target and velocity is low, snap to target
        let distance = (self.target - self.position).abs();
        if distance < SPRING_THRESHOLD && self.velocity.abs() < SPRING_THRESHOLD {
            self.position = self.target;
            self.velocity = 0.0;
            return false;
        }
        
        self.velocity.abs() > SPRING_THRESHOLD || distance > SPRING_THRESHOLD
    }

    /// Sets the target value for the spring to animate towards.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Returns true if the spring is open (target > 0.5).
    pub fn is_open(&self) -> bool {
        self.target > 0.5
    }
}
