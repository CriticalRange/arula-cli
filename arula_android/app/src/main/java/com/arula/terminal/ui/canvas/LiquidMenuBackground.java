package com.arula.terminal.ui.canvas;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;

import com.arula.terminal.R;
import com.arula.terminal.ui.animation.SpringAnimation;

/**
 * Liquid expanding menu background animation
 * Exact port from desktop version's liquid_menu.rs
 * 
 * The menu expands as a circle from bottom-left corner (anchor point at 40,
 * height-40)
 * with radius based on spring progress and alpha = 0.98 * progress
 */
public class LiquidMenuBackground extends View {
    private SpringAnimation spring;
    private Paint circlePaint;
    private ValueAnimator animator;

    // Colors from palette
    private int backgroundColor;

    // Anchor point at bottom left (matching desktop: Point::new(40.0, bounds.height
    // - 40.0))
    private static final float ANCHOR_X = 40f;
    private float anchorY;
    private float maxRadius;

    public LiquidMenuBackground(Context context) {
        super(context);
        init();
    }

    public LiquidMenuBackground(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        init();
    }

    public LiquidMenuBackground(Context context, @Nullable AttributeSet attrs, int defStyleAttr) {
        super(context, attrs, defStyleAttr);
        init();
    }

    private void init() {
        // Initialize spring with default values (matching desktop)
        spring = new SpringAnimation();

        // Get background color from palette
        backgroundColor = getContext().getColor(R.color.neon_background);

        // Initialize paint for the circle
        circlePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        circlePaint.setStyle(Paint.Style.FILL);

        // Start hidden
        setVisibility(View.INVISIBLE);
    }

    @Override
    protected void onSizeChanged(int w, int h, int oldw, int oldh) {
        super.onSizeChanged(w, h, oldw, oldh);
        // Anchor at bottom left (matching desktop)
        anchorY = h - 40f;

        // For vertical/portrait screens, we need to reach the farthest corner
        // (top-right)
        // Calculate diagonal distance from anchor (ANCHOR_X, anchorY) to (w, 0)
        float dx = w - ANCHOR_X;
        float dy = anchorY; // Distance from anchor to top (y=0)
        float diagonalDistance = (float) Math.sqrt(dx * dx + dy * dy);

        // Use the larger of: diagonal distance or desktop formula with some padding
        // This ensures full coverage on both portrait and landscape orientations
        float desktopFormula = Math.max(w, h) * 1.8f;
        maxRadius = Math.max(diagonalDistance * 1.1f, desktopFormula);
    }

    /**
     * Opens the liquid menu with spring animation
     */
    public void openMenu() {
        setVisibility(View.VISIBLE);
        spring.setTarget(1.0f);
        startAnimation();
    }

    /**
     * Closes the liquid menu with spring animation
     */
    public void closeMenu() {
        spring.setTarget(0.0f);
        startAnimation();
    }

    private void startAnimation() {
        if (animator != null && animator.isRunning()) {
            animator.cancel();
        }

        animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(16); // ~60fps tick
        animator.setRepeatCount(ValueAnimator.INFINITE);

        animator.addUpdateListener(animation -> {
            boolean stillAnimating = spring.update();
            invalidate(); // Redraw

            if (!stillAnimating) {
                animation.cancel();
                // Hide completely when closed
                if (spring.getPosition() < 0.01f) {
                    setVisibility(View.INVISIBLE);
                }
            }
        });

        animator.start();
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);

        float progress = spring.getPosition();

        // Don't draw if progress is negligible (matching desktop: if progress < 0.01
        // return)
        if (progress < 0.01f) {
            return;
        }

        // Calculate current radius based on spring progress
        // Matching desktop: let current_radius = max_radius * progress;
        float currentRadius = maxRadius * progress;

        // Calculate alpha based on progress (matching desktop: a: 0.98 *
        // progress.min(1.0))
        float alpha = 0.98f * Math.min(progress, 1.0f);

        // Set color with calculated alpha (matching desktop: ..self.palette.background
        // with alpha)
        int colorWithAlpha = Color.argb(
                (int) (alpha * 255),
                Color.red(backgroundColor),
                Color.green(backgroundColor),
                Color.blue(backgroundColor));
        circlePaint.setColor(colorWithAlpha);

        // Draw the expanding circle from anchor point
        // Matching desktop: let circle = Path::circle(anchor, current_radius);
        // frame.fill(&circle, color);
        canvas.drawCircle(ANCHOR_X, anchorY, currentRadius, circlePaint);
    }

    @Override
    protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        if (animator != null) {
            animator.cancel();
        }
    }

    /**
     * Returns current spring progress (0.0 to 1.0)
     */
    public float getProgress() {
        return spring.getPosition();
    }
}