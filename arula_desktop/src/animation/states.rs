use iced::widget::canvas;
use iced::Point;
use super::Spring;
use crate::constants::{TICK_INCREMENT, PAGE_TRANSITION_STIFFNESS, PAGE_TRANSITION_DAMPING};

/// Page enum for settings submenu navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsPage {
    #[default]
    Main,          // Category selection
    Provider,      // Provider + Model
    Api,           // API Key + URL (legacy - redirects to Provider)
    Behavior,      // System prompt, temp, tokens, toggles
    Appearance,    // Living background, etc.
    ModelSelector, // Model list selector
}

impl SettingsPage {
    /// Returns the title for the page.
    pub fn title(&self) -> &'static str {
        match self {
            SettingsPage::Main => "Settings",
            SettingsPage::Provider => "Provider & Model",
            SettingsPage::Api => "API Configuration",
            SettingsPage::Behavior => "Behavior",
            SettingsPage::Appearance => "Appearance",
            SettingsPage::ModelSelector => "Select Model",
        }
    }

    /// Returns the subtitle/description for the page.
    pub fn subtitle(&self) -> &'static str {
        match self {
            SettingsPage::Main => "Configure your AI",
            SettingsPage::Provider => "Select AI provider and model",
            SettingsPage::Api => "Configure API credentials",
            SettingsPage::Behavior => "Adjust AI behavior settings",
            SettingsPage::Appearance => "Customize visual settings",
            SettingsPage::ModelSelector => "Choose a model",
        }
    }
}

/// Direction of page transition animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionDirection {
    #[default]
    None,
    Forward,  // Navigating to submenu (slide from right)
    Backward, // Navigating back (slide from left)
}

/// State for settings submenu navigation with transitions.
#[derive(Debug)]
pub struct SettingsMenuState {
    pub current_page: SettingsPage,
    pub previous_page: Option<SettingsPage>,
    pub transition: Spring,
    pub direction: TransitionDirection,
}

impl Default for SettingsMenuState {
    fn default() -> Self {
        Self {
            current_page: SettingsPage::Main,
            previous_page: None,
            transition: Spring::new(PAGE_TRANSITION_STIFFNESS, PAGE_TRANSITION_DAMPING),
            direction: TransitionDirection::None,
        }
    }
}

impl SettingsMenuState {
    /// Navigate to a new page with forward transition.
    pub fn navigate_to(&mut self, page: SettingsPage) {
        if page != self.current_page {
            self.previous_page = Some(self.current_page);
            self.current_page = page;
            self.direction = TransitionDirection::Forward;
            self.transition.position = 0.0;
            self.transition.velocity = 0.0;
            self.transition.set_target(1.0);
        }
    }

    /// Navigate back to main page with backward transition.
    pub fn navigate_back(&mut self) {
        if self.current_page != SettingsPage::Main {
            self.previous_page = Some(self.current_page);
            self.current_page = SettingsPage::Main;
            self.direction = TransitionDirection::Backward;
            self.transition.position = 0.0;
            self.transition.velocity = 0.0;
            self.transition.set_target(1.0);
        }
    }

    /// Reset to main page (when menu closes).
    pub fn reset(&mut self) {
        self.current_page = SettingsPage::Main;
        self.previous_page = None;
        self.transition.position = 1.0;
        self.transition.velocity = 0.0;
        self.direction = TransitionDirection::None;
    }

    /// Update transition animation. Returns true if still animating.
    pub fn update(&mut self) -> bool {
        let animating = self.transition.update();
        if !animating {
            // Transition complete
            self.previous_page = None;
            self.direction = TransitionDirection::None;
        }
        animating
    }

    /// Returns the current transition progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        self.transition.position
    }

    /// Returns true if currently transitioning between pages.
    pub fn is_transitioning(&self) -> bool {
        self.direction != TransitionDirection::None && self.transition.position < 0.99
    }
}

/// State for the living background animation.
/// State for the living background animation.
#[derive(Debug)]
pub struct LivingBackgroundState {
    pub tick: f32,
    pub sway_angle: f32,
    pub travel: f32,
    pub cache: canvas::Cache,
}

impl Default for LivingBackgroundState {
    fn default() -> Self {
        Self {
            tick: 0.0,
            sway_angle: 0.0,
            travel: 0.0,
            cache: canvas::Cache::default(),
        }
    }
}

impl LivingBackgroundState {
    /// Updates the background animation state.
    pub fn update(&mut self) {
        self.tick += TICK_INCREMENT;
        
        // Gentle sway based on sine wave
        // Provides a floating sensation
        self.sway_angle = (self.tick * 0.5).sin() * 0.05; 
        
        // Move forward through 3D space
        self.travel += 0.8; // Faster travel for exploration feel
        
        self.cache.clear();
    }
}

/// State for the liquid menu overlay animation.
#[derive(Debug, Default)]
pub struct LiquidMenuState {
    pub spring: Spring,
    pub cache: canvas::Cache,
}

impl LiquidMenuState {
    /// Updates the menu animation state. Returns true if still animating.
    pub fn update(&mut self) -> bool {
        let animating = self.spring.update();
        if animating {
            self.cache.clear();
        }
        animating
    }

    /// Opens the menu.
    pub fn open(&mut self) {
        self.spring.set_target(1.0);
    }

    /// Closes the menu.
    pub fn close(&mut self) {
        self.spring.set_target(0.0);
    }

    /// Returns true if the menu is open or opening.
    pub fn is_open(&self) -> bool {
        self.spring.is_open()
    }

    /// Returns the current animation progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        self.spring.position
    }
}

/// State for a tilt-responsive card.
#[derive(Debug, Default)]
pub struct TiltCardState {
    pub mouse_position: Point,
    pub is_hovered: bool,
    pub hover_tick: f32,
    pub cache: canvas::Cache,
}

impl TiltCardState {
    /// Updates the card hover animation.
    pub fn update(&mut self) -> bool {
        if self.is_hovered {
            self.hover_tick += crate::constants::HOVER_TICK_INCREMENT;
            true
        } else {
            false
        }
    }

    /// Sets the hover state.
    pub fn set_hovered(&mut self, hovered: bool) {
        self.is_hovered = hovered;
        if !hovered {
            self.hover_tick = 0.0;
        }
    }

    /// Sets the mouse position for tilt calculation.
    pub fn set_mouse_position(&mut self, position: Point) {
        self.mouse_position = position;
    }

    /// Clears the canvas cache for redraw.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}
