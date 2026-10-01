//! The WebGPU renderer behind a painted box: vello's hybrid renderer over wgpu, one
//! device shared by every box, with a surface per canvas.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::web::{Canvas, Renderer, kurbo, paint, peniko};
use vello_hybrid::{RenderSize, RenderTargetConfig, Resources, Scene, TextureBindings};
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

/// Whether the browser renders painted content on its GPU.
pub(crate) enum Availability {
  /// Asked, not yet answered; nothing paints until it is.
  Pending,
  /// The renderer.
  Ready(Rc<Context>),
  /// No WebGPU here; the CPU path paints.
  Unavailable,
}

enum State {
  Unasked,
  Pending,
  Ready(Rc<Context>),
  Unavailable,
}

thread_local! {
  static GPU: RefCell<State> = const { RefCell::new(State::Unasked) };
}

/// The GPU renderer, asking the browser for one on the first call and again after the
/// device is lost.
///
/// While the answer is pending the result is [`Availability::Pending`]; `settle` runs
/// once it arrives, ready or not, so every box that waited on it repaints, and `lost`
/// runs once the device it brought is lost, the next call asking afresh.
pub(crate) fn renderer(settle: impl FnOnce() + 'static, lost: fn()) -> Availability {
  let state = GPU.with(|gpu| match &*gpu.borrow() {
    State::Unasked => None,
    State::Pending => Some(Availability::Pending),
    State::Ready(context) => Some(Availability::Ready(Rc::clone(context))),
    State::Unavailable => Some(Availability::Unavailable),
  });

  if let Some(state) = state {
    return state;
  }

  if !supported() {
    GPU.with(|gpu| *gpu.borrow_mut() = State::Unavailable);

    return Availability::Unavailable;
  }

  GPU.with(|gpu| *gpu.borrow_mut() = State::Pending);
  wasm_bindgen_futures::spawn_local(async move {
    let state = match Context::acquire(lost).await {
      Some(context) => State::Ready(Rc::new(context)),
      None => State::Unavailable,
    };

    GPU.with(|gpu| *gpu.borrow_mut() = state);
    settle();
  });

  Availability::Pending
}

/// Whether the page's navigator exposes WebGPU at all.
fn supported() -> bool {
  web_sys::window()
    .map(|window| window.navigator())
    .and_then(|navigator| js_sys::Reflect::get(&navigator, &JsValue::from_str("gpu")).ok())
    .is_some_and(|gpu| !gpu.is_undefined() && !gpu.is_null())
}

/// The device every painted box renders through.
pub(crate) struct Context {
  instance: wgpu::Instance,
  adapter: wgpu::Adapter,
  device: wgpu::Device,
  queue: wgpu::Queue,
  scene: RefCell<Scene>,
  rendering: RefCell<Option<Rendering>>,
}

/// The renderer, made for the surface format the browser prefers.
struct Rendering {
  renderer: vello_hybrid::Renderer,
  resources: Resources,
  format: wgpu::TextureFormat,
}

impl Context {
  async fn acquire(lost: fn()) -> Option<Self> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
      backends: wgpu::Backends::BROWSER_WEBGPU,
      flags: wgpu::InstanceFlags::default(),
      memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
      backend_options: wgpu::BackendOptions::default(),
      display: None,
    });
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions::default())
      .await
      .ok()?;
    let (device, queue) = adapter
      .request_device(&wgpu::DeviceDescriptor::default())
      .await
      .ok()?;

    device.set_device_lost_callback(move |_, _| {
      GPU.with(|gpu| *gpu.borrow_mut() = State::Unasked);
      lost();
    });

    Some(Self {
      instance,
      adapter,
      device,
      queue,
      scene: RefCell::new(Scene::new(1, 1)),
      rendering: RefCell::new(None),
    })
  }

  /// A surface on `canvas`, configured at `width` by `height` device pixels.
  pub(crate) fn target(
    &self,
    canvas: &HtmlCanvasElement,
    width: u16,
    height: u16,
  ) -> Option<Target> {
    let surface = self
      .instance
      .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
      .ok()?;
    let capabilities = surface.get_capabilities(&self.adapter);
    let mut config =
      surface.get_default_config(&self.adapter, u32::from(width), u32::from(height))?;

    if capabilities
      .alpha_modes
      .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
    {
      config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
    }

    surface.configure(&self.device, &config);

    Some(Target { surface, config })
  }

  /// Renders `canvas` into `target`, resized to the canvas first.
  ///
  /// `false` when the surface yielded no frame; the caller paints another way.
  pub(crate) fn render(&self, target: &mut Target, canvas: &Canvas) -> bool {
    let (width, height) = canvas.size();

    target.resize(&self.device, width, height);

    let (wgpu::CurrentSurfaceTexture::Success(frame)
    | wgpu::CurrentSurfaceTexture::Suboptimal(frame)) = target.surface.get_current_texture()
    else {
      return false;
    };
    let view = frame
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
    let mut rendering = self.rendering.borrow_mut();
    let rendering = match &mut *rendering {
      Some(rendering) if rendering.format == target.config.format => rendering,
      slot => {
        let (renderer, resources) = vello_hybrid::Renderer::new(
          &self.device,
          &RenderTargetConfig {
            format: target.config.format,
            width: target.config.width,
            height: target.config.height,
          },
        );

        slot.insert(Rendering {
          renderer,
          resources,
          format: target.config.format,
        })
      }
    };
    let mut scene = self.scene.borrow_mut();

    scene.reset_and_resize(width, height);
    canvas.replay(&mut Hybrid(&mut scene));

    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    let rendered = rendering
      .renderer
      .render(
        &scene,
        &mut rendering.resources,
        &self.device,
        &self.queue,
        &mut encoder,
        &RenderSize {
          width: target.config.width,
          height: target.config.height,
        },
        &view,
        &TextureBindings::new(),
      )
      .is_ok();

    self.queue.submit([encoder.finish()]);
    frame.present();

    rendered
  }
}

/// A canvas's surface and the configuration it was last given.
pub(crate) struct Target {
  surface: wgpu::Surface<'static>,
  config: wgpu::SurfaceConfiguration,
}

impl Target {
  fn resize(&mut self, device: &wgpu::Device, width: u16, height: u16) {
    let (width, height) = (u32::from(width), u32::from(height));

    if self.config.width == width && self.config.height == height {
      return;
    }

    self.config.width = width;
    self.config.height = height;
    self.surface.configure(device, &self.config);
  }
}

/// The hybrid scene as the renderer a canvas replays into.
struct Hybrid<'scene>(&'scene mut Scene);

impl Renderer for Hybrid<'_> {
  #[inline(always)]
  fn set_transform(&mut self, transform: kurbo::Affine) {
    self.0.set_transform(transform);
  }

  fn fill(&mut self, path: &kurbo::BezPath, brush: &peniko::Brush) {
    self.0.set_paint(paint(brush));
    self.0.fill_path(path);
  }

  fn stroke(&mut self, path: &kurbo::BezPath, stroke: &kurbo::Stroke, brush: &peniko::Brush) {
    self.0.set_stroke(stroke.clone());
    self.0.set_paint(paint(brush));
    self.0.stroke_path(path);
  }

  #[inline(always)]
  fn push_clip(&mut self, path: &kurbo::BezPath) {
    self.0.push_clip_layer(path);
  }

  #[inline(always)]
  fn pop(&mut self) {
    self.0.pop_layer();
  }
}
