package com.gouzil.oshwhub;

import android.content.Context;
import android.graphics.SurfaceTexture;
import android.view.Surface;
import android.view.TextureView;
import android.view.View;
import com.longbridge.nsh.NshComponent;
import com.longbridge.nsh.NshComponentContext;
import java.util.HashMap;
import java.util.Map;
import org.json.JSONObject;

/** TextureView preserves NSH clipping, overlays and parent gesture handling. */
public final class GpuComponent implements NshComponent, TextureView.SurfaceTextureListener {
    private final TextureView view;
    private final NshComponentContext events;
    private final Runnable nextFrame = this::render;
    private Surface surface;
    private long host;
    private long model;
    private String lastStatus;
    private boolean destroyed;

    private static native long create(Surface surface, int width, int height);
    private static native String draw(long host, long model, int width, int height);
    private static native void destroy(long host);

    public GpuComponent(Context context, NshComponentContext events) {
        this.events = events;
        view = new TextureView(context) {
            private void resumeFrame() {
                removeCallbacks(nextFrame);
                if (!destroyed && isShown() && getWindowVisibility() == View.VISIBLE) {
                    postOnAnimation(nextFrame);
                }
            }

            @Override
            protected void onVisibilityChanged(View changedView, int visibility) {
                super.onVisibilityChanged(changedView, visibility);
                resumeFrame();
            }

            @Override
            protected void onWindowVisibilityChanged(int visibility) {
                super.onWindowVisibilityChanged(visibility);
                resumeFrame();
            }
        };
        view.setOpaque(true);
        view.setFocusable(false);
        view.setClickable(false);
        view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        view.setSurfaceTextureListener(this);
    }

    @Override
    public View view() { return view; }

    @Override
    public void setProp(String name, Object value) {
        if (!"model".equals(name)) return;
        Object handle = value instanceof Map ? ((Map<?, ?>) value).get("handle") : null;
        model = handle instanceof Number ? ((Number) handle).longValue() : 0;
        render();
    }

    private void render() {
        if (destroyed || surface == null || !view.isShown() || view.getWindowVisibility() != View.VISIBLE || view.getWidth() == 0 || view.getHeight() == 0) return;
        try {
            if (host == 0) host = create(surface, view.getWidth(), view.getHeight());
            String status = draw(host, model, view.getWidth(), view.getHeight());
            if (status == null) return;
            JSONObject result = new JSONObject(status);
            if (result.optBoolean("pending")) {
                view.removeCallbacks(nextFrame);
                view.postOnAnimation(nextFrame);
            } else if (!status.equals(lastStatus)) {
                lastStatus = status;
                if (result.has("error")) events.emit("error", result.getString("error"));
                else {
                    Map<String, Object> data = new HashMap<>();
                    data.put("backend", result.getString("backend"));
                    data.put("width", result.getInt("width"));
                    data.put("height", result.getInt("height"));
                    events.emit("ready", data);
                }
            }
        } catch (Exception error) {
            String message = String.valueOf(error.getMessage());
            if (!message.equals(lastStatus)) {
                lastStatus = message;
                events.emit("error", message);
            }
        }
    }

    private void release() {
        view.removeCallbacks(nextFrame);
        if (host != 0) destroy(host);
        host = 0;
        if (surface != null) surface.release();
        surface = null;
        lastStatus = null;
    }

    @Override
    public void onSurfaceTextureAvailable(SurfaceTexture texture, int width, int height) {
        release();
        surface = new Surface(texture);
        render();
    }

    @Override
    public void onSurfaceTextureSizeChanged(SurfaceTexture texture, int width, int height) { render(); }

    @Override
    public boolean onSurfaceTextureDestroyed(SurfaceTexture texture) { release(); return true; }

    @Override
    public void onSurfaceTextureUpdated(SurfaceTexture texture) {}

    @Override
    public void destroy() {
        destroyed = true;
        view.setSurfaceTextureListener(null);
        release();
    }
}
