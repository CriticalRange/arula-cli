package com.arula.terminal.ui.canvas;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.MotionEvent;
import android.widget.FrameLayout;
import androidx.annotation.Nullable;

import com.arula.terminal.R;

/**
 * Interactive card with 3D tilt effect
 * Replicates the desktop version's tilt card interaction
 */
public class TiltCardView extends FrameLayout {
    private Paint cardPaint;
    private Paint shadowPaint;
    private Paint highlightPaint;
    private Path cardPath;
    private RectF cardBounds;

    private int cardColor;
    private int shadowColor;
    private int highlightColor;
    private int borderColor;

    private float tiltAngleX = 0f;
    private float tiltAngleY = 0f;
    private float targetTiltX = 0f;
    private float targetTiltY = 0f;
    private float cornerRadius = 24f;
    private float elevation = 8f;
    private float maxTiltAngle = 15f;

    private boolean isPressed = false;
    private float touchX = 0f;
    private float touchY = 0f;

    private ValueAnimator tiltAnimator;
    private com.arula.terminal.ui.animation.SpringAnimation springX;
    private com.arula.terminal.ui.animation.SpringAnimation springY;

    public TiltCardView(Context context) {
        super(context);
        init();
    }

    public TiltCardView(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        init();
    }

    public TiltCardView(Context context, @Nullable AttributeSet attrs, int defStyleAttr) {
        super(context, attrs, defStyleAttr);
        init();
    }

    private void init() {
        cardColor = getContext().getColor(R.color.neon_surface_raised);
        shadowColor = getContext().getColor(R.color.neon_background);
        borderColor = getContext().getColor(R.color.neon_border);
        highlightColor = getContext().getColor(R.color.neon_glow);

        // Initialize paints
        cardPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        cardPaint.setColor(cardColor);
        cardPaint.setStyle(Paint.Style.FILL);

        shadowPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        shadowPaint.setColor(shadowColor);
        shadowPaint.setStyle(Paint.Style.FILL);
        shadowPaint.setAlpha(100);

        highlightPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        highlightPaint.setColor(highlightColor);
        highlightPaint.setStyle(Paint.Style.STROKE);
        highlightPaint.setStrokeWidth(2f);

        cardPath = new Path();
        cardBounds = new RectF();

        // Initialize spring animations (matching desktop defaults)
        springX = new com.arula.terminal.ui.animation.SpringAnimation();
        springY = new com.arula.terminal.ui.animation.SpringAnimation();

        // Enable touch events
        setClickable(true);

        // Required for custom drawing with children
        setWillNotDraw(false);
    }

    @Override
    protected void onSizeChanged(int w, int h, int oldw, int oldh) {
        super.onSizeChanged(w, h, oldw, oldh);
        float padding = 16f;
        cardBounds.set(padding, padding, w - padding, h - padding);
        updateCardPath();
    }

    private void updateCardPath() {
        cardPath.reset();
        float left = cardBounds.left;
        float top = cardBounds.top - elevation;
        float right = cardBounds.right;
        float bottom = cardBounds.bottom - elevation;
        float r = cornerRadius;

        cardPath.addRoundRect(new RectF(left, top, right, bottom), r, r, Path.Direction.CW);
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        touchX = event.getX();
        touchY = event.getY();

        switch (event.getAction()) {
            case MotionEvent.ACTION_DOWN:
                isPressed = true;
                calculateTargetTilt();
                startTiltAnimation();
                return true;

            case MotionEvent.ACTION_MOVE:
                if (isPressed) {
                    calculateTargetTilt();
                }
                return true;

            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                isPressed = false;
                targetTiltX = 0f;
                targetTiltY = 0f;
                startTiltAnimation();
                performClick();
                return true;
        }

        return super.onTouchEvent(event);
    }

    private void calculateTargetTilt() {
        float centerX = getWidth() / 2f;
        float centerY = getHeight() / 2f;

        // Calculate tilt based on touch position
        float deltaX = (touchX - centerX) / centerX;
        float deltaY = (touchY - centerY) / centerY;

        targetTiltX = deltaY * maxTiltAngle; // Invert Y for natural tilt
        targetTiltY = deltaX * maxTiltAngle;

        // Apply spring physics
        springX.setTarget(targetTiltX / maxTiltAngle);
        springY.setTarget(targetTiltY / maxTiltAngle);
    }

    private void startTiltAnimation() {
        if (tiltAnimator != null) {
            tiltAnimator.cancel();
        }

        tiltAnimator = ValueAnimator.ofFloat(0f, 1f);
        tiltAnimator.setDuration(16); // ~60fps
        tiltAnimator.setRepeatCount(ValueAnimator.INFINITE);

        tiltAnimator.addUpdateListener(animation -> {
            boolean stillAnimatingX = springX.update();
            boolean stillAnimatingY = springY.update();

            tiltAngleX = springX.getPosition() * maxTiltAngle;
            tiltAngleY = springY.getPosition() * maxTiltAngle;

            if (!stillAnimatingX && !stillAnimatingY) {
                animation.cancel();
            }

            invalidate();
        });

        tiltAnimator.start();
    }

    @Override
    protected void dispatchDraw(Canvas canvas) {
        // Draw shadow
        drawShadow(canvas);

        // Apply tilt transformation
        canvas.save();
        applyTiltTransform(canvas);

        // Draw card background
        cardPaint.setColor(cardColor);
        canvas.drawPath(cardPath, cardPaint);

        // Draw border with glow effect if pressed
        if (isPressed || Math.abs(tiltAngleX) > 1f || Math.abs(tiltAngleY) > 1f) {
            float glowAlpha = Math.min(1f, (Math.abs(tiltAngleX) + Math.abs(tiltAngleY)) / maxTiltAngle);
            highlightPaint.setAlpha((int) (glowAlpha * 255));
            canvas.drawPath(cardPath, highlightPaint);
        } else {
            // Draw normal border
            highlightPaint.setColor(borderColor);
            highlightPaint.setAlpha(100);
            canvas.drawPath(cardPath, highlightPaint);
        }

        // Draw children on top of the card
        super.dispatchDraw(canvas);

        canvas.restore();
    }

    private void drawShadow(Canvas canvas) {
        float shadowOffset = elevation + Math.abs(tiltAngleX) * 2f;
        float shadowBlur = elevation * 2f;

        // Draw multiple shadow layers for depth
        for (int i = 3; i > 0; i--) {
            float layerOffset = shadowOffset * i / 3f;
            float layerAlpha = 30f / i;
            float left = cardBounds.left + layerOffset;
            float top = cardBounds.top + layerOffset;
            float right = cardBounds.right + layerOffset;
            float bottom = cardBounds.bottom + layerOffset;

            shadowPaint.setAlpha((int) layerAlpha);
            canvas.drawRoundRect(new RectF(left, top, right, bottom), cornerRadius, cornerRadius, shadowPaint);
        }
    }

    private void applyTiltTransform(Canvas canvas) {
        float centerX = getWidth() / 2f;
        float centerY = getHeight() / 2f;

        // Apply perspective transformation based on tilt angles
        canvas.translate(centerX, centerY);

        // Simulate 3D rotation by scaling based on tilt
        float scaleX = 1f + Math.abs(tiltAngleY) / 100f;
        float scaleY = 1f + Math.abs(tiltAngleX) / 100f;

        canvas.scale(scaleX, scaleY);
        canvas.translate(-centerX, -centerY);
    }

    public void setCardColor(int color) {
        this.cardColor = color;
        invalidate();
    }

    public void setElevation(float elevation) {
        this.elevation = elevation;
        updateCardPath();
        invalidate();
    }

    public void setMaxTiltAngle(float angle) {
        this.maxTiltAngle = angle;
    }

    @Override
    protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        if (tiltAnimator != null) {
            tiltAnimator.cancel();
        }
    }
}