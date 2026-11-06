package com.longbridge.nsh;

import android.app.Activity;
import android.os.Bundle;
import com.gouzil.oshwhub.GpuComponent;

/** Thin Android host for the Rust root and its composited GPU surface. */
public final class AppActivity extends Activity {
    private NshView view;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        DevActivity.edgeToEdge(this);
        view = new NshView(this);
        view.useStandaloneLimits();
        view.registerComponent("oshwhub.gpu", events -> new GpuComponent(this, events));
        setContentView(view);
        view.runRustApp("nsh");
    }

    @Override
    public void onConfigurationChanged(android.content.res.Configuration config) {
        super.onConfigurationChanged(config);
        DevActivity.systemBars(this);
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        if (view == null || !view.handleBack()) super.onBackPressed();
    }

    @Override
    protected void onDestroy() {
        if (view != null) view.destroy();
        super.onDestroy();
    }
}
