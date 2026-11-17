use std::rc::Rc;

use nsh_core::JsonValue;
#[cfg(target_os = "ios")]
use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
#[cfg(target_os = "macos")]
use nsh_platform_macos::{
    ComponentContext, Components, NativeComponent,
    objc2::{MainThreadMarker, rc::Retained},
    objc2_app_kit::NSView as PlatformView,
};
#[cfg(target_os = "ios")]
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_quartz_core::CAMetalLayer;
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIView as PlatformView;

use super::{SurfaceModel, SurfaceStatus, physical_size};
use crate::viewer::gpu::Gpu;

struct MetalSurface {
    // The GPU surface must be released before its backing layer and NSView.
    gpu: Option<Gpu>,
    layer: Retained<CAMetalLayer>,
    view: Retained<PlatformView>,
    context: ComponentContext,
    size: [f64; 2],
    handle: Option<u64>,
    last_status: Option<SurfaceStatus>,
}

impl MetalSurface {
    fn refresh(&mut self, allow_prepare: bool) -> Result<Option<SurfaceStatus>, String> {
        let [width, height] = self.size;
        if width == 0.0 || height == 0.0 {
            return Ok(None);
        }
        let model = self.handle.and_then(SurfaceModel::resolve);
        if self.handle.is_some() && model.is_none() {
            self.gpu.take();
            return Ok(None);
        }
        if model.as_ref().is_some_and(|model| !model.frame().active) {
            return Ok(None);
        }
        if self.gpu.is_none() {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::METAL,
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            // SAFETY: this component owns the layer until after its GPU is dropped.
            let surface = unsafe {
                instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    Retained::as_ptr(&self.layer).cast_mut().cast(),
                ))
            }
            .map_err(|error| format!("无法创建 Metal 表面：{error}"))?;
            self.gpu = Some(pollster::block_on(Gpu::new(instance, surface))?);
        }
        #[cfg(target_os = "macos")]
        let scale = self
            .view
            .window()
            .map_or(1.0, |window| window.backingScaleFactor());
        #[cfg(target_os = "ios")]
        let scale = {
            // A plain UIView starts at scale 1, independently of its Retina screen.
            #[allow(deprecated)]
            let screen = self.view.window().map_or_else(
                || objc2_ui_kit::UIScreen::mainScreen(self.view.mtm()),
                |window| window.screen(),
            );
            let scale = screen.scale();
            scale
        };
        let gpu = self.gpu.as_mut().expect("GPU initialized above");
        let Some(size) = physical_size(
            width,
            height,
            scale,
            gpu.device.limits().max_texture_dimension_2d,
        )
        .map_err(str::to_owned)?
        else {
            return Ok(None);
        };
        if allow_prepare && gpu.needs_prepare(model.as_deref(), size) {
            pollster::block_on(gpu.prepare(model.as_deref(), size))?;
            #[cfg(target_os = "ios")]
            {
                use objc2_core_foundation::{CGPoint, CGRect, CGSize};
                self.layer.setFrame(CGRect::new(
                    CGPoint::new(0.0, 0.0),
                    CGSize::new(width, height),
                ));
                self.view.setContentScaleFactor(scale);
            }
            self.layer.setContentsScale(scale);
            gpu.configure(size);
        }
        if gpu.draw(model.as_deref())? {
            let [width, height] = gpu.size();
            return Ok(Some(SurfaceStatus::Ready {
                backend: gpu.backend.clone(),
                width,
                height,
            }));
        }
        Ok(None)
    }

    fn publish(&mut self, result: Result<Option<SurfaceStatus>, String>) {
        let status = match result {
            Ok(Some(status)) => status,
            Err(error) => SurfaceStatus::Error(error),
            _ => return,
        };
        if self.last_status.as_ref() == Some(&status) {
            return;
        }
        match &status {
            SurfaceStatus::Ready {
                backend,
                width,
                height,
            } => self.context.emit(
                "ready",
                serde_json::json!({"backend":backend,"width":width,"height":height}),
            ),
            SurfaceStatus::Error(error) => {
                self.context.emit("error", JsonValue::String(error.clone()))
            }
            SurfaceStatus::Pending => {}
        }
        self.last_status = Some(status);
    }
}

impl NativeComponent for MetalSurface {
    #[cfg(target_os = "macos")]
    fn view(&self) -> Retained<PlatformView> {
        self.view.clone()
    }
    #[cfg(target_os = "ios")]
    fn view(&self) -> &PlatformView {
        &self.view
    }
    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        let result = (|| {
            let mut allow_prepare = true;
            match name {
                "size" => {
                    self.size = value
                        .and_then(JsonValue::as_array)
                        .and_then(|size| Some([size.first()?.as_f64()?, size.get(1)?.as_f64()?]))
                        .ok_or("绘图尺寸无效")?
                }
                "model" => {
                    self.handle = value
                        .map(|value| value["handle"].as_u64().ok_or("三维绘图句柄无效"))
                        .transpose()?;
                    let model = self.handle.and_then(SurfaceModel::resolve);
                    allow_prepare = self
                        .gpu
                        .as_ref()
                        .is_none_or(|gpu| gpu.needs_resource_prepare(model.as_deref()));
                    if allow_prepare {
                        self.last_status = None;
                    }
                }
                _ => return Ok(None),
            }
            self.refresh(allow_prepare)
        })();
        self.publish(result);
    }
}

pub fn register(components: &Components) {
    components.register(
        "oshwhub.gpu",
        Rc::new(|context| {
            let mtm =
                MainThreadMarker::new().expect("NSH creates native components on the UI thread");
            let view = PlatformView::new(mtm);
            let layer = CAMetalLayer::new();
            #[cfg(target_os = "macos")]
            {
                view.setWantsLayer(true);
                view.setLayer(Some(&layer));
            }
            #[cfg(target_os = "ios")]
            view.layer().addSublayer(&layer);
            Box::new(MetalSurface {
                gpu: None,
                layer,
                view,
                context,
                size: [0.0; 2],
                handle: None,
                last_status: None,
            })
        }),
    );
}
