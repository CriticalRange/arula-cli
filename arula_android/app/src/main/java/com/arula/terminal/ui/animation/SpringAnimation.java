package com.arula.terminal.ui.animation;

/**
 * Spring physics animation system for smooth, natural transitions
 * Exact port from desktop version's spring.rs
 */
public class SpringAnimation {
    // Spring physics defaults (matching desktop constants.rs)
    public static final float DEFAULT_STIFFNESS = 0.03f;
    public static final float DEFAULT_DAMPING = 0.80f;
    public static final float SPRING_THRESHOLD = 0.001f;

    public float position;
    public float velocity;
    public float target;
    public float stiffness;
    public float damping;

    private SpringListener listener;

    public interface SpringListener {
        void onAnimationUpdate(float position, float velocity);

        void onAnimationComplete();
    }

    public SpringAnimation() {
        this(DEFAULT_STIFFNESS, DEFAULT_DAMPING);
    }

    public SpringAnimation(float stiffness, float damping) {
        this.position = 0.0f;
        this.velocity = 0.0f;
        this.target = 0.0f;
        this.stiffness = stiffness;
        this.damping = damping;
    }

    /**
     * Updates the spring physics. Returns true if still animating.
     * Exact match to desktop spring.rs update() method
     */
    public boolean update() {
        // Spring force: F = (target - position) * stiffness
        float force = (target - position) * stiffness;

        // Update velocity with force and apply damping
        velocity = (velocity + force) * damping;

        // Update position
        position += velocity;

        // Clamp position to valid range to prevent oscillation overshoot
        position = Math.max(0.0f, Math.min(1.0f, position));

        // If very close to target and velocity is low, snap to target
        float distance = Math.abs(target - position);
        if (distance < SPRING_THRESHOLD && Math.abs(velocity) < SPRING_THRESHOLD) {
            position = target;
            velocity = 0.0f;
            if (listener != null) {
                listener.onAnimationComplete();
            }
            return false;
        }

        // Notify listener of update
        if (listener != null) {
            listener.onAnimationUpdate(position, velocity);
        }

        return Math.abs(velocity) > SPRING_THRESHOLD || distance > SPRING_THRESHOLD;
    }

    /**
     * Sets the target value for the spring to animate towards.
     */
    public void setTarget(float target) {
        this.target = target;
    }

    /**
     * Instantly sets position without animation
     */
    public void setPosition(float position) {
        this.position = Math.max(0.0f, Math.min(1.0f, position));
        this.velocity = 0.0f;
    }

    /**
     * Returns true if the spring is open (target > 0.5).
     */
    public boolean isOpen() {
        return target > 0.5f;
    }

    // Getters
    public float getPosition() {
        return position;
    }

    public float getVelocity() {
        return velocity;
    }

    public float getTarget() {
        return target;
    }

    public void setListener(SpringListener listener) {
        this.listener = listener;
    }
}