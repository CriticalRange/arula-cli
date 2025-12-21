package com.arula.terminal.ui.canvas;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PointF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;

import com.arula.terminal.R;

/**
 * Living background with 3D perspective grid effect
 * Exact port from desktop version's living_background.rs
 */
public class LivingBackground extends View {
    // Animation state
    private float tick = 0f;
    private float swayAngle = 0f;
    private float travel = 0f;
    private float opacity = 1f;
    private boolean isEnabled = true;

    // Colors from palette
    private int backgroundColor;
    private int accentColor;
    private int disabledBackgroundColor;

    // Paints
    private Paint backgroundPaint;
    private Paint gridPaint;

    // Animation
    private ValueAnimator animator;

    // 3D Projection Constants (matching desktop)
    private static final float FOV = 250f; // Field of view / focal length
    private static final float VISIBILITY = 2000f; // Max draw distance
    private static final float GRID_SPACING = 100f; // Grid line spacing
    private static final int NUM_LINES = 40; // Number of horizontal lines
    private static final float TICK_INCREMENT = 0.016f; // ~60fps
    private static final float TRAVEL_SPEED = 0.8f; // Forward motion speed
    private static final float SWAY_SPEED = 0.5f; // Sway oscillation speed
    private static final float SWAY_AMPLITUDE = 0.05f; // Sway angle amplitude

    public LivingBackground(Context context) {
        super(context);
        init();
    }

    public LivingBackground(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        init();
    }

    public LivingBackground(Context context, @Nullable AttributeSet attrs, int defStyleAttr) {
        super(context, attrs, defStyleAttr);
        init();
    }

    private void init() {
        // Get colors from resources
        backgroundColor = getContext().getColor(R.color.neon_background);
        accentColor = getContext().getColor(R.color.neon_accent);
        disabledBackgroundColor = Color.rgb(25, 25, 25); // Dark gray when disabled

        // Initialize paints
        backgroundPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        backgroundPaint.setColor(backgroundColor);
        backgroundPaint.setStyle(Paint.Style.FILL);

        gridPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        gridPaint.setStyle(Paint.Style.STROKE);
        gridPaint.setStrokeCap(Paint.Cap.ROUND);

        // Start animation loop
        startAnimation();
    }

    private void startAnimation() {
        animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(16); // ~60fps
        animator.setRepeatCount(ValueAnimator.INFINITE);

        animator.addUpdateListener(animation -> {
            if (isEnabled) {
                updateAnimation();
                invalidate();
            }
        });

        animator.start();
    }

    private void updateAnimation() {
        // Update tick (matches desktop TICK_INCREMENT)
        tick += TICK_INCREMENT;

        // Gentle sway based on sine wave (matches desktop)
        swayAngle = (float) Math.sin(tick * SWAY_SPEED) * SWAY_AMPLITUDE;

        // Move forward through 3D space
        travel += TRAVEL_SPEED;
    }

    /**
     * Project 3D point to 2D screen space
     * Matches desktop project() function exactly
     */
    private PointF project(float x, float y, float z, float centerX, float centerY, float rotation) {
        if (z <= 1.0f) {
            return null; // Behind camera
        }

        float scale = FOV / z;

        // Apply rotation
        float rx = (float) (x * Math.cos(rotation) - y * Math.sin(rotation));
        float ry = (float) (x * Math.sin(rotation) + y * Math.cos(rotation));

        return new PointF(
                centerX + rx * scale,
                centerY + ry * scale);
    }

    public void setOpacity(float opacity) {
        this.opacity = Math.max(0f, Math.min(1f, opacity));
    }

    public void setEnabled(boolean enabled) {
        this.isEnabled = enabled;
        if (enabled && animator != null && !animator.isRunning()) {
            animator.start();
        }
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);

        float width = getWidth();
        float height = getHeight();
        float centerX = width / 2f;
        float centerY = height / 2f;

        // Interpolate background color between active (purple) and disabled (gray)
        // Matches desktop: active_bg * opacity + disabled_bg * (1 - opacity)
        int bgR = (int) (Color.red(backgroundColor) * opacity + Color.red(disabledBackgroundColor) * (1f - opacity));
        int bgG = (int) (Color.green(backgroundColor) * opacity
                + Color.green(disabledBackgroundColor) * (1f - opacity));
        int bgB = (int) (Color.blue(backgroundColor) * opacity + Color.blue(disabledBackgroundColor) * (1f - opacity));

        backgroundPaint.setColor(Color.rgb(bgR, bgG, bgB));
        canvas.drawRect(0, 0, width, height, backgroundPaint);

        // Optimization: If opacity is ~0, don't draw grid
        if (opacity <= 0.01f) {
            return;
        }

        // Draw 3D perspective grid for floor and ceiling
        float[] yLevels = { -200f, 200f };

        for (float yLevel : yLevels) {
            // 1. Draw Longitudinal Lines (going into distance)
            for (int i = -15; i <= 15; i++) {
                float xPos = i * GRID_SPACING;
                float zStart = 10f;
                float zEnd = VISIBILITY;

                PointF p1 = project(xPos, yLevel, zStart, centerX, centerY, swayAngle);
                PointF p2 = project(xPos, yLevel, zEnd, centerX, centerY, swayAngle);

                if (p1 != null && p2 != null) {
                    // Set stroke style (matches desktop: 0.15 * opacity alpha)
                    gridPaint.setColor(accentColor);
                    gridPaint.setAlpha((int) (0.15f * opacity * 255));
                    gridPaint.setStrokeWidth(1f);

                    canvas.drawLine(p1.x, p1.y, p2.x, p2.y, gridPaint);
                }
            }

            // 2. Draw Transverse Lines (moving towards camera)
            // Infinite scrolling: offset Z by (travel % spacing)
            float zOffset = travel % GRID_SPACING;

            for (int i = 0; i < NUM_LINES; i++) {
                // Calculate Z depth for this line
                float z = VISIBILITY - (i * GRID_SPACING) - zOffset;

                if (z < 10f) {
                    continue; // Too close/behind
                }

                // Calculate fade based on distance (Linear fog)
                // Matches desktop: (1.0 - (z / visibility)) * 0.3 * opacity
                float alpha = Math.max(0f, (1f - (z / VISIBILITY))) * 0.3f * opacity;
                if (alpha <= 0.01f) {
                    continue;
                }

                float gridWidth = 1500f; // Width of the grid plane

                PointF p1 = project(-gridWidth, yLevel, z, centerX, centerY, swayAngle);
                PointF p2 = project(gridWidth, yLevel, z, centerX, centerY, swayAngle);

                if (p1 != null && p2 != null) {
                    gridPaint.setColor(accentColor);
                    gridPaint.setAlpha((int) (alpha * 255));
                    gridPaint.setStrokeWidth(1.5f); // Slightly thicker horizontal lines

                    canvas.drawLine(p1.x, p1.y, p2.x, p2.y, gridPaint);
                }
            }
        }
    }

    @Override
    protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        if (animator != null) {
            animator.cancel();
        }
    }
}